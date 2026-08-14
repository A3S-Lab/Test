use std::{path::Path, sync::Arc, time::Duration};

use a3s_test_driver_tui::TuiCapabilities;
use a3s_test_worker::{
    RemoteExecutionJob, RemoteExecutionResult, RemoteInputBundle, RemoteInputFile,
    RemoteJobExecutor, RemoteJobState, RemoteJobSubmission, RemotePayloadState, RemoteReportQuery,
    RemoteRetentionPolicy, RemoteScenarioCounts, RemoteWorkerDescriptor, RemoteWorkerError,
    RemoteWorkerIdentity, RemoteWorkerLimits, RemoteWorkerService, RemoteWorkerServiceConfig,
    WorkerCapabilityInventory, WorkerSurface, WorkerSurfaceCapability,
};
use async_trait::async_trait;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

const IMAGE_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn descriptor() -> RemoteWorkerDescriptor {
    let inventory = WorkerCapabilityInventory::local(
        1,
        vec![WorkerSurfaceCapability::Tui {
            terminal: TuiCapabilities::compiled().expect("compiled TUI capabilities"),
        }],
    )
    .expect("worker inventory");
    RemoteWorkerDescriptor::new(
        RemoteWorkerIdentity {
            instance_id: "runner-retention".to_string(),
            image_digest: IMAGE_DIGEST.to_string(),
        },
        inventory,
        RemoteWorkerLimits::default(),
    )
    .expect("worker descriptor")
}

fn submission(
    descriptor: &RemoteWorkerDescriptor,
    now_ms: u64,
    job_id: &str,
) -> RemoteJobSubmission {
    RemoteJobSubmission {
        job_id: job_id.to_string(),
        dispatch_id: format!("dispatch-{job_id}"),
        worker_instance: descriptor.identity.instance_id.clone(),
        required_image_digest: descriptor.identity.image_digest.clone(),
        required_inventory_digest: descriptor.inventory_digest.clone(),
        issued_at_ms: now_ms,
        deadline_ms: now_ms + 60_000,
        lease_expires_at_ms: now_ms + 30_000,
        max_parallel_scenarios: 1,
        required_surfaces: vec![WorkerSurface::Tui],
        input: RemoteInputBundle {
            manifest: "suite.acl".to_string(),
            files: vec![RemoteInputFile::from_bytes(
                "suite.acl",
                format!("suite \"{job_id}\" {{ version = 1 }}\n"),
            )],
        },
    }
}

struct SizedExecutor {
    evidence_bytes: usize,
}

#[async_trait]
impl RemoteJobExecutor for SizedExecutor {
    async fn execute(
        &self,
        job: RemoteExecutionJob,
        _cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        tokio::fs::create_dir_all(job.artifacts_root())
            .await
            .expect("artifact root");
        tokio::fs::write(
            job.artifacts_root().join("evidence.bin"),
            vec![b'x'; self.evidence_bytes],
        )
        .await
        .expect("sized evidence");
        Ok(RemoteExecutionResult {
            run_id: format!("run-{}", job.job_id()),
            suite: format!("suite-{}", job.job_id()),
            status: RemoteJobState::Passed,
            scenarios: RemoteScenarioCounts {
                passed: 1,
                failed: 0,
                timed_out: 0,
                cancelled: 0,
            },
            report: format!("{{\"job\":\"{}\"}}", job.job_id()).into_bytes(),
            media_type: "application/json".to_string(),
        })
    }
}

async fn open_service(
    root: &Path,
    descriptor: RemoteWorkerDescriptor,
    retention: RemoteRetentionPolicy,
    evidence_bytes: usize,
) -> RemoteWorkerService {
    RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(root.to_path_buf(), descriptor)
            .with_retention_policy(retention),
        Arc::new(SizedExecutor { evidence_bytes }),
    )
    .await
    .expect("open retained service")
}

async fn run_job(service: &RemoteWorkerService, descriptor: &RemoteWorkerDescriptor, job_id: &str) {
    service
        .submit(submission(descriptor, service.now_ms(), job_id))
        .await
        .expect("submit retained job");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = service
                .status(job_id, &format!("dispatch-{job_id}"))
                .await
                .expect("retained job status");
            if snapshot.state.terminal() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("retained job completion");
}

