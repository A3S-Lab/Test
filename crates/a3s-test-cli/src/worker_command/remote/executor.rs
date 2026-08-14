use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Component, Path},
    sync::Arc,
    time::Duration,
};

use a3s_test_core::{Surface, SurfaceDriver};
use a3s_test_driver_gui::{GuiDriver, GuiDriverConfig};
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
    pub gui: Option<GuiDriverConfig>,
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
        let mut admitted = crate::read_suite(job.manifest_path())
            .await
            .map_err(|error| execution_error("test.worker.remote.suite_invalid", error, false))?;
        select_scenarios(&mut admitted.suite, job.scenario_ids())?;
        bind_upload_inputs(&mut admitted.suite, job.input_root()).await?;
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
        if actual_surfaces.contains(&WorkerSurface::Gui) {
            let config = self.profiles.gui.clone().ok_or_else(|| {
                RemoteWorkerError::new(
                    "test.worker.remote.gui_profile_missing",
                    "remote GUI execution profile is unavailable",
                    false,
                )
            })?;
            drivers.push(Arc::new(GuiDriver::new(config)));
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

fn select_scenarios(
    suite: &mut a3s_test_core::TestSuite,
    selected: &[String],
) -> Result<(), RemoteWorkerError> {
    let selected = selected.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let available = suite
        .scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect::<BTreeSet<_>>();
    if selected
        .iter()
        .any(|scenario| !available.contains(scenario))
    {
        return Err(RemoteWorkerError::new(
            "test.worker.remote.scenario_selection_mismatch",
            "remote scenario selection names a scenario absent from the admitted suite",
            false,
        ));
    }
    suite
        .scenarios
        .retain(|scenario| selected.contains(scenario.id.as_str()));
    if suite.scenarios.len() != selected.len() {
        return Err(RemoteWorkerError::new(
            "test.worker.remote.scenario_selection_mismatch",
            "remote scenario selection could not be resolved exactly once",
            false,
        ));
    }
    Ok(())
}

async fn bind_upload_inputs(
    suite: &mut a3s_test_core::TestSuite,
    input_root: &Path,
) -> Result<(), RemoteWorkerError> {
    let canonical_root = tokio::fs::canonicalize(input_root).await.map_err(|error| {
        RemoteWorkerError::new(
            "test.worker.remote.input_root_invalid",
            format!("failed to resolve the private input root: {error}"),
            false,
        )
    })?;
    for action in suite
        .scenarios
        .iter_mut()
        .flat_map(|scenario| &mut scenario.steps)
        .map(|step| &mut step.action)
    {
        let a3s_test_core::Action::Upload { paths, .. } = action else {
            continue;
        };
        for path in paths {
            let relative = Path::new(path);
            if relative.is_absolute()
                || relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
            {
                return Err(RemoteWorkerError::new(
                    "test.worker.remote.upload_path_invalid",
                    "remote upload paths must be contained relative input paths",
                    false,
                ));
            }
            let mut requested = canonical_root.clone();
            for component in relative.components() {
                match component {
                    Component::Normal(component) => requested.push(component),
                    Component::CurDir => continue,
                    _ => {
                        return Err(RemoteWorkerError::new(
                            "test.worker.remote.upload_path_invalid",
                            "remote upload path changed after validation",
                            false,
                        ));
                    }
                }
                let metadata = tokio::fs::symlink_metadata(&requested)
                    .await
                    .map_err(|error| {
                        RemoteWorkerError::new(
                            "test.worker.remote.upload_path_invalid",
                            format!("failed to inspect remote upload input: {error}"),
                            false,
                        )
                    })?;
                if is_link_like(&metadata) {
                    return Err(RemoteWorkerError::new(
                        "test.worker.remote.upload_path_invalid",
                        "remote upload input cannot traverse a link or reparse point",
                        false,
                    ));
                }
            }
            let metadata = tokio::fs::metadata(&requested).await.map_err(|error| {
                RemoteWorkerError::new(
                    "test.worker.remote.upload_path_invalid",
                    format!("failed to inspect remote upload input: {error}"),
                    false,
                )
            })?;
            if !metadata.is_file() {
                return Err(RemoteWorkerError::new(
                    "test.worker.remote.upload_path_invalid",
                    "remote upload input must be a regular non-link file",
                    false,
                ));
            }
            let canonical = tokio::fs::canonicalize(&requested).await.map_err(|error| {
                RemoteWorkerError::new(
                    "test.worker.remote.upload_path_invalid",
                    format!("failed to resolve remote upload input: {error}"),
                    false,
                )
            })?;
            if !canonical.starts_with(&canonical_root) {
                return Err(RemoteWorkerError::new(
                    "test.worker.remote.upload_path_invalid",
                    "remote upload input escaped the private input root",
                    false,
                ));
            }
            *path = canonical.to_str().map(ToOwned::to_owned).ok_or_else(|| {
                RemoteWorkerError::new(
                    "test.worker.remote.upload_path_invalid",
                    "remote upload input path is not portable UTF-8",
                    false,
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn suite_surfaces(
    suite: &a3s_test_core::TestSuite,
) -> Result<BTreeSet<WorkerSurface>, RemoteWorkerError> {
    suite
        .scenarios
        .iter()
        .map(|scenario| match scenario.surface {
            Surface::Web => Ok(WorkerSurface::Web),
            Surface::Gui => Ok(WorkerSurface::Gui),
            Surface::Tui => Ok(WorkerSurface::Tui),
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

#[cfg(test)]
mod tests {
    use super::{bind_upload_inputs, select_scenarios, suite_surfaces};
    use a3s_test_core::{Action, TestSuite};
    use a3s_test_worker::WorkerSurface;
    use std::path::Path;

    #[test]
    fn selection_runs_only_the_exact_requested_scenarios() {
        let mut suite = TestSuite::from_acl(
            r#"suite "distributed" {
  scenario "alpha" { surface = "tui" expect "a" { text = "a" } }
  scenario "beta" { surface = "tui" expect "b" { text = "b" } }
}
"#,
        )
        .expect("suite");
        select_scenarios(&mut suite, &["beta".to_string()]).expect("selection");
        assert_eq!(suite.scenarios.len(), 1);
        assert_eq!(suite.scenarios[0].id, "beta");
        let error = select_scenarios(&mut suite, &["alpha".to_string()])
            .expect_err("missing selected scenario");
        assert_eq!(
            error.code(),
            "test.worker.remote.scenario_selection_mismatch"
        );
    }

    #[test]
    fn gui_suites_map_to_the_remote_gui_surface() {
        let suite = TestSuite::from_acl(
            r#"suite "remote-gui" {
  scenario "editor" {
    surface = "gui"
    click "save" { target = automation_id("save-button") }
  }
}
"#,
        )
        .expect("GUI suite");

        assert_eq!(
            suite_surfaces(&suite).expect("remote surface mapping"),
            [WorkerSurface::Gui].into_iter().collect()
        );
    }

    #[tokio::test]
    async fn upload_paths_are_bound_to_the_private_input_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("avatar.png"), b"avatar").expect("avatar");
        let mut suite = TestSuite::from_acl(
            r#"suite "distributed" {
  scenario "upload" {
    surface = "web"
    upload "avatar" { target = testid("avatar") paths = ["avatar.png"] }
  }
}
"#,
        )
        .expect("suite");
        bind_upload_inputs(&mut suite, temp.path())
            .await
            .expect("bound upload");
        let Action::Upload { paths, .. } = &suite.scenarios[0].steps[0].action else {
            panic!("upload action");
        };
        assert_eq!(
            Path::new(&paths[0]),
            &temp
                .path()
                .canonicalize()
                .expect("canonical temp")
                .join("avatar.png")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn upload_paths_reject_linked_components() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("avatar.png"), b"avatar").expect("avatar");
        symlink(outside.path(), temp.path().join("linked")).expect("linked input");
        let mut suite = TestSuite::from_acl(
            r#"suite "distributed" {
  scenario "upload" {
    surface = "web"
    upload "avatar" { target = testid("avatar") paths = ["linked/avatar.png"] }
  }
}
"#,
        )
        .expect("suite");
        let error = bind_upload_inputs(&mut suite, temp.path())
            .await
            .expect_err("linked upload input");
        assert_eq!(error.code(), "test.worker.remote.upload_path_invalid");
        assert!(error.message.contains("link"));
    }
}
