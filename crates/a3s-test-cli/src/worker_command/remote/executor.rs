use std::{collections::BTreeSet, ffi::OsString, sync::Arc, time::Duration};

use a3s_test_core::{Surface, SurfaceDriver};
use a3s_test_driver_tui::{TuiDriver, TuiDriverConfig};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserDriver, CommandError, CommandExecutor, CommandInvocation,
    CommandOutput, TokioCommandExecutor,
};
use a3s_test_runner::{RetryPolicy, RunStatus, Runner, RunnerOptions};
use a3s_test_worker::{
    RemoteExecutionJob, RemoteExecutionResult, RemoteJobExecutor, RemoteJobState,
    RemoteScenarioCounts, RemoteWorkerError, WorkerSurface,
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(super) struct ExecutorProfiles {
    pub web: Option<AgentBrowserConfig>,
    pub tui: Option<TuiDriverConfig>,
    pub authorization_environment: OsString,
    pub cleanup_timeout: Duration,
    pub infrastructure_retries: u32,
    pub retry_backoff: Duration,
}

pub(super) fn remote_web_driver(
    config: AgentBrowserConfig,
    authorization_environment: OsString,
) -> AgentBrowserDriver {
    AgentBrowserDriver::with_executor(
        config,
        Arc::new(EnvironmentScrubbingExecutor {
            authorization_environment,
        }),
    )
}

struct EnvironmentScrubbingExecutor {
    authorization_environment: OsString,
}

#[async_trait]
impl CommandExecutor for EnvironmentScrubbingExecutor {
    async fn run(&self, mut invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        invocation.env.remove(&self.authorization_environment);
        invocation
            .env_remove
            .insert(self.authorization_environment.clone());
        TokioCommandExecutor.run(invocation).await
    }
}

pub(super) struct CliRemoteExecutor {
    profiles: ExecutorProfiles,
}

impl CliRemoteExecutor {
    pub(super) fn new(profiles: ExecutorProfiles) -> Self {
        Self { profiles }
    }
}

#[async_trait]
impl RemoteJobExecutor for CliRemoteExecutor {
    async fn execute(
        &self,
        job: RemoteExecutionJob,
        cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        let admitted = crate::read_suite(job.manifest_path())
            .await
            .map_err(|error| execution_error("test.worker.remote.suite_invalid", error, false))?;
        let actual_surfaces = suite_surfaces(&admitted.suite)?;
        let required_surfaces = job
            .required_surfaces()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if actual_surfaces != required_surfaces {
            return Err(RemoteWorkerError::new(
                "test.worker.remote.surface_binding_mismatch",
                "suite surfaces do not exactly match the dispatch requirements",
                false,
            ));
        }

        let mut drivers: Vec<Arc<dyn SurfaceDriver>> = Vec::new();
        if actual_surfaces.contains(&WorkerSurface::Web) {
            let config = self.profiles.web.clone().ok_or_else(|| {
                RemoteWorkerError::new(
                    "test.worker.remote.web_profile_missing",
                    "remote Web execution profile is unavailable",
                    false,
                )
            })?;
            drivers.push(Arc::new(remote_web_driver(
                config,
                self.profiles.authorization_environment.clone(),
            )));
        }
        if actual_surfaces.contains(&WorkerSurface::Tui) {
            let mut config = self.profiles.tui.clone().ok_or_else(|| {
                RemoteWorkerError::new(
                    "test.worker.remote.tui_profile_missing",
                    "remote TUI execution profile is unavailable",
                    false,
                )
            })?;
            if config.command.working_directory.is_none() {
                config.command.working_directory = Some(job.input_root().to_path_buf());
            }
            config
                .command
                .removed_environment
                .insert(self.profiles.authorization_environment.clone());
            config.validate().map_err(|error| {
                RemoteWorkerError::new(error.code(), error.message(), error.retryable())
            })?;
            drivers.push(Arc::new(TuiDriver::new(config)));
        }

        let runner = Runner::new(
            drivers,
            RunnerOptions {
                cleanup_timeout: self.profiles.cleanup_timeout,
                quality_projection_timeout: RunnerOptions::default().quality_projection_timeout,
                retry_policy: RetryPolicy {
                    max_retries: self.profiles.infrastructure_retries,
                    backoff: self.profiles.retry_backoff,
                },
                max_parallel_scenarios: usize::from(job.max_parallel_scenarios()),
            },
        )
        .map_err(|error| {
            RemoteWorkerError::new(
                "test.worker.remote.runner_invalid",
                format!("remote runner configuration is invalid: {error}"),
                false,
            )
        })?
        .with_contracts(admitted.contracts)
        .with_artifacts_root(job.artifacts_root().to_path_buf())
        .map_err(|error| {
            RemoteWorkerError::new(
                "test.worker.remote.artifact_root_invalid",
                format!("remote runner artifact root is invalid: {error}"),
                false,
            )
        })?;
        let result = runner.run(&admitted.suite, cancellation).await;
        let report = serde_json::to_vec(&result).map_err(|error| {
            RemoteWorkerError::new(
                "test.worker.remote.report_encode_failed",
                format!("failed to encode remote run report: {error}"),
                false,
            )
        })?;
        let mut scenarios = RemoteScenarioCounts {
            passed: 0,
            failed: 0,
            timed_out: 0,
            cancelled: 0,
        };
        for scenario in &result.scenarios {
            let count = match scenario.status {
                RunStatus::Passed => &mut scenarios.passed,
                RunStatus::Failed => &mut scenarios.failed,
                RunStatus::TimedOut => &mut scenarios.timed_out,
                RunStatus::Cancelled => &mut scenarios.cancelled,
            };
            *count = count.saturating_add(1);
        }
        Ok(RemoteExecutionResult {
            run_id: result.run_id,
            suite: result.suite,
            status: if result.status == RunStatus::Passed {
                RemoteJobState::Passed
            } else {
                RemoteJobState::Failed
            },
            scenarios,
            report,
            media_type: "application/vnd.a3s-test.run-result+json".to_string(),
        })
    }
}

fn suite_surfaces(
    suite: &a3s_test_core::TestSuite,
) -> Result<BTreeSet<WorkerSurface>, RemoteWorkerError> {
    suite
        .scenarios
        .iter()
        .map(|scenario| match scenario.surface {
            Surface::Web => Ok(WorkerSurface::Web),
            Surface::Tui => Ok(WorkerSurface::Tui),
            Surface::Gui => Err(RemoteWorkerError::new(
                "test.worker.remote.gui_unsupported",
                "reference remote worker does not execute GUI scenarios",
                false,
            )),
        })
        .collect()
}

fn execution_error(code: &'static str, error: anyhow::Error, retryable: bool) -> RemoteWorkerError {
    RemoteWorkerError::new(
        code,
        format!("remote execution admission failed: {error:#}"),
        retryable,
    )
}