async fn reports(service: &RemoteWorkerService) -> Vec<(String, RemotePayloadState)> {
    service
        .list_reports(RemoteReportQuery {
            states: vec![RemoteJobState::Passed],
            suite: None,
            run_id: None,
            finished_after_ms: None,
            finished_before_ms: None,
            limit: 100,
            cursor: None,
        })
        .await
        .expect("retained reports")
        .reports
        .into_iter()
        .map(|entry| (entry.job.job_id, entry.payload_state))
        .collect()
}

#[tokio::test]
async fn retention_enforces_the_aggregate_payload_byte_budget() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let policy = RemoteRetentionPolicy {
        max_retained_jobs: 10,
        max_retained_bytes: 1024 * 1024,
        max_retention_age_ms: 60_000,
        max_indexed_jobs: 10,
        max_index_age_ms: 120_000,
    };
    let service = open_service(temp.path(), descriptor.clone(), policy, 600 * 1024).await;
    run_job(&service, &descriptor, "job-byte-1").await;
    run_job(&service, &descriptor, "job-byte-2").await;

    assert_eq!(
        reports(&service).await,
        vec![
            ("job-byte-2".to_string(), RemotePayloadState::Retained),
            ("job-byte-1".to_string(), RemotePayloadState::Pruned),
        ]
    );
    service.shutdown().await.expect("service shutdown");
}

#[tokio::test(start_paused = true)]
async fn idle_retention_enforces_payload_and_index_age_independently() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let policy = RemoteRetentionPolicy {
        max_retained_jobs: 10,
        max_retained_bytes: 1024 * 1024,
        max_retention_age_ms: 1_000,
        max_indexed_jobs: 10,
        max_index_age_ms: 2_000,
    };
    let service = open_service(temp.path(), descriptor.clone(), policy, 32).await;
    run_job(&service, &descriptor, "job-age-1").await;

    tokio::time::advance(Duration::from_millis(1_000)).await;
    wait_for_reports(
        &service,
        vec![("job-age-1".to_string(), RemotePayloadState::Pruned)],
    )
    .await;

    tokio::time::advance(Duration::from_millis(1_000)).await;
    wait_for_reports(&service, Vec::new()).await;
    assert!(!temp.path().join("jobs/job-age-1").exists());
    service.shutdown().await.expect("service shutdown");
}

async fn wait_for_reports(
    service: &RemoteWorkerService,
    expected: Vec<(String, RemotePayloadState)>,
) {
    for _ in 0..100 {
        let actual = reports(service).await;
        if actual == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(reports(service).await, expected);
}

#[tokio::test]
async fn lowering_the_deployment_budget_prunes_existing_payload_on_restart() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let initial_policy = RemoteRetentionPolicy {
        max_retained_jobs: 10,
        max_retained_bytes: 2 * 1024 * 1024,
        max_retention_age_ms: 60_000,
        max_indexed_jobs: 10,
        max_index_age_ms: 120_000,
    };
    let first = open_service(temp.path(), descriptor.clone(), initial_policy, 1200 * 1024).await;
    run_job(&first, &descriptor, "job-budget").await;
    assert_eq!(
        reports(&first).await,
        vec![("job-budget".to_string(), RemotePayloadState::Retained)]
    );
    first.shutdown().await.expect("first shutdown");
    drop(first);

    let lower_policy = RemoteRetentionPolicy {
        max_retained_jobs: 10,
        max_retained_bytes: 1024 * 1024,
        max_retention_age_ms: 60_000,
        max_indexed_jobs: 10,
        max_index_age_ms: 120_000,
    };
    let reopened = open_service(temp.path(), descriptor, lower_policy, 1).await;
    assert_eq!(
        reports(&reopened).await,
        vec![("job-budget".to_string(), RemotePayloadState::Pruned)]
    );
    reopened.shutdown().await.expect("reopened shutdown");
}
