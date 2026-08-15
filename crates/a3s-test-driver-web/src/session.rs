use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_test_core::{
    Action, CaptureOperation, ContractReport, DesignAuditReport, DriverError, DriverSession,
    Evidence, Expectation, GroundingScreenshot, PageContextInspectRequest, PageContextInspectScope,
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
    validate_artifact_file, MAX_GROUNDING_IMAGE_BYTES,
};
use crate::capabilities;
use crate::process::{create_runtime_directory, terminate_owned_session, SessionRegistration};
use crate::protocol::{
    bounded, compact_component, direct_selector, invocation, scalar_bool, scalar_string,
    semantic_target_action_args, target_action, validate_component, visibility_args, wait_args,
};
use crate::runtime::RuntimeDirectory;
use crate::{AgentBrowserConfig, BrowserCapabilities, CommandExecutor, TokioCommandExecutor};

mod advanced;

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
        self.capture_observation(false).await
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.ensure_open()?;

        match &step.action {
            Action::Navigate { url } => self
                .execute_command(vec!["open".into(), url.into()])
                .await
                .map(|data| StepOutput::new("page opened").with_data(data)),
            Action::Snapshot { interactive } => {
                self.capture_observation(*interactive)
                    .await
                    .map(|observation| StepOutput {
                        summary: "page snapshot captured".to_string(),
                        data: observation.data,
                        evidence: observation.evidence,
                        page_context: observation.page_context,
                    })
            }
            Action::Click { target } => {
                self.execute_target_action(target, "click", None)
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
                self.execute_target_action(target, "fill", Some(value))
                    .await
                    .map(|data| StepOutput::new("target filled").with_data(data))
            }
            Action::Type { target, value } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["type".into(), selector.into(), value.into()])
                    .await
                    .map(|data| StepOutput::new("text typed into target").with_data(data))
            }
            Action::InsertText { value } => self
                .execute_command(vec!["keyboard".into(), "inserttext".into(), value.into()])
                .await
                .map(|data| StepOutput::new("text inserted at current focus").with_data(data)),
            Action::Check { target } => {
                self.execute_target_action(target, "check", None)
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
            Action::TerminalPaste { .. }
            | Action::TerminalResize { .. }
            | Action::TerminalRecording { .. } => Err(DriverError::new(
                "test.driver.web.action_unsupported",
                "terminal actions are available only on terminal surfaces",
            )),
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
            Action::VerifyContract { .. } => Err(DriverError::new(
                "test.driver.web.runner_action_unsupported",
                "verify_contract is executed by the A3S Test runner and must not reach a surface driver",
            )),
        }
    }

    async fn take_repairs(&mut self, limit: usize) -> Result<Vec<RepairFinding>, DriverError> {
        self.ensure_open()?;
        let bounded = limit.clamp(1, 50);
        let script = TAKE_REPAIRS_SCRIPT.replace("(50)", &format!("({bounded})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        serde_json::from_value(browser_result(value)).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_queue_invalid",
                format!("Test Kit repair queue returned invalid findings: {error}"),
            )
        })
    }

    async fn wait_for_repairs(
        &mut self,
        limit: usize,
        timeout_ms: u64,
        batch_window_ms: u64,
    ) -> Result<Vec<RepairFinding>, DriverError> {
        self.ensure_open()?;
        let bounded_limit = limit.clamp(1, 50);
        let command_budget_ms = u64::try_from(self.config.command_timeout.as_millis())
            .unwrap_or(u64::MAX)
            .saturating_sub(100)
            .max(1);
        let bounded_timeout = timeout_ms.min(300_000).min(command_budget_ms);
        let bounded_window = batch_window_ms.min(5_000).min(bounded_timeout);
        let script = WAIT_REPAIRS_SCRIPT
            .replace("limit: 50", &format!("limit: {bounded_limit}"))
            .replace("timeoutMs: 0", &format!("timeoutMs: {bounded_timeout}"))
            .replace(
                "batchWindowMs: 0",
                &format!("batchWindowMs: {bounded_window}"),
            );
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        serde_json::from_value(browser_result(value)).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_queue_invalid",
                format!("Test Kit repair watch returned invalid findings: {error}"),
            )
        })
    }

    async fn apply_repair_event(&mut self, event: &RepairStatusEvent) -> Result<(), DriverError> {
        self.ensure_open()?;
        let event = serde_json::to_string(event).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_event_invalid",
                format!("failed to encode repair status event: {error}"),
            )
        })?;
        let script = format!(
            "(() => {{ const bridge = window[Symbol.for(\"a3s.test.page-context\")]; return bridge?.applyRepairEvent?.({event}) ?? null; }})()"
        );
        self.execute_command(vec!["eval".into(), script.into()])
            .await
            .map(drop)
    }

    async fn project_quality_report(
        &mut self,
        report: &ContractReport,
    ) -> Result<bool, DriverError> {
        self.ensure_open()?;
        let report = serde_json::to_string(report).map_err(|error| {
            DriverError::new(
                "test.driver.web.quality_report_invalid",
                format!("failed to encode the quality report: {error}"),
            )
        })?;
        let script = REPORT_QUALITY_SCRIPT.replace("(null)", &format!("({report})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        Ok(browser_result(value).as_bool().unwrap_or(false))
    }

    async fn project_design_audit_report(
        &mut self,
        report: &DesignAuditReport,
    ) -> Result<bool, DriverError> {
        self.ensure_open()?;
        let report = serde_json::to_string(report).map_err(|error| {
            DriverError::new(
                "test.driver.web.design_audit_report_invalid",
                format!("failed to encode the design-audit report: {error}"),
            )
        })?;
        let script = REPORT_DESIGN_AUDIT_SCRIPT.replace("(null)", &format!("({report})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        Ok(browser_result(value).as_bool().unwrap_or(false))
    }

    async fn take_repair_actions(
        &mut self,
        limit: usize,
    ) -> Result<Vec<RepairHumanAction>, DriverError> {
        self.ensure_open()?;
        let bounded = limit.clamp(1, 50);
        let script = TAKE_REPAIR_ACTIONS_SCRIPT.replace("(50)", &format!("({bounded})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        serde_json::from_value(browser_result(value)).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_action_invalid",
                format!("Test Kit returned invalid human repair actions: {error}"),
            )
        })
    }

    async fn capture_repair_evidence(
        &mut self,
        request: &RepairEvidenceRequest,
    ) -> Result<RepairEvidenceBundle, DriverError> {
        self.ensure_open()?;
        let context = self.capture_page_context().await?;
        let snapshot = context.snapshot.ok_or_else(|| {
            DriverError::new(
                "test.driver.web.repair_evidence_context_missing",
                "repair evidence requires a compatible Test Kit context",
            )
        })?;
        let context_revision = snapshot.revision.ok_or_else(|| {
            DriverError::new(
                "test.driver.web.repair_evidence_context_invalid",
                "repair evidence context is missing its revision",
            )
        })?;
        let context_bytes = serde_json::to_vec(&snapshot).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_evidence_invalid",
                format!("failed to encode repair page context: {error}"),
            )
        })?;
        let phase = match request.phase {
            RepairEvidencePhase::Before => "before",
            RepairEvidencePhase::After => "after",
        };
        let attempt = request.attempt_id.as_deref().unwrap_or("submitted");
        validate_component(&request.finding_id, "finding id")?;
        validate_component(attempt, "attempt id")?;
        let requested = format!("repairs/{}/{attempt}/{phase}.png", request.finding_id);
        let screenshot_output = self.screenshot(&requested).await?;
        let screenshot = screenshot_output
            .evidence
            .into_iter()
            .next()
            .ok_or_else(|| {
                DriverError::new(
                    "test.driver.web.repair_evidence_invalid",
                    "repair screenshot did not produce evidence metadata",
                )
            })?;
        let screenshot_path = PathBuf::from(&screenshot.path);
        let screenshot_bytes = tokio::fs::read(&screenshot_path).await.map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_evidence_invalid",
                format!("failed to read repair screenshot: {error}"),
            )
        })?;
        Ok(RepairEvidenceBundle {
            captured_at_ms: unix_ms(),
            context_revision,
            context_sha256: format!("{:x}", Sha256::digest(context_bytes)),
            context: snapshot,
            console_errors: self.page_console_error_count().await?,
            page_errors: self.page_error_count().await?,
            screenshot,
            screenshot_sha256: format!("{:x}", Sha256::digest(screenshot_bytes)),
        })
    }

    async fn prove_repair_acl(
        &mut self,
        finding_id: &str,
        attempt_id: &str,
        finding_url: &str,
        candidate: &str,
    ) -> Result<RepairAclProof, DriverError> {
        self.ensure_open()?;
        validate_component(finding_id, "finding id")?;
        validate_component(attempt_id, "attempt id")?;
        let suite = TestSuite::from_repair_acl(candidate, finding_url).map_err(|error| {
            DriverError::new(
                "test.driver.repair_acl_invalid",
                format!("repair ACL candidate is invalid: {}", error.message()),
            )
        })?;
        let requested = format!("repairs/{finding_id}/{attempt_id}/regression.acl");
        let path = prepare_artifact_path(&self.artifacts_dir, &requested).await?;
        tokio::fs::write(&path, candidate).await.map_err(|error| {
            DriverError::new(
                "test.driver.repair_acl_write_failed",
                format!("failed to persist repair ACL candidate: {error}"),
            )
        })?;
        validate_artifact_file(&self.artifacts_dir, &path).await?;

        let proof_context = ScenarioContext {
            run_id: format!("repair-proof-{}", compact_component(attempt_id, 24)),
            scenario_id: format!("proof-{}", compact_component(finding_id, 24)),
            artifacts_dir: self
                .artifacts_dir
                .join("repairs")
                .join(finding_id)
                .join(attempt_id)
                .join("proof"),
        };
        let driver =
            AgentBrowserDriver::with_executor(self.config.clone(), Arc::clone(&self.executor));
        let mut proof_session = driver.open(&proof_context).await?;
        let scenario = suite.scenarios.first().ok_or_else(|| {
            DriverError::new(
                "test.driver.repair_acl_invalid",
                "repair ACL candidate has no scenario",
            )
        })?;
        let mut failure = None;
        for step in &scenario.steps {
            if let Err(error) = proof_session.execute(step).await {
                failure = Some(format!(
                    "step '{}' failed with {}: {}",
                    step.id,
                    error.code(),
                    error.message()
                ));
                break;
            }
        }
        if let Err(error) = proof_session.close().await {
            let cleanup = format!("fresh proof browser cleanup failed: {}", error.message());
            failure =
                Some(failure.map_or(cleanup.clone(), |existing| format!("{existing}; {cleanup}")));
        }
        let passed = failure.is_none();
        Ok(RepairAclProof {
            path: requested,
            passed,
            summary: failure.unwrap_or_else(|| {
                "ACL candidate passed in a fresh browser session with the owning network policy"
                    .to_string()
            }),
        })
    }

    async fn validate_page_context_revision(
        &mut self,
        expected_revision: u64,
    ) -> Result<(), DriverError> {
        self.ensure_open()?;
        let current = self.capture_page_context().await?;
        if !current.present {
            return Err(DriverError::new(
                "test.driver.web.page_context_lost",
                "the Test Kit page context bridge is no longer present",
            ));
        }
        if current.revision != Some(expected_revision) {
            return Err(DriverError::new(
                "test.driver.web.page_context_stale",
                format!(
                    "page context revision changed from {expected_revision} to {}",
                    current
                        .revision
                        .map_or_else(|| "unknown".to_string(), |value| value.to_string())
                ),
            ));
        }
        Ok(())
    }

    async fn inspect_page_context(
        &mut self,
        request: &PageContextInspectRequest,
    ) -> Result<PageContextObservation, DriverError> {
        self.ensure_open()?;
        let scope = match &request.scope {
            PageContextInspectScope::Page => serde_json::json!({ "kind": "page" }),
            PageContextInspectScope::Node(node_id) => {
                serde_json::json!({ "kind": "node", "nodeId": node_id })
            }
            PageContextInspectScope::Component(component_id) => {
                serde_json::json!({ "kind": "component", "componentId": component_id })
            }
            PageContextInspectScope::Region {
                space,
                x,
                y,
                width,
                height,
            } => serde_json::json!({
                "kind": "region",
                "space": space,
                "x": x,
                "y": y,
                "width": width,
                "height": height,
            }),
        };
        let request = serde_json::json!({
            "detail": request.detail,
            "scope": scope,
            "cursor": request.cursor,
            "limits": { "nodes": request.limit.clamp(1, 5_000) },
        });
        let script = format!(
            "(() => {{ const bridge = window[Symbol.for(\"a3s.test.page-context\")]; if (!bridge || typeof bridge.probe !== \"function\" || typeof bridge.snapshot !== \"function\") return {{ present: false }}; const probe = bridge.probe(); if (probe?.protocol !== \"a3s.test.page-context/1\") return {{ present: false }}; return {{ present: true, ...bridge.snapshot({request}) }}; }})()"
        );
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        parse_page_context_value(browser_result(value))
    }

    async fn page_console_error_count(&mut self) -> Result<u32, DriverError> {
        self.ensure_open()?;
        let value = self.execute_command(vec!["console".into()]).await?;
        Ok(count_error_entries(&browser_result(value)))
    }

    async fn page_error_count(&mut self) -> Result<u32, DriverError> {
        self.ensure_open()?;
        let value = self.execute_command(vec!["errors".into()]).await?;
        Ok(count_collection_entries(&browser_result(value)))
    }

    async fn capture_grounding_screenshot(
        &mut self,
        requested_path: &str,
        expected_surface_revision: Option<u64>,
    ) -> Result<GroundingScreenshot, DriverError> {
        self.ensure_open()?;
        if PathBuf::from(requested_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("png"))
        {
            return Err(DriverError::new(
                "test.driver.web.grounding_screenshot_invalid",
                "grounding screenshots must use a .png artifact path",
            ));
        }
        let before = self.capture_page_context().await?;
        validate_grounding_revision(&before, expected_surface_revision)?;
        let screenshot_output = self.screenshot(requested_path).await?;
        let after = self.capture_page_context().await?;
        validate_grounding_revision(&after, expected_surface_revision)?;
        if !stable_page_context(&before, &after) {
            return Err(DriverError::new(
                "test.driver.web.page_context_changed",
                "page context changed while the grounding screenshot was captured",
            ));
        }
        let evidence = screenshot_output
            .evidence
            .into_iter()
            .next()
            .ok_or_else(|| {
                DriverError::new(
                    "test.driver.web.grounding_screenshot_invalid",
                    "grounding screenshot did not produce evidence metadata",
                )
            })?;
        let bytes = read_bounded_artifact(
            &self.artifacts_dir,
            Path::new(&evidence.path),
            MAX_GROUNDING_IMAGE_BYTES,
        )
        .await?;
        let (width, height) = png_dimensions(&bytes)?;
        Ok(GroundingScreenshot {
            evidence,
            sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
            width,
            height,
            surface_revision: after.revision,
        })
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        if self.closed {
            return Ok(());
        }

        if self.active_video.is_some() {
            let _ = self
                .execute_command(vec![OsString::from("record"), OsString::from("stop")])
                .await;
            self.active_video = None;
        }

        let graceful = self.execute_command(vec![OsString::from("close")]).await;
        let containment = self.terminate_registered_processes().await;
        let emergency_terminated = self.emergency_cleanup().await;
        let contained = containment?;
        match graceful {
            Ok(_) => {
                self.closed = true;
                Ok(())
            }
            Err(_) if contained || emergency_terminated => {
                self.closed = true;
                Ok(())
            }
            Err(error) => Err(error),
        }
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
                let data = self.execute_command(visibility_args(target)?).await?;
                match scalar_bool(&browser_result(data.clone())).or_else(|| scalar_bool(&data)) {
                    Some(true) => Ok(StepOutput::new("target is visible").with_data(data)),
                    Some(false) => Err(DriverError::new(
                        "test.assert.visible",
                        "target is not visible",
                    )),
                    None => Err(DriverError::new(
                        "test.driver.web.output_invalid",
                        "browser visibility response did not contain a boolean",
                    )),
                }
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

    async fn execute_target_action(
        &self,
        target: &Target,
        action: &str,
        value: Option<&str>,
    ) -> Result<Value, DriverError> {
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
