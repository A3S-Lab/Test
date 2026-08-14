use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::Surface;
use a3s_test_worker::{
    analyze_distributed_run, plan_distributed_run, DistributedAnalysisRequest,
    DistributedPlanRequest, DistributedRunAnalysis, DistributedRunPlan, DistributedScenarioSpec,
    DistributedWorkerSpec, RemoteArtifactCommand, RemoteArtifactDescriptor, RemoteArtifactOutcome,
    RemoteArtifactRequest, RemoteWorkerCommand, RemoteWorkerDescriptor, RemoteWorkerOutcome,
    RemoteWorkerRequest, WorkerSurface, REMOTE_ARTIFACT_PROTOCOL, REMOTE_WORKER_PROTOCOL,
};
use anyhow::Result;
use futures::future::join_all;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::config::{self, DistributedConfig, WorkerConfig};
use super::history::HistoryStore;
use super::http::RemoteHttpClient;
use super::input::{self, PreparedSuite};
use super::shard::{execute_shard, ShardExecution};
use super::{operation_id, request_id, unix_ms};

const MAX_CONCURRENT_SUBMISSIONS: usize = 4;

#[derive(Clone, Debug)]
pub(super) struct InspectedWorker {
    pub config: WorkerConfig,
    pub client: RemoteHttpClient,
    pub descriptor: RemoteWorkerDescriptor,
    pub artifacts: RemoteArtifactDescriptor,
}

struct PreparedDistributedRun {
    config: Arc<DistributedConfig>,
    suite: PreparedSuite,
    history: Vec<a3s_test_worker::DistributedHistoryRun>,
    history_store: HistoryStore,
    plan: DistributedRunPlan,
    workers: BTreeMap<String, InspectedWorker>,
    scenario_surfaces: BTreeMap<String, WorkerSurface>,
}

pub(super) async fn create_plan(config_path: &Path) -> Result<DistributedRunPlan> {
    let now_ms = unix_ms()?;
    Ok(prepare(config_path, now_ms, None).await?.plan)
}

pub(super) async fn run(
    config_path: &Path,
    cancellation: CancellationToken,
) -> Result<(DistributedRunAnalysis, PathBuf)> {
    let started_at_ms = unix_ms()?;
    let prepared = prepare(config_path, started_at_ms, Some(&cancellation)).await?;
    let run_id = scoped_operation_id("run", &prepared.config.id, started_at_ms);
    if cancellation.is_cancelled() {
        anyhow::bail!("distributed run was cancelled during planning");
    }

    let submission_slots = Arc::new(Semaphore::new(MAX_CONCURRENT_SUBMISSIONS));
    let input = Arc::new(prepared.suite.bundle.clone());
    let scenario_surfaces = Arc::new(prepared.scenario_surfaces.clone());
    let executions = join_all(prepared.plan.shards.iter().map(|shard| {
        let worker = prepared
            .workers
            .get(&shard.worker_instance)
            .expect("planned worker was inspected")
            .clone();
        execute_shard(
            Arc::clone(&prepared.config),
            worker,
            shard.clone(),
            run_id.clone(),
            prepared.plan.suite.clone(),
            Arc::clone(&input),
            Arc::clone(&scenario_surfaces),
            Arc::clone(&submission_slots),
            cancellation.clone(),
        )
    }))
    .await;
    let (scenarios, shard_issues) = collect_shards(executions, &prepared.plan)?;
    let finished_at_ms = unix_ms()?.max(started_at_ms);
    let analysis = analyze_distributed_run(DistributedAnalysisRequest {
        plan_id: prepared.plan.plan_id.clone(),
        plan_digest: prepared.plan.plan_digest.clone(),
        run_id: run_id.clone(),
        suite: prepared.plan.suite.clone(),
        suite_digest: prepared.plan.suite_digest.clone(),
        started_at_ms,
        finished_at_ms,
        history_window: prepared.config.history_window,
        scenarios,
        quarantines: prepared.plan.quarantines.clone(),
        history: prepared.history,
        shard_issues,
    })
    .map_err(anyhow::Error::new)?;
    prepared
        .history_store
        .persist(
            &analysis,
            prepared.config.history_max_runs,
            prepared.config.history_max_age_ms,
        )
        .await?;
    let report_path = prepared.history_store.report_path(&run_id);
    Ok((analysis, report_path))
}

