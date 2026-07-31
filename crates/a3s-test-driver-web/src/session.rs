use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use a3s_test_core::{
    Action, DriverError, DriverSession, Evidence, Expectation, ScenarioContext, StepOutput,
    Surface, SurfaceDriver, SurfaceObservation, TestStep,
};
use async_trait::async_trait;
use serde_json::Value;

use crate::process::{create_runtime_directory, terminate_owned_session, SessionRegistration};
use crate::protocol::{
    bounded, compact_component, direct_selector, invocation, resolve_artifact_path, scalar_bool,
    scalar_string, target_action, validate_component, wait_args,
};
use crate::{AgentBrowserConfig, CommandExecutor, TokioCommandExecutor};

pub struct AgentBrowserDriver {
    config: AgentBrowserConfig,
    executor: Arc<dyn CommandExecutor>,
}

impl AgentBrowserDriver {
    #[must_use]
    pub fn new(config: AgentBrowserConfig) -> Self {
        Self::with_executor(config, Arc::new(TokioCommandExecutor))
    }

    #[must_use]
    pub fn with_executor(config: AgentBrowserConfig, executor: Arc<dyn CommandExecutor>) -> Self {
        Self { config, executor }
    }
}

#[async_trait]
impl SurfaceDriver for AgentBrowserDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError> {
        self.config.validate()?;
        let requested_namespace = if self.config.namespace.is_empty() {
            context.run_id.clone()
        } else {
            self.config.namespace.clone()
        };
        validate_component(&requested_namespace, "namespace")?;
        validate_component(&context.run_id, "run id")?;
        validate_component(&context.scenario_id, "scenario id")?;
        let namespace = compact_component(&requested_namespace, 24);
        let session = compact_component(&context.scenario_id, 32);
        let artifacts_dir = absolute_artifacts_dir(&context.artifacts_dir)?;
        let runtime_guard = tokio::task::spawn_blocking(create_runtime_directory)
            .await
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.runtime_create_failed",
                    format!("failed to join browser runtime setup: {error}"),
                )
            })?
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.runtime_create_failed",
                    format!("failed to create browser runtime directory: {error}"),
                )
            })?;
        let runtime_dir = runtime_guard.path().to_path_buf();
        let registration = SessionRegistration::new(
            runtime_dir.clone(),
            namespace.clone(),
            session.clone(),
            self.config.command.process_markers(),
        );

        Ok(Box::new(AgentBrowserSession {
            config: self.config.clone(),
            namespace,
            session,
            runtime_dir,
            runtime_guard: Some(runtime_guard),
            registration: Some(registration),
            artifacts_dir,
            executor: Arc::clone(&self.executor),
            closed: false,
        }))
    }
}

struct AgentBrowserSession {
    config: AgentBrowserConfig,
    namespace: String,
    session: String,
    artifacts_dir: PathBuf,
    runtime_dir: PathBuf,
    runtime_guard: Option<tempfile::TempDir>,
    registration: Option<SessionRegistration>,
    executor: Arc<dyn CommandExecutor>,
    closed: bool,
}

