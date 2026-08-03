use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use a3s_test_core::{
    Action, CaptureOperation, DriverError, DriverSession, Evidence, Expectation, ScenarioContext,
    StepOutput, Surface, SurfaceDriver, SurfaceObservation, Target, TestStep, VideoOperation,
};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::actions::{
    dialog_args, frame_args, network_route_args, network_unroute_args, select_args, tab_args,
    upload_args, viewport_args,
};
use crate::artifact::{
    admit_artifact_path, prepare_artifact_path, prepare_artifact_root, validate_artifact_file,
};
use crate::capabilities;
use crate::process::{create_runtime_directory, terminate_owned_session, SessionRegistration};
use crate::protocol::{
    bounded, compact_component, direct_selector, invocation, scalar_bool, scalar_string,
    target_action, validate_component, wait_args,
};
use crate::runtime::RuntimeDirectory;
use crate::{AgentBrowserConfig, BrowserCapabilities, CommandExecutor, TokioCommandExecutor};

mod advanced;

const SESSION_FRESH: u8 = 0;
const SESSION_ACTIVE: u8 = 1;
const SESSION_START_FAILED: u8 = 2;

#[derive(Clone, Debug)]
pub struct AgentBrowserConnectionConfig {
    pub namespace: String,
    pub session: String,
    pub runtime_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub active_video_path: Option<String>,
}

pub struct AgentBrowserDriver {
    config: AgentBrowserConfig,
    executor: Arc<dyn CommandExecutor>,
    capabilities: OnceCell<BrowserCapabilities>,
}

impl AgentBrowserDriver {
    #[must_use]
    pub fn new(config: AgentBrowserConfig) -> Self {
        Self::with_executor(config, Arc::new(TokioCommandExecutor))
    }

    #[must_use]
    pub fn with_executor(config: AgentBrowserConfig, executor: Arc<dyn CommandExecutor>) -> Self {
        Self {
            config,
            executor,
            capabilities: OnceCell::new(),
        }
    }

    pub async fn capabilities(&self) -> Result<BrowserCapabilities, DriverError> {
        self.config.validate()?;
        self.capabilities
            .get_or_try_init(|| capabilities::discover(&self.config, self.executor.as_ref()))
            .await
            .cloned()
    }

    /// Connect to a browser session whose lifecycle spans multiple CLI
    /// invocations.
    ///
    /// Unlike [`SurfaceDriver::open`], dropping this handle does not close the
    /// browser session. The owning application must call
    /// [`AgentBrowserSession::close_surface`] when the interactive run ends.
    pub async fn connect(
        &self,
        connection: AgentBrowserConnectionConfig,
    ) -> Result<AgentBrowserSession, DriverError> {
        self.config.validate()?;
        validate_component(&connection.namespace, "namespace")?;
        validate_component(&connection.session, "session id")?;
        let runtime = RuntimeDirectory::bind_or_create(&connection.runtime_dir).await?;
        let artifacts_dir = prepare_artifact_root(&connection.artifacts_dir).await?;
        let active_video = match connection.active_video_path {
            Some(requested) => {
                let path = admit_artifact_path(&artifacts_dir, &requested).await?;
                Some(ActiveVideo { requested, path })
            }
            None => None,
        };
        self.capabilities().await?;

        Ok(AgentBrowserSession {
            config: self.config.clone(),
            namespace: connection.namespace,
            session: connection.session,
            runtime,
            runtime_guard: None,
            registration: None,
            artifacts_dir,
            executor: Arc::clone(&self.executor),
            active_video,
            close_on_drop: false,
            closed: false,
            lifecycle: AtomicU8::new(SESSION_ACTIVE),
        })
    }
}

#[async_trait]
impl SurfaceDriver for AgentBrowserDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError> {
        self.config.validate()?;
        self.capabilities().await?;
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
        let artifacts_dir = prepare_artifact_root(&context.artifacts_dir).await?;
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
        let runtime = RuntimeDirectory::bind_existing(&runtime_dir).await?;
        let registration = SessionRegistration::new(
            runtime.clone(),
            namespace.clone(),
            session.clone(),
            self.config.command.process_markers(),
        );

        Ok(Box::new(AgentBrowserSession {
            config: self.config.clone(),
            namespace,
            session,
            runtime,
            runtime_guard: Some(runtime_guard),
            registration: Some(registration),
            artifacts_dir,
            executor: Arc::clone(&self.executor),
            active_video: None,
            close_on_drop: true,
            closed: false,
            lifecycle: AtomicU8::new(SESSION_FRESH),
        }))
    }
}

