use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_test_core::{
    Action, CaptureOperation, ContractReport, DesignAuditReport, DriverError, DriverSession,
    Evidence, GroundingScreenshot, PageContextInspectRequest, PageContextInspectScope,
    PageContextObservation, PageContextSnapshot, RepairAclProof, RepairEvidenceBundle,
    RepairEvidencePhase, RepairEvidenceRequest, RepairFinding, RepairHumanAction,
    RepairStatusEvent, ScenarioContext, StepOutput, Surface, SurfaceDriver, SurfaceObservation,
    Target, TestStep, TestSuite, VideoOperation, WaitCondition, PAGE_CONTEXT_PROTOCOL,
};
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;

use crate::actions::{
    dialog_args, frame_args, network_route_args, network_unroute_args, select_args, tab_args,
    upload_args, viewport_args,
};
use crate::artifact::{
    admit_artifact_path, prepare_artifact_path, prepare_artifact_root, read_bounded_artifact,
    validate_artifact_file, MAX_GROUNDING_IMAGE_BYTES, MAX_SCREENSHOT_BYTES,
};
use crate::capabilities;
use crate::process::{create_runtime_directory, terminate_owned_session, SessionRegistration};
use crate::protocol::{
    bounded, compact_component, direct_selector, invocation, semantic_target_action_args,
    target_action, validate_component, wait_args,
};
use crate::repair_reference::materialize_design_references;
use crate::runtime::RuntimeDirectory;
use crate::testkit::{parse_testkit_handshake, testkit_handshake_script, TestKitHandshake};
use crate::{AgentBrowserConfig, BrowserCapabilities, CommandExecutor, TokioCommandExecutor};

mod advanced;
mod assertion;
mod driver;

const SESSION_FRESH: u8 = 0;
const SESSION_ACTIVE: u8 = 1;
const SESSION_START_FAILED: u8 = 2;
const PAGE_CONTEXT_SCRIPT: &str = r#"(() => {
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!bridge || typeof bridge.probe !== "function" || typeof bridge.snapshot !== "function") {
    return { present: false };
  }
  const probe = bridge.probe();
  if (probe?.protocol !== "a3s.test.page-context/1") {
    return { present: false };
  }
  const snapshot = bridge.snapshot({ detail: "summary" });
  return { present: true, ...snapshot };
})()"#;
const TAKE_REPAIRS_SCRIPT: &str = r#"((limit) => {
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!bridge || typeof bridge.takeRepairBatch !== "function") return [];
  return bridge.takeRepairBatch(limit);
})(50)"#;
const TAKE_REPAIR_ACTIONS_SCRIPT: &str = r#"((limit) => {
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!bridge || typeof bridge.takeRepairActions !== "function") return [];
  return bridge.takeRepairActions(limit);
})(50)"#;
const WAIT_REPAIRS_SCRIPT: &str = r#"(async ({ limit, timeoutMs, batchWindowMs }) => {
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!bridge || typeof bridge.takeRepairBatch !== "function") return [];
  const peek = () => typeof bridge.peekRepairBatch === "function"
    ? bridge.peekRepairBatch(limit)
    : bridge.listRepairs?.().filter((repair) => repair.status === "queued").slice(0, limit) ?? [];
  if (peek().length > 0) return bridge.takeRepairBatch(limit);
  if (typeof bridge.subscribe !== "function" || timeoutMs <= 0) return [];
  await new Promise((resolve) => {
    let settled = false;
    let batchTimer;
    const finish = () => {
      if (settled) return;
      settled = true;
      clearTimeout(timeoutTimer);
      if (batchTimer) clearTimeout(batchTimer);
      unsubscribe();
      resolve();
    };
    const timeoutTimer = setTimeout(finish, timeoutMs);
    const unsubscribe = bridge.subscribe((event) => {
      if (event?.type !== "repair.submitted") return;
      if (batchWindowMs === 0) finish();
      else if (!batchTimer) batchTimer = setTimeout(finish, batchWindowMs);
    });
    if (peek().length > 0) finish();
  });
  return bridge.takeRepairBatch(limit);
})({ limit: 50, timeoutMs: 0, batchWindowMs: 0 })"#;
const REPORT_QUALITY_SCRIPT: &str = r#"((report) => {
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!bridge || typeof bridge.reportQuality !== "function") return false;
  return bridge.reportQuality(report) === true;
})(null)"#;
const REPORT_DESIGN_AUDIT_SCRIPT: &str = r#"((report) => {
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!bridge || typeof bridge.reportDesignAudit !== "function") return false;
  return bridge.reportDesignAudit(report) === true;
})(null)"#;

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
        let capabilities = self.capabilities().await?;
        validate_containment_capability(&self.config, &capabilities)?;

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
        let capabilities = self.capabilities().await?;
        validate_containment_capability(&self.config, &capabilities)?;
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
        )
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.process_containment_failed",
                format!("failed to create browser process containment: {error}"),
            )
        })?;

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