#[async_trait]
impl DriverSession for AgentBrowserSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        self.ensure_open()?;
        let data = self
            .execute_command(vec![OsString::from("snapshot")])
            .await?;
        Ok(SurfaceObservation::new("browser accessibility snapshot").with_data(data))
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.ensure_open()?;

        match &step.action {
            Action::Navigate { url } => self
                .execute_command(vec!["open".into(), url.into()])
                .await
                .map(|data| StepOutput::new("page opened").with_data(data)),
            Action::Snapshot { interactive } => {
                let mut args = vec![OsString::from("snapshot")];
                if *interactive {
                    args.push(OsString::from("-i"));
                }
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("page snapshot captured").with_data(data))
            }
            Action::Click { target } => {
                let args = target_action(target, "click", None)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("target clicked").with_data(data))
            }
            Action::Fill { target, value } => {
                let args = target_action(target, "fill", Some(value))?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("target filled").with_data(data))
            }
            Action::Press { key } => self
                .execute_command(vec!["press".into(), key.into()])
                .await
                .map(|data| StepOutput::new("key pressed").with_data(data)),
            Action::Wait { condition } => {
                let args = wait_args(condition);
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("wait condition satisfied").with_data(data))
            }
            Action::Assert { expectation } => self.assert(expectation).await,
            Action::Screenshot { path } => self.screenshot(path).await,
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        if self.closed {
            return Ok(());
        }

        match self.execute_command(vec![OsString::from("close")]).await {
            Ok(_) => {
                self.closed = true;
                Ok(())
            }
            Err(error) => {
                if self.emergency_cleanup().await {
                    self.closed = true;
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }
}

impl AgentBrowserSession {
    fn ensure_open(&self) -> Result<(), DriverError> {
        if self.closed {
            return Err(DriverError::new(
                "test.driver.web.session_closed",
                "browser session is already closed",
            ));
        }
        Ok(())
    }

    async fn assert(&self, expectation: &Expectation) -> Result<StepOutput, DriverError> {
        match expectation {
            Expectation::TextVisible(text) => {
                let data = self
                    .execute_command(vec!["wait".into(), "--text".into(), text.into()])
                    .await?;
                Ok(StepOutput::new("text is visible").with_data(data))
            }
            Expectation::Url(expected) => {
                let data = self
                    .execute_command(vec!["get".into(), "url".into()])
                    .await?;
                let actual = scalar_string(&data).ok_or_else(|| {
                    DriverError::new(
                        "test.driver.web.output_invalid",
                        "browser URL response did not contain a string",
                    )
                })?;
                if actual != expected {
                    return Err(DriverError::new(
                        "test.assert.url",
                        format!("expected URL '{expected}', received '{actual}'"),
                    ));
                }
                Ok(StepOutput::new("URL matched").with_data(data))
            }
            Expectation::Visible(target) => {
                let selector = direct_selector(target)?;
                let data = self
                    .execute_command(vec![
                        "is".into(),
                        "visible".into(),
                        OsString::from(selector),
                    ])
                    .await?;
                if scalar_bool(&data) == Some(false) {
                    return Err(DriverError::new(
                        "test.assert.visible",
                        "target is not visible",
                    ));
                }
                Ok(StepOutput::new("target is visible").with_data(data))
            }
        }
    }

    async fn screenshot(&self, requested: &str) -> Result<StepOutput, DriverError> {
        let path = resolve_artifact_path(&self.artifacts_dir, requested)?;
        let parent = path.parent().ok_or_else(|| {
            DriverError::new(
                "test.driver.web.artifact_path_invalid",
                "screenshot path has no parent directory",
            )
        })?;
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_create_failed",
                format!("failed to create artifact directory: {error}"),
            )
        })?;

        let data = self
            .execute_command(vec!["screenshot".into(), path.as_os_str().to_os_string()])
            .await?;
        Ok(StepOutput::new("screenshot captured")
            .with_data(data)
            .with_evidence(Evidence {
                name: requested.to_string(),
                path: path.display().to_string(),
                media_type: "image/png".to_string(),
            }))
    }

    async fn execute_command(&self, action_args: Vec<OsString>) -> Result<Value, DriverError> {
        tokio::fs::create_dir_all(&self.runtime_dir)
            .await
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.runtime_create_failed",
                    format!("failed to create browser runtime directory: {error}"),
                )
            })?;
        let invocation = invocation(
            &self.config,
            &self.namespace,
            &self.session,
            &self.runtime_dir,
            action_args,
        );
        let output =
            self.executor.run(invocation).await.map_err(|message| {
                DriverError::new("test.driver.web.command_unavailable", message)
            })?;

        if output.exit_code != 0 {
            let detail = if output.stderr.trim().is_empty() {
                output.stdout.trim()
            } else {
                output.stderr.trim()
            };
            return Err(DriverError::new(
                "test.driver.web.command_failed",
                bounded(detail, 2_048),
            ));
        }

        let trimmed = output.stdout.trim();
        if trimmed.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_string())))
    }

    async fn emergency_cleanup(&self) -> bool {
        let runtime_dir = self.runtime_dir.clone();
        let namespace = self.namespace.clone();
        let session = self.session.clone();
        let markers = self.config.command.process_markers();
        tokio::task::spawn_blocking(move || {
            terminate_owned_session(&runtime_dir, &namespace, &session, &markers)
        })
        .await
        .unwrap_or(false)
    }

    fn emergency_cleanup_sync(&self) -> bool {
        terminate_owned_session(
            &self.runtime_dir,
            &self.namespace,
            &self.session,
            &self.config.command.process_markers(),
        )
    }
}

impl Drop for AgentBrowserSession {
    fn drop(&mut self) {
        if self.closed {
            return;
        }

        if self.emergency_cleanup_sync() {
            return;
        }

        let invocation = invocation(
            &self.config,
            &self.namespace,
            &self.session,
            &self.runtime_dir,
            vec![OsString::from("close")],
        );
        let executor = Arc::clone(&self.executor);
        let runtime_guard = self.runtime_guard.take();
        let registration = self.registration.take();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _runtime_guard = runtime_guard;
                let _registration = registration;
                let _ = executor.run(invocation).await;
            });
        }
    }
}

fn absolute_artifacts_dir(path: &std::path::Path) -> Result<PathBuf, DriverError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.working_directory_failed",
                format!("failed to resolve current directory: {error}"),
            )
        })
}