pub struct AgentBrowserSession {
    config: AgentBrowserConfig,
    namespace: String,
    session: String,
    artifacts_dir: PathBuf,
    runtime: RuntimeDirectory,
    runtime_guard: Option<tempfile::TempDir>,
    registration: Option<SessionRegistration>,
    executor: Arc<dyn CommandExecutor>,
    active_video: Option<ActiveVideo>,
    close_on_drop: bool,
    closed: bool,
    lifecycle: AtomicU8,
}

struct ActiveVideo {
    requested: String,
    path: PathBuf,
}

impl AgentBrowserSession {
    pub async fn observe_surface(&mut self) -> Result<SurfaceObservation, DriverError> {
        <Self as DriverSession>::observe(self).await
    }

    pub async fn execute_action(
        &mut self,
        id: impl Into<String>,
        action: Action,
    ) -> Result<StepOutput, DriverError> {
        <Self as DriverSession>::execute(
            self,
            &TestStep {
                id: id.into(),
                action,
            },
        )
        .await
    }

    pub async fn close_surface(&mut self) -> Result<(), DriverError> {
        <Self as DriverSession>::close(self).await
    }

    #[must_use]
    pub fn active_video_path(&self) -> Option<&str> {
        self.active_video
            .as_ref()
            .map(|active| active.requested.as_str())
    }
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
            Action::Hover { target } => {
                let args = target_action(target, "hover", None)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("target hovered").with_data(data))
            }
            Action::Focus { target } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["focus".into(), selector.into()])
                    .await
                    .map(|data| StepOutput::new("target focused").with_data(data))
            }
            Action::DoubleClick { target } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["dblclick".into(), selector.into()])
                    .await
                    .map(|data| StepOutput::new("target double-clicked").with_data(data))
            }
            Action::ContextClick { target } => self.context_click(target).await,
            Action::Fill { target, value } => {
                let args = target_action(target, "fill", Some(value))?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("target filled").with_data(data))
            }
            Action::Type { target, value } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["type".into(), selector.into(), value.into()])
                    .await
                    .map(|data| StepOutput::new("text typed into target").with_data(data))
            }
            Action::Check { target } => {
                let args = target_action(target, "check", None)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("target checked").with_data(data))
            }
            Action::Uncheck { target } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["uncheck".into(), selector.into()])
                    .await
                    .map(|data| StepOutput::new("target unchecked").with_data(data))
            }
            Action::Select { target, values } => self
                .execute_command(select_args(target, values)?)
                .await
                .map(|data| StepOutput::new("target options selected").with_data(data)),
            Action::Drag { source, target } => self.drag(source, target).await,
            Action::Press { key } => self
                .execute_command(vec!["press".into(), key.into()])
                .await
                .map(|data| StepOutput::new("key pressed").with_data(data)),
            Action::Wheel {
                target,
                delta_x,
                delta_y,
                modifiers,
            } => {
                self.wheel(target.as_ref(), *delta_x, *delta_y, modifiers.as_slice())
                    .await
            }
            Action::Viewport {
                width,
                height,
                scale,
            } => {
                if *width == 0 || *height == 0 || scale == &Some(0) {
                    return Err(DriverError::new(
                        "test.driver.web.viewport_invalid",
                        "viewport width, height, and optional scale must be greater than zero",
                    ));
                }
                self.execute_command(viewport_args(*width, *height, *scale))
                    .await
                    .map(|data| StepOutput::new("viewport updated").with_data(data))
            }
            Action::Wait { condition } => {
                let args = wait_args(condition)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("wait condition satisfied").with_data(data))
            }
            Action::Assert { expectation } => self.assert(expectation).await,
            Action::Screenshot { path } => self.screenshot(path).await,
            Action::Tab { operation } => self
                .execute_command(tab_args(operation))
                .await
                .map(|data| StepOutput::new("tab operation completed").with_data(data)),
            Action::Frame { target } => self
                .execute_command(frame_args(target))
                .await
                .map(|data| StepOutput::new("frame context changed").with_data(data)),
            Action::Dialog { operation } => self
                .execute_command(dialog_args(operation))
                .await
                .map(|data| StepOutput::new("dialog operation completed").with_data(data)),
            Action::Upload { target, paths } => {
                let args = upload_args(target, paths)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("files uploaded").with_data(data))
            }
            Action::Download { target, path } => self.download(target, path).await,
            Action::NetworkRoute { pattern, route } => self
                .execute_command(network_route_args(pattern, route))
                .await
                .map(|data| StepOutput::new("network route installed").with_data(data)),
            Action::NetworkUnroute { pattern } => self
                .execute_command(network_unroute_args(pattern.as_deref()))
                .await
                .map(|data| StepOutput::new("network route removed").with_data(data)),
            Action::Har { operation } => self.har(operation).await,
            Action::Trace { operation } => self.trace(operation).await,
            Action::Video { operation } => self.video(operation).await,
            Action::Accessibility { path, interactive } => {
                let mut args = vec![OsString::from("snapshot")];
                if *interactive {
                    args.push(OsString::from("-i"));
                }
                self.capture_json(args, path, "accessibility snapshot captured")
                    .await
            }
            Action::Console { path, clear } => {
                let mut args = vec![OsString::from("console")];
                if *clear {
                    args.push(OsString::from("--clear"));
                }
                self.capture_json(args, path, "browser console captured")
                    .await
            }
            Action::PageErrors { path, clear } => {
                let mut args = vec![OsString::from("errors")];
                if *clear {
                    args.push(OsString::from("--clear"));
                }
                self.capture_json(args, path, "page errors captured").await
            }
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        if self.closed {
            return Ok(());
        }

        if self.lifecycle.load(Ordering::Acquire) == SESSION_START_FAILED {
            let _ = self.emergency_cleanup().await;
            self.closed = true;
            return Ok(());
        }

        if self.active_video.is_some() {
            let _ = self
                .execute_command(vec![OsString::from("record"), OsString::from("stop")])
                .await;
            self.active_video = None;
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
        let path = self.prepare_artifact(requested).await?;
        let data = self
            .execute_command(vec!["screenshot".into(), path.as_os_str().to_os_string()])
            .await?;
        validate_artifact_file(&self.artifacts_dir, &path).await?;
        Ok(StepOutput::new("screenshot captured")
            .with_data(data)
            .with_evidence(evidence(requested, &path, media_type_for_path(&path))))
    }

    async fn download(&self, target: &Target, requested: &str) -> Result<StepOutput, DriverError> {
        let selector = direct_selector(target)?;
        let path = self.prepare_artifact(requested).await?;
        let data = self
            .execute_command(vec![
                OsString::from("download"),
                OsString::from(selector),
                path.as_os_str().to_os_string(),
            ])
            .await?;
        validate_artifact_file(&self.artifacts_dir, &path).await?;
        Ok(StepOutput::new("file downloaded")
            .with_data(data)
            .with_evidence(evidence(requested, &path, media_type_for_path(&path))))
    }

    async fn har(&self, operation: &CaptureOperation) -> Result<StepOutput, DriverError> {
        match operation {
            CaptureOperation::Start => self
                .execute_command(vec!["network".into(), "har".into(), "start".into()])
                .await
                .map(|data| StepOutput::new("HAR recording started").with_data(data)),
            CaptureOperation::Stop { path: requested } => {
                let path = self.prepare_artifact(requested).await?;
                let data = self
                    .execute_command(vec![
                        "network".into(),
                        "har".into(),
                        "stop".into(),
                        path.as_os_str().to_os_string(),
                    ])
                    .await?;
                validate_artifact_file(&self.artifacts_dir, &path).await?;
                Ok(StepOutput::new("HAR recording saved")
                    .with_data(data)
                    .with_evidence(evidence(requested, &path, "application/json")))
            }
        }
    }

    async fn trace(&self, operation: &CaptureOperation) -> Result<StepOutput, DriverError> {
        match operation {
            CaptureOperation::Start => self
                .execute_command(vec!["trace".into(), "start".into()])
                .await
                .map(|data| StepOutput::new("trace recording started").with_data(data)),
            CaptureOperation::Stop { path: requested } => {
                let path = self.prepare_artifact(requested).await?;
                let data = self
                    .execute_command(vec![
                        "trace".into(),
                        "stop".into(),
                        path.as_os_str().to_os_string(),
                    ])
                    .await?;
                validate_artifact_file(&self.artifacts_dir, &path).await?;
                Ok(StepOutput::new("trace recording saved")
                    .with_data(data)
                    .with_evidence(evidence(requested, &path, "application/zip")))
            }
        }
    }

    async fn video(&mut self, operation: &VideoOperation) -> Result<StepOutput, DriverError> {
        match operation {
            VideoOperation::Start {
                path: requested,
                url,
            } => {
                if self.active_video.is_some() {
                    return Err(DriverError::new(
                        "test.driver.web.video_already_active",
                        "a video recording is already active",
                    ));
                }
                let path = self.prepare_artifact(requested).await?;
                let mut args = vec![
                    OsString::from("record"),
                    OsString::from("start"),
                    path.as_os_str().to_os_string(),
                ];
                if let Some(url) = url {
                    args.push(OsString::from(url));
                }
                let data = self.execute_command(args).await?;
                self.active_video = Some(ActiveVideo {
                    requested: requested.clone(),
                    path,
                });
                Ok(StepOutput::new("video recording started").with_data(data))
            }
            VideoOperation::Stop => {
                let (requested, path) = self
                    .active_video
                    .as_ref()
                    .map(|active| (active.requested.clone(), active.path.clone()))
                    .ok_or_else(|| {
                        DriverError::new(
                            "test.driver.web.video_not_active",
                            "no video recording is active",
                        )
                    })?;
                let data = self
                    .execute_command(vec![OsString::from("record"), OsString::from("stop")])
                    .await?;
                self.active_video = None;
                validate_artifact_file(&self.artifacts_dir, &path).await?;
                Ok(StepOutput::new("video recording saved")
                    .with_data(data)
                    .with_evidence(evidence(&requested, &path, "video/webm")))
            }
        }
    }

    async fn capture_json(
        &self,
        args: Vec<OsString>,
        requested: &str,
        summary: &str,
    ) -> Result<StepOutput, DriverError> {
        let path = self.prepare_artifact(requested).await?;
        let data = self.execute_command(args).await?;
        let bytes = serde_json::to_vec_pretty(&data).map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_serialize_failed",
                format!("failed to serialize browser evidence: {error}"),
            )
        })?;
        tokio::fs::write(&path, bytes).await.map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_write_failed",
                format!("failed to write browser evidence: {error}"),
            )
        })?;
        validate_artifact_file(&self.artifacts_dir, &path).await?;
        Ok(StepOutput::new(summary)
            .with_data(data)
            .with_evidence(evidence(requested, &path, "application/json")))
    }

    async fn prepare_artifact(&self, requested: &str) -> Result<PathBuf, DriverError> {
        prepare_artifact_path(&self.artifacts_dir, requested).await
    }

    async fn execute_command(&self, action_args: Vec<OsString>) -> Result<Value, DriverError> {
        self.runtime.verify().await?;
        let invocation = invocation(
            &self.config,
            &self.namespace,
            &self.session,
            self.runtime.path(),
            action_args,
        );
        let output = match self.executor.run(invocation).await {
            Ok(output) => output,
            Err(error) => {
                self.mark_start_failed();
                let retryable = error.retryable();
                return Err(DriverError::new(
                    "test.driver.web.command_unavailable",
                    error.to_string(),
                )
                .with_retryable(retryable));
            }
        };

        if output.exit_code != 0 {
            self.mark_start_failed();
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

        self.lifecycle.store(SESSION_ACTIVE, Ordering::Release);

        let trimmed = output.stdout.trim();
        if trimmed.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_string())))
    }

    async fn emergency_cleanup(&self) -> bool {
        let runtime = self.runtime.clone();
        let namespace = self.namespace.clone();
        let session = self.session.clone();
        let markers = self.config.command.process_markers();
        tokio::task::spawn_blocking(move || {
            terminate_owned_session(&runtime, &namespace, &session, &markers)
        })
        .await
        .unwrap_or(false)
    }

    fn emergency_cleanup_sync(&self) -> bool {
        terminate_owned_session(
            &self.runtime,
            &self.namespace,
            &self.session,
            &self.config.command.process_markers(),
        )
    }

    fn mark_start_failed(&self) {
        let _ = self.lifecycle.compare_exchange(
            SESSION_FRESH,
            SESSION_START_FAILED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

impl Drop for AgentBrowserSession {
    fn drop(&mut self) {
        if self.closed || !self.close_on_drop {
            return;
        }

        if self.lifecycle.load(Ordering::Acquire) == SESSION_START_FAILED {
            let _ = self.emergency_cleanup_sync();
            return;
        }

        if self.emergency_cleanup_sync() {
            return;
        }

        if self.runtime.verify_sync().is_err() {
            return;
        }

        let invocation = invocation(
            &self.config,
            &self.namespace,
            &self.session,
            self.runtime.path(),
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

fn evidence(requested: &str, path: &std::path::Path, media_type: &str) -> Evidence {
    Evidence {
        name: requested.to_string(),
        path: path.display().to_string(),
        media_type: media_type.to_string(),
    }
}

fn media_type_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("pdf") => "application/pdf",
        Some("json" | "har") => "application/json",
        Some("zip") => "application/zip",
        Some("webm") => "video/webm",
        Some("txt" | "log") => "text/plain",
        Some("html") => "text/html",
        _ => "application/octet-stream",
    }
}