fn validate_containment_capability(
    config: &AgentBrowserConfig,
    capabilities: &BrowserCapabilities,
) -> Result<(), DriverError> {
    if !config.network_policy.allowed_origins().is_empty()
        && capabilities.integration == crate::BrowserIntegration::A3s
        && !capabilities
            .features
            .contains(&crate::WebCapability::ExactOriginContainment)
    {
        return Err(DriverError::new(
            "test.driver.web.exact_origin_containment_unavailable",
            "A3S Browser did not report exact-origin containment for the requested network policy",
        ));
    }
    Ok(())
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
        self.capture_observation(false).await
    }

    pub async fn observe_surface_interactive(
        &mut self,
        interactive: bool,
    ) -> Result<SurfaceObservation, DriverError> {
        self.capture_observation(interactive).await
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
                stability: None,
                assertion_mode: Default::default(),
                wait_mode: Default::default(),
            },
        )
        .await
    }

    pub async fn close_surface(&mut self) -> Result<(), DriverError> {
        <Self as DriverSession>::close(self).await
    }

    pub async fn take_repair_batch(
        &mut self,
        limit: usize,
    ) -> Result<Vec<RepairFinding>, DriverError> {
        <Self as DriverSession>::take_repairs(self, limit).await
    }

    pub async fn wait_for_repair_batch(
        &mut self,
        limit: usize,
        timeout_ms: u64,
        batch_window_ms: u64,
    ) -> Result<Vec<RepairFinding>, DriverError> {
        <Self as DriverSession>::wait_for_repairs(self, limit, timeout_ms, batch_window_ms).await
    }

    pub async fn project_repair_event(
        &mut self,
        event: &RepairStatusEvent,
    ) -> Result<(), DriverError> {
        <Self as DriverSession>::apply_repair_event(self, event).await
    }

    pub async fn project_quality_report(
        &mut self,
        report: &ContractReport,
    ) -> Result<bool, DriverError> {
        <Self as DriverSession>::project_quality_report(self, report).await
    }

    pub async fn project_design_audit_report(
        &mut self,
        report: &DesignAuditReport,
    ) -> Result<bool, DriverError> {
        <Self as DriverSession>::project_design_audit_report(self, report).await
    }

    pub async fn take_human_repair_actions(
        &mut self,
        limit: usize,
    ) -> Result<Vec<RepairHumanAction>, DriverError> {
        <Self as DriverSession>::take_repair_actions(self, limit).await
    }

    pub async fn capture_owned_repair_evidence(
        &mut self,
        request: &RepairEvidenceRequest,
    ) -> Result<RepairEvidenceBundle, DriverError> {
        <Self as DriverSession>::capture_repair_evidence(self, request).await
    }

    pub async fn prove_repair_acl_candidate(
        &mut self,
        finding_id: &str,
        attempt_id: &str,
        finding_url: &str,
        candidate: &str,
    ) -> Result<RepairAclProof, DriverError> {
        <Self as DriverSession>::prove_repair_acl(
            self,
            finding_id,
            attempt_id,
            finding_url,
            candidate,
        )
        .await
    }

    pub async fn validate_context_revision(
        &mut self,
        expected_revision: u64,
    ) -> Result<(), DriverError> {
        <Self as DriverSession>::validate_page_context_revision(self, expected_revision).await
    }

    pub async fn inspect_context(
        &mut self,
        request: &PageContextInspectRequest,
    ) -> Result<PageContextObservation, DriverError> {
        <Self as DriverSession>::inspect_page_context(self, request).await
    }

    pub async fn console_error_count(&mut self) -> Result<u32, DriverError> {
        <Self as DriverSession>::page_console_error_count(self).await
    }

    pub async fn page_error_count(&mut self) -> Result<u32, DriverError> {
        <Self as DriverSession>::page_error_count(self).await
    }

    pub async fn testkit_handshake(
        &mut self,
        require_review_overlay: bool,
    ) -> Result<Option<TestKitHandshake>, DriverError> {
        self.ensure_open()?;
        let command_timeout_ms =
            u64::try_from(self.config.command_timeout.as_millis()).unwrap_or(u64::MAX);
        let readiness_timeout_ms = if require_review_overlay {
            command_timeout_ms.saturating_sub(500).min(5_000)
        } else {
            0
        };
        let script = testkit_handshake_script(require_review_overlay, readiness_timeout_ms);
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        parse_testkit_handshake(browser_result(value), require_review_overlay)
    }

    #[must_use]
    pub fn active_video_path(&self) -> Option<&str> {
        self.active_video
            .as_ref()
            .map(|active| active.requested.as_str())
    }
}