async fn prepare(
    config_path: &Path,
    created_at_ms: u64,
    cancellation: Option<&CancellationToken>,
) -> Result<PreparedDistributedRun> {
    let config = Arc::new(config::load_config(config_path).await?);
    ensure_not_cancelled(cancellation, "configuration admission")?;
    let plan_id = scoped_operation_id("plan", &config.id, created_at_ms);
    let suite = input::prepare_suite(&config).await?;
    ensure_not_cancelled(cancellation, "input preparation")?;
    let scenario_surfaces = scenario_surfaces(&suite)?;
    let history_root = config.config_directory.join(&config.history_root);
    let history_store = HistoryStore::open(history_root).await?;
    let history = history_store
        .load(
            created_at_ms,
            config.history_max_runs,
            config.history_max_age_ms,
        )
        .await?;
    ensure_not_cancelled(cancellation, "history loading")?;
    let inspected = inspect_workers(&config, cancellation).await?;
    validate_run_limits(&config, &inspected)?;
    let workers = inspected
        .iter()
        .map(|worker| {
            (
                worker.config.instance_id.clone(),
                DistributedWorkerSpec {
                    instance_id: worker.config.instance_id.clone(),
                    image_digest: worker.descriptor.identity.image_digest.clone(),
                    inventory_digest: worker.descriptor.inventory_digest.clone(),
                    max_parallel_scenarios: worker.config.max_parallel_scenarios,
                    surfaces: worker
                        .descriptor
                        .inventory
                        .surfaces
                        .iter()
                        .map(|surface| surface.surface())
                        .collect(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let plan = plan_distributed_run(DistributedPlanRequest {
        plan_id,
        suite: suite.suite.name.clone(),
        suite_digest: suite.suite_digest.clone(),
        created_at_ms,
        scenarios: suite
            .suite
            .scenarios
            .iter()
            .map(|scenario| DistributedScenarioSpec {
                id: scenario.id.clone(),
                surface: scenario_surfaces[&scenario.id],
                fallback_duration_ms: scenario.timeout_ms.max(1),
            })
            .collect(),
        workers: workers.into_values().collect(),
        history: history.clone(),
        quarantines: config.quarantines.clone(),
    })
    .map_err(anyhow::Error::new)?;
    let workers = inspected
        .into_iter()
        .map(|worker| (worker.config.instance_id.clone(), worker))
        .collect();
    Ok(PreparedDistributedRun {
        config,
        suite,
        history,
        history_store,
        plan,
        workers,
        scenario_surfaces,
    })
}

fn scoped_operation_id(kind: &str, scope: &str, now_ms: u64) -> String {
    let scope = &scope[..scope.len().min(32)];
    operation_id(&format!("{kind}-{scope}"), now_ms)
}

fn scenario_surfaces(suite: &PreparedSuite) -> Result<BTreeMap<String, WorkerSurface>> {
    suite
        .suite
        .scenarios
        .iter()
        .map(|scenario| {
            let surface = match scenario.surface {
                Surface::Web => WorkerSurface::Web,
                Surface::Tui => WorkerSurface::Tui,
                Surface::Gui => {
                    anyhow::bail!(
                        "distributed reference workers do not execute GUI scenario '{}'",
                        scenario.id
                    )
                }
            };
            Ok((scenario.id.clone(), surface))
        })
        .collect()
}

async fn inspect_workers(
    config: &DistributedConfig,
    cancellation: Option<&CancellationToken>,
) -> Result<Vec<InspectedWorker>> {
    let inspections = config.workers.iter().cloned().map(|worker| {
        let cancellation = cancellation.cloned();
        async move {
            match cancellation {
                Some(cancellation) => {
                    tokio::select! {
                        biased;
                        _ = cancellation.cancelled() => {
                            anyhow::bail!("distributed run was cancelled during worker inspection")
                        }
                        result = inspect_worker(worker, config.http_timeout_ms) => result,
                    }
                }
                None => inspect_worker(worker, config.http_timeout_ms).await,
            }
        }
    });
    join_all(inspections).await.into_iter().collect()
}

fn ensure_not_cancelled(cancellation: Option<&CancellationToken>, stage: &str) -> Result<()> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        anyhow::bail!("distributed run was cancelled during {stage}");
    }
    Ok(())
}

async fn inspect_worker(worker: WorkerConfig, timeout_ms: u64) -> Result<InspectedWorker> {
    let client = RemoteHttpClient::new(&worker, Duration::from_millis(timeout_ms))?;
    let worker_request = RemoteWorkerRequest {
        protocol: REMOTE_WORKER_PROTOCOL.to_string(),
        request_id: request_id("inspect-worker"),
        command: RemoteWorkerCommand::Inspect,
    };
    let artifact_request = RemoteArtifactRequest {
        protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
        request_id: request_id("inspect-artifacts"),
        command: RemoteArtifactCommand::Inspect,
    };
    let (worker_response, artifact_response) = tokio::try_join!(
        client.worker(&worker_request),
        client.artifacts(&artifact_request)
    )?;
    let descriptor = match worker_response.outcome {
        RemoteWorkerOutcome::Descriptor { worker } => worker,
        RemoteWorkerOutcome::Error { error } => {
            anyhow::bail!(
                "worker inspection failed [{}]: {}",
                error.code,
                error.message
            )
        }
        RemoteWorkerOutcome::Job { .. } => {
            anyhow::bail!("worker inspection returned an unexpected job outcome")
        }
    };
    let artifacts = match artifact_response.outcome {
        RemoteArtifactOutcome::Descriptor { service } => service,
        RemoteArtifactOutcome::Error { error } => {
            anyhow::bail!(
                "artifact service inspection failed [{}]: {}",
                error.code,
                error.message
            )
        }
        _ => anyhow::bail!("artifact service inspection returned an unexpected outcome"),
    };
    validate_worker_binding(&worker, &descriptor, &artifacts)?;
    Ok(InspectedWorker {
        config: worker,
        client,
        descriptor,
        artifacts,
    })
}

fn validate_worker_binding(
    config: &WorkerConfig,
    descriptor: &RemoteWorkerDescriptor,
    artifacts: &RemoteArtifactDescriptor,
) -> Result<()> {
    descriptor.validate().map_err(anyhow::Error::new)?;
    artifacts.validate().map_err(anyhow::Error::new)?;
    if descriptor.identity.instance_id != config.instance_id
        || descriptor.identity.image_digest != config.image_digest
    {
        anyhow::bail!(
            "worker '{}' identity or image digest does not match the distributed config",
            config.instance_id
        );
    }
    if config
        .inventory_digest
        .as_ref()
        .is_some_and(|expected| expected != &descriptor.inventory_digest)
    {
        anyhow::bail!(
            "worker '{}' inventory digest does not match the configured pin",
            config.instance_id
        );
    }
    if artifacts.worker != descriptor.identity
        || artifacts.inventory_digest != descriptor.inventory_digest
    {
        anyhow::bail!(
            "worker '{}' artifact service is not bound to the inspected runtime",
            config.instance_id
        );
    }
    if config.max_parallel_scenarios > descriptor.inventory.max_parallel_scenarios {
        anyhow::bail!(
            "worker '{}' configured concurrency exceeds its inspected inventory",
            config.instance_id
        );
    }
    Ok(())
}

fn validate_run_limits(config: &DistributedConfig, workers: &[InspectedWorker]) -> Result<()> {
    for worker in workers {
        let limits = &worker.descriptor.limits;
        if config.job_timeout_ms > limits.max_job_duration_ms
            || config.lease_ms > limits.max_lease_ms
            || config.lease_ms > config.job_timeout_ms
        {
            anyhow::bail!(
                "worker '{}' cannot admit the configured job deadline or lease",
                worker.config.instance_id
            );
        }
        if worker.artifacts.retention.max_retention_age_ms < config.http_timeout_ms {
            anyhow::bail!(
                "worker '{}' artifact retention age is shorter than one configured HTTP deadline",
                worker.config.instance_id
            );
        }
    }
    Ok(())
}

fn collect_shards(
    mut executions: Vec<ShardExecution>,
    plan: &DistributedRunPlan,
) -> Result<(
    Vec<a3s_test_worker::DistributedScenarioObservation>,
    Vec<a3s_test_worker::DistributedShardIssue>,
)> {
    executions.sort_by_key(|execution| execution.index);
    let scenarios = executions
        .iter()
        .flat_map(|execution| execution.observations.iter().cloned())
        .collect::<Vec<_>>();
    let issues = executions
        .into_iter()
        .flat_map(|execution| execution.issues)
        .collect::<Vec<_>>();
    let actual = scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected = plan
        .shards
        .iter()
        .flat_map(|shard| &shard.scenario_ids)
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual.len() != scenarios.len() || actual != expected {
        anyhow::bail!("distributed shard results do not cover every planned scenario exactly once");
    }
    Ok((scenarios, issues))
}