impl AgentBrowserSession {
    async fn capture_observation(
        &mut self,
        interactive: bool,
    ) -> Result<SurfaceObservation, DriverError> {
        self.ensure_open()?;
        for attempt in 0..2 {
            let before = self.capture_page_context().await?;
            let mut args = vec![OsString::from("snapshot")];
            if interactive {
                args.push(OsString::from("-i"));
            }
            let data = self.execute_command(args).await?;
            let after = self.capture_page_context().await?;
            if stable_page_context(&before, &after) {
                return Ok(SurfaceObservation::new("browser accessibility snapshot")
                    .with_data(data)
                    .with_page_context(after));
            }
            if attempt == 1 {
                return Err(DriverError::new(
                    "test.driver.web.page_context_changed",
                    "page context changed while the accessibility snapshot was captured",
                ));
            }
        }
        unreachable!("bounded page context capture loop always returns")
    }

    async fn capture_page_context(&self) -> Result<PageContextObservation, DriverError> {
        let value = self
            .execute_command(vec!["eval".into(), PAGE_CONTEXT_SCRIPT.into()])
            .await?;
        parse_page_context_value(browser_result(value))
    }

    fn ensure_open(&self) -> Result<(), DriverError> {
        if self.closed {
            return Err(DriverError::new(
                "test.driver.web.session_closed",
                "browser session is already closed",
            ));
        }
        Ok(())
    }

    async fn screenshot(&self, requested: &str) -> Result<StepOutput, DriverError> {
        let path = self.prepare_artifact(requested).await?;
        let data = self
            .execute_command(vec!["screenshot".into(), path.as_os_str().to_os_string()])
            .await?;
        if let Err(error) =
            read_bounded_artifact(&self.artifacts_dir, &path, MAX_SCREENSHOT_BYTES).await
        {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(error);
        }
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

    async fn execute_target_action(
        &self,
        target: &Target,
        action: &str,
        value: Option<&str>,
    ) -> Result<Value, DriverError> {
        if action == "click" && matches!(target, Target::Ref { .. } | Target::Css { .. }) {
            let selector = direct_selector(target)?;
            self.execute_command(vec!["scrollintoview".into(), selector.into()])
                .await?;
            return self
                .execute_command(target_action(target, action, value)?)
                .await;
        }
        if let Some(args) = semantic_target_action_args(target, action, value)? {
            let mut shadow_result = self.execute_command(args.clone()).await?;
            let mut state = browser_result(shadow_result.clone());
            if state.get("handled").and_then(Value::as_bool) == Some(true) {
                return Ok(shadow_result);
            }
            if let Some((x, y)) = semantic_pointer(&state)? {
                return self.click_at(x, y).await;
            }
            if state.get("matched").and_then(Value::as_bool) == Some(false) {
                self.execute_command(wait_args(&WaitCondition::Visible(target.clone()))?)
                    .await?;
                shadow_result = self.execute_command(args).await?;
                state = browser_result(shadow_result.clone());
                if state.get("handled").and_then(Value::as_bool) == Some(true) {
                    return Ok(shadow_result);
                }
                if let Some((x, y)) = semantic_pointer(&state)? {
                    return self.click_at(x, y).await;
                }
            }
        }
        self.execute_command(target_action(target, action, value)?)
            .await
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

    async fn terminate_registered_processes(&mut self) -> Result<bool, DriverError> {
        let Some(registration) = self.registration.take() else {
            return Ok(false);
        };
        tokio::task::spawn_blocking(move || registration.terminate())
            .await
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.process_cleanup_failed",
                    format!("failed to join browser process cleanup: {error}"),
                )
            })?
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.process_cleanup_failed",
                    format!("failed to terminate browser process tree: {error}"),
                )
            })
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

fn browser_result(value: Value) -> Value {
    value
        .pointer("/data/result")
        .cloned()
        .or_else(|| value.get("result").cloned())
        .unwrap_or(value)
}

fn semantic_pointer(value: &Value) -> Result<Option<(i32, i32)>, DriverError> {
    let Some(pointer) = value.get("pointer") else {
        return Ok(None);
    };
    let coordinate = |name: &str| {
        pointer
            .get(name)
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| {
                DriverError::new(
                    "test.driver.web.box_invalid",
                    format!("semantic target pointer is missing a supported '{name}' coordinate"),
                )
            })
    };
    Ok(Some((coordinate("x")?, coordinate("y")?)))
}

fn parse_page_context_value(mut value: Value) -> Result<PageContextObservation, DriverError> {
    if value.get("present").and_then(Value::as_bool) != Some(true) {
        return Ok(PageContextObservation::absent());
    }
    if let Some(object) = value.as_object_mut() {
        object.remove("present");
    }
    let parsed: PageContextSnapshot = serde_json::from_value(value).map_err(|error| {
        DriverError::new(
            "test.driver.web.page_context_invalid",
            format!("page context bridge returned an invalid bounded snapshot: {error}"),
        )
    })?;
    if parsed.protocol.as_deref() != Some(PAGE_CONTEXT_PROTOCOL) {
        return Err(DriverError::new(
            "test.driver.web.page_context_protocol_unsupported",
            "page context bridge protocol is unsupported",
        ));
    }
    if let Some(ui) = &parsed.ui {
        ui.validate(
            parsed.revision,
            parsed.page.as_ref().map(|page| &page.viewport),
        )
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.ui_understanding_invalid",
                format!("page context bridge returned invalid UI understanding: {error}"),
            )
        })?;
    }
    Ok(PageContextObservation::from_snapshot(parsed))
}

fn count_collection_entries(value: &Value) -> u32 {
    let count = value
        .as_array()
        .map(Vec::len)
        .or_else(|| value.get("entries").and_then(Value::as_array).map(Vec::len))
        .or_else(|| value.get("errors").and_then(Value::as_array).map(Vec::len))
        .unwrap_or(0);
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn count_error_entries(value: &Value) -> u32 {
    let entries = value
        .as_array()
        .or_else(|| value.get("entries").and_then(Value::as_array));
    let count = entries.map_or(0, |entries| {
        entries
            .iter()
            .filter(|entry| {
                entry
                    .get("type")
                    .or_else(|| entry.get("level"))
                    .and_then(Value::as_str)
                    .is_some_and(|level| level.eq_ignore_ascii_case("error"))
            })
            .count()
    });
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn stable_page_context(before: &PageContextObservation, after: &PageContextObservation) -> bool {
    match (before.present, after.present) {
        (false, false) => true,
        (true, true) => before.protocol == after.protocol && before.revision == after.revision,
        _ => false,
    }
}

fn validate_grounding_revision(
    context: &PageContextObservation,
    expected: Option<u64>,
) -> Result<(), DriverError> {
    if expected.is_none() && context.present {
        return Err(DriverError::new(
            "test.driver.web.page_context_revision_unbound",
            "grounding requires the revision from the latest page-context observation",
        ));
    }
    if expected.is_some_and(|expected| context.revision != Some(expected)) {
        return Err(DriverError::new(
            "test.driver.web.page_context_revision_changed",
            "page context revision changed before grounding evidence capture",
        ));
    }
    Ok(())
}

fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), DriverError> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return Err(DriverError::new(
            "test.driver.web.grounding_screenshot_invalid",
            "grounding screenshot is not a valid PNG header",
        ));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("checked PNG width slice"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("checked PNG height slice"));
    if width == 0 || height == 0 {
        return Err(DriverError::new(
            "test.driver.web.grounding_screenshot_invalid",
            "grounding screenshot has zero dimensions",
        ));
    }
    Ok((width, height))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

impl Drop for AgentBrowserSession {
    fn drop(&mut self) {
        if self.closed || !self.close_on_drop {
            return;
        }

        let registration = self.registration.take();
        let contained = registration
            .as_ref()
            .is_some_and(SessionRegistration::has_attached_processes);
        if let Some(registration) = registration {
            let _ = registration.terminate();
        }

        if self.lifecycle.load(Ordering::Acquire) == SESSION_START_FAILED {
            let _ = self.emergency_cleanup_sync();
            return;
        }

        if self.emergency_cleanup_sync() || contained {
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
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _runtime_guard = runtime_guard;
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
