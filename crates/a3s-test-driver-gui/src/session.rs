use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use a3s_test_core::{
    Action, DriverError, DriverSession, Evidence, Expectation, LayoutRect, ScenarioContext,
    StepOutput, Surface, SurfaceDriver, SurfaceObservation, Target, TestStep,
    MAX_LAYOUT_TOLERANCE_PX,
};
use async_trait::async_trait;
use serde_json::{json, Map, Value};

use crate::api::{CuaActionResult, CuaApi};
use crate::artifact::{
    image_evidence, prepare_artifact_root, prepare_png_artifact, validate_grounding_image,
    validate_screenshot,
};
use crate::host::validate_permissions;
use crate::lifecycle::{
    bind_application, bind_window, cleanup_resources, validate_runtime_binding, ApplicationBinding,
    WindowBinding,
};
use crate::semantic::{ElementAddress, SemanticState, VisualAddress};
use crate::{
    CuaClient, CuaTransportFactory, GuiDriverConfig, GuiProfile, StdioCuaTransportFactory,
};

static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub struct GuiDriver {
    config: GuiDriverConfig,
    transport_factory: Arc<dyn CuaTransportFactory>,
}

impl GuiDriver {
    #[must_use]
    pub fn new(config: GuiDriverConfig) -> Self {
        Self::with_transport_factory(config, Arc::new(StdioCuaTransportFactory))
    }

    #[must_use]
    pub fn with_transport_factory(
        config: GuiDriverConfig,
        transport_factory: Arc<dyn CuaTransportFactory>,
    ) -> Self {
        Self {
            config,
            transport_factory,
        }
    }

    pub fn execution_profile(&self) -> Result<crate::GuiExecutionProfile, DriverError> {
        self.config.execution_profile()
    }

    pub async fn probe_host(&self) -> Result<crate::GuiHostProbe, DriverError> {
        self.config.validate()?;
        let config = self.config.clone();
        let transport_factory = Arc::clone(&self.transport_factory);
        tokio::spawn(async move { probe_gui_host(config, transport_factory).await })
            .await
            .map_err(|error| {
                DriverError::new(
                    "test.driver.gui.probe_task_failed",
                    format!("GUI host probe task failed: {error}"),
                )
            })?
    }
}

async fn probe_gui_host(
    config: GuiDriverConfig,
    transport_factory: Arc<dyn CuaTransportFactory>,
) -> Result<crate::GuiHostProbe, DriverError> {
    let transport = transport_factory.connect(&config).await?;
    let client = Arc::new(CuaClient::new(transport));
    let capabilities = match client.admit_locked().await {
        Ok(capabilities) => capabilities,
        Err(error) => {
            let _ = client.close().await;
            return Err(error);
        }
    };
    let api = CuaApi::new(Arc::clone(&client), "host-probe".to_string());
    let permissions = match api.permissions().await {
        Ok(permissions) => permissions,
        Err(error) => {
            let _ = api.close().await;
            return Err(error);
        }
    };
    let permissions = match validate_permissions(&config.endpoint, &permissions) {
        Ok(permissions) => permissions,
        Err(error) => {
            let _ = api.close().await;
            return Err(error);
        }
    };
    api.close().await?;
    Ok(crate::GuiHostProbe {
        driver_version: capabilities.driver_version.to_string(),
        protocol_version: capabilities.protocol_version,
        capability_vocabulary: capabilities.capability_vocabulary,
        tools_schema: capabilities.tools_schema,
        permissions,
    })
}

#[async_trait]
impl SurfaceDriver for GuiDriver {
    fn surface(&self) -> Surface {
        Surface::Gui
    }

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError> {
        self.config.validate()?;
        validate_context_component(&context.run_id, "run id")?;
        validate_context_component(&context.scenario_id, "scenario id")?;
        let artifacts_dir = prepare_artifact_root(&context.artifacts_dir).await?;

        // CUA calls can acquire external ownership before their response arrives. Keep that
        // bounded opening workflow alive after caller cancellation so it can learn the owned PID
        // and route it through the normal session/opening cleanup path.
        let config = self.config.clone();
        let transport_factory = Arc::clone(&self.transport_factory);
        let context = context.clone();
        tokio::spawn(async move {
            open_gui_session(config, transport_factory, context, artifacts_dir).await
        })
        .await
        .map_err(|error| {
            DriverError::new(
                "test.driver.gui.open_task_failed",
                format!("GUI session opening task failed: {error}"),
            )
        })?
    }
}

async fn open_gui_session(
    config: GuiDriverConfig,
    transport_factory: Arc<dyn CuaTransportFactory>,
    context: ScenarioContext,
    artifacts_dir: PathBuf,
) -> Result<Box<dyn DriverSession>, DriverError> {
    let transport = transport_factory.connect(&config).await?;
    let client = Arc::new(CuaClient::new(transport));
    if let Err(error) = client.admit_locked().await {
        let _ = client.close().await;
        return Err(error);
    }
    let api = Arc::new(CuaApi::new(client, session_id(&context)));
    let mut opening = OpeningCleanup::new(Arc::clone(&api));
    if let Err(error) = api.start_window_session().await {
        let _ = opening.cleanup().await;
        return Err(error);
    }
    let permissions = match api.permissions().await {
        Ok(permissions) => permissions,
        Err(error) => {
            let _ = opening.cleanup().await;
            return Err(error);
        }
    };
    if let Err(error) = validate_permissions(&config.endpoint, &permissions) {
        let _ = opening.cleanup().await;
        return Err(error);
    }
    let application = match bind_application(&api, &config.target).await {
        Ok(application) => application,
        Err(error) => {
            let _ = opening.cleanup().await;
            return Err(error);
        }
    };
    opening.bind_application(application.clone());
    let window = match bind_window(&api, &application, &config.window).await {
        Ok(window) => window,
        Err(error) => {
            let _ = opening.cleanup().await;
            return Err(error);
        }
    };
    opening.disarm();

    Ok(Box::new(GuiSession {
        api,
        application,
        window,
        artifacts_dir,
        profile: config.profile,
        capture_sequence: 0,
        semantics: SemanticState::default(),
        closed: false,
    }))
}

struct OpeningCleanup {
    api: Arc<CuaApi>,
    application: Option<ApplicationBinding>,
    armed: bool,
}

impl OpeningCleanup {
    fn new(api: Arc<CuaApi>) -> Self {
        Self {
            api,
            application: None,
            armed: true,
        }
    }

    fn bind_application(&mut self, application: ApplicationBinding) {
        self.application = Some(application);
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    async fn cleanup(mut self) -> Result<(), DriverError> {
        let result = cleanup_resources(Arc::clone(&self.api), self.application.clone()).await;
        self.armed = result.as_ref().is_err_and(|error| error.retryable());
        result
    }
}

impl Drop for OpeningCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        let api = Arc::clone(&self.api);
        let application = self.application.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = cleanup_resources(api, application).await;
            });
        }
    }
}

pub struct GuiSession {
    api: Arc<CuaApi>,
    application: ApplicationBinding,
    window: WindowBinding,
    artifacts_dir: PathBuf,
    profile: GuiProfile,
    capture_sequence: u64,
    semantics: SemanticState,
    closed: bool,
}

#[async_trait]
impl DriverSession for GuiSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        self.ensure_open()?;
        self.capture_observation().await
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.ensure_open()?;
        match &step.action {
            Action::Snapshot { interactive: true } => {
                let observation = self.capture_observation().await?;
                Ok(StepOutput {
                    summary: observation.summary,
                    data: observation.data,
                    evidence: observation.evidence,
                    page_context: observation.page_context,
                })
            }
            Action::Snapshot { interactive: false } => Err(DriverError::new(
                "test.driver.gui.action_unsupported",
                "GUI semantic snapshots currently expose actionable elements only",
            )),
            Action::Click { target } => {
                self.target_action("click", target, "GUI target clicked", Map::new())
                    .await
            }
            Action::DoubleClick { target } => {
                self.target_action(
                    "double_click",
                    target,
                    "GUI target double-clicked",
                    Map::new(),
                )
                .await
            }
            Action::ContextClick { target } => {
                self.target_action(
                    "right_click",
                    target,
                    "GUI target context-clicked",
                    Map::new(),
                )
                .await
            }
            Action::Fill { target, value } => {
                let mut extra = Map::new();
                extra.insert("value".to_string(), Value::String(value.clone()));
                self.target_action("set_value", target, "GUI target filled", extra)
                    .await
            }
            Action::Type { target, value } => {
                let mut extra = Map::new();
                extra.insert("text".to_string(), Value::String(value.clone()));
                self.target_action("type_text", target, "text typed into GUI target", extra)
                    .await
            }
            Action::Drag { source, target } => self.drag(source, target).await,
            Action::Press { key } => self.press_key(key).await,
            Action::Wheel {
                target,
                delta_x,
                delta_y,
                modifiers,
            } => {
                self.wheel(target.as_ref(), *delta_x, *delta_y, modifiers.is_empty())
                    .await
            }
            Action::Assert { expectation } => self.assert(expectation).await,
            Action::Screenshot { path } => self.screenshot(path).await,
            Action::VerifyContract { .. } => Err(DriverError::new(
                "test.driver.gui.runner_action_unsupported",
                "verify_contract is executed by the A3S Test runner and must not reach a surface driver",
            )),
            _ => Err(DriverError::new(
                "test.driver.gui.action_unsupported",
                "this action is not implemented by the GUI semantic profile",
            )),
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        if self.closed {
            return Ok(());
        }
        match cleanup_resources(Arc::clone(&self.api), Some(self.application.clone())).await {
            Ok(()) => {
                self.closed = true;
                Ok(())
            }
            Err(error) => {
                self.closed = !error.retryable();
                Err(error)
            }
        }
    }
}

impl GuiSession {
    fn ensure_open(&self) -> Result<(), DriverError> {
        if self.closed {
            return Err(DriverError::new(
                "test.driver.gui.session_closed",
                "GUI session is already closed",
            ));
        }
        Ok(())
    }

    async fn capture_observation(&mut self) -> Result<SurfaceObservation, DriverError> {
        match self.profile {
            GuiProfile::Semantic => {
                let data = self.refresh(None).await?;
                Ok(SurfaceObservation::new("GUI semantic snapshot captured").with_data(data))
            }
            GuiProfile::WindowVision => {
                self.capture_sequence = self.capture_sequence.checked_add(1).ok_or_else(|| {
                    DriverError::new(
                        "test.driver.gui.snapshot_limit_reached",
                        "GUI visual capture sequence overflowed",
                    )
                })?;
                let requested = format!("observations/gui-{:06}.png", self.capture_sequence);
                let path = prepare_png_artifact(&self.artifacts_dir, &requested).await?;
                let data = self.refresh(Some(&path)).await?;
                Ok(
                    SurfaceObservation::new("GUI semantic and visual snapshot captured")
                        .with_data(data)
                        .with_evidence(image_evidence(&requested, &path)),
                )
            }
        }
    }

    async fn refresh(&mut self, screenshot: Option<&Path>) -> Result<Value, DriverError> {
        self.validate_runtime_binding().await?;
        let state = self
            .api
            .window_state(self.application.pid, self.window.window_id, screenshot)
            .await?;
        let visual_digest = match screenshot {
            Some(path) => Some(validate_screenshot(&self.artifacts_dir, &state, path).await?),
            None => None,
        };
        self.semantics.install(state, visual_digest)?;
        self.semantics.data(&self.application, &self.window)
    }

    async fn ensure_target_snapshot(&mut self, target: &Target) -> Result<(), DriverError> {
        if self.semantics.has_snapshot() {
            return Ok(());
        }
        if matches!(target, Target::Ref { .. }) {
            self.semantics.resolve(target)?;
            return Ok(());
        }
        if let Target::VisualPoint { snapshot, x, y } = target {
            self.semantics.resolve_visual(snapshot, *x, *y)?;
            return Ok(());
        }
        self.refresh(None).await.map(|_| ())
    }

    async fn target_action(
        &mut self,
        tool: &str,
        target: &Target,
        summary: &str,
        extra: Map<String, Value>,
    ) -> Result<StepOutput, DriverError> {
        if let Target::VisualPoint { snapshot, x, y } = target {
            if tool == "set_value" {
                return Err(DriverError::new(
                    "test.driver.gui.visual_action_unsupported",
                    "fill requires a semantic element; use type for a visual point",
                ));
            }
            let address = self.semantics.resolve_visual(snapshot, *x, *y)?;
            validate_grounding_image(&self.artifacts_dir, &address).await?;
            let mut arguments = self.visual_arguments(&address);
            arguments.extend(extra);
            let result = self.dispatch_action(tool, Value::Object(arguments)).await?;
            self.semantics.invalidate();
            return Ok(visual_action_output(summary, &address, &result));
        }
        self.ensure_target_snapshot(target).await?;
        let address = self.semantics.resolve(target)?;
        let mut arguments = self.element_arguments(&address);
        arguments.extend(extra);
        let result = self
            .dispatch_action(tool, Value::Object(arguments))
            .await
            .inspect_err(|error| {
                if error.code() == "test.driver.gui.stale_reference" {
                    self.semantics.invalidate();
                }
            })?;
        self.semantics.invalidate();
        Ok(action_output(summary, Some(&address.reference), &result))
    }

    fn element_arguments(&self, address: &ElementAddress) -> Map<String, Value> {
        let mut arguments = Map::new();
        arguments.insert(
            "session".to_string(),
            Value::String(self.api.session_id().to_string()),
        );
        arguments.insert("pid".to_string(), Value::from(self.application.pid));
        arguments.insert("window_id".to_string(), Value::from(self.window.window_id));
        arguments.insert(
            "element_token".to_string(),
            Value::String(address.token.clone()),
        );
        arguments
    }

    fn visual_arguments(&self, address: &VisualAddress) -> Map<String, Value> {
        let mut arguments = Map::new();
        arguments.insert(
            "session".to_string(),
            Value::String(self.api.session_id().to_string()),
        );
        arguments.insert("pid".to_string(), Value::from(self.application.pid));
        arguments.insert("window_id".to_string(), Value::from(self.window.window_id));
        arguments.insert("x".to_string(), Value::from(address.x));
        arguments.insert("y".to_string(), Value::from(address.y));
        arguments
    }

    async fn press_key(&mut self, key: &str) -> Result<StepOutput, DriverError> {
        if key.trim().is_empty() {
            return Err(DriverError::new(
                "test.driver.gui.key_invalid",
                "GUI key name must not be empty",
            ));
        }
        let result = self
            .dispatch_action(
                "press_key",
                json!({
                    "session": self.api.session_id(),
                    "pid": self.application.pid,
                    "window_id": self.window.window_id,
                    "key": key,
                }),
            )
            .await?;
        self.semantics.invalidate();
        Ok(action_output("GUI key pressed", None, &result))
    }

    async fn wheel(
        &mut self,
        target: Option<&Target>,
        delta_x: i32,
        delta_y: i32,
        modifiers_empty: bool,
    ) -> Result<StepOutput, DriverError> {
        if !modifiers_empty {
            return Err(DriverError::new(
                "test.driver.gui.wheel_unsupported",
                "GUI semantic scrolling does not support modifier keys",
            ));
        }
        let (direction, magnitude) = match (delta_x, delta_y) {
            (0, value) if value > 0 => ("down", value.unsigned_abs()),
            (0, value) if value < 0 => ("up", value.unsigned_abs()),
            (value, 0) if value > 0 => ("right", value.unsigned_abs()),
            (value, 0) if value < 0 => ("left", value.unsigned_abs()),
            _ => {
                return Err(DriverError::new(
                    "test.driver.gui.wheel_invalid",
                    "GUI semantic scrolling requires exactly one non-zero delta axis",
                ));
            }
        };
        let amount = magnitude.div_ceil(100).clamp(1, 50);
        let mut arguments = Map::new();
        arguments.insert(
            "session".to_string(),
            Value::String(self.api.session_id().to_string()),
        );
        arguments.insert("pid".to_string(), Value::from(self.application.pid));
        arguments.insert("window_id".to_string(), Value::from(self.window.window_id));
        arguments.insert(
            "direction".to_string(),
            Value::String(direction.to_string()),
        );
        arguments.insert("by".to_string(), Value::String("line".to_string()));
        arguments.insert("amount".to_string(), Value::from(amount));
        let mut visual = None;
        let reference = if let Some(Target::VisualPoint { snapshot, x, y }) = target {
            let address = self.semantics.resolve_visual(snapshot, *x, *y)?;
            validate_grounding_image(&self.artifacts_dir, &address).await?;
            arguments.insert("x".to_string(), Value::from(address.x));
            arguments.insert("y".to_string(), Value::from(address.y));
            visual = Some(address);
            None
        } else if let Some(target) = target {
            self.ensure_target_snapshot(target).await?;
            let address = self.semantics.resolve(target)?;
            arguments.insert("element_token".to_string(), Value::String(address.token));
            Some(address.reference)
        } else {
            None
        };
        let result = self
            .dispatch_action("scroll", Value::Object(arguments))
            .await?;
        self.semantics.invalidate();
        if let Some(visual) = &visual {
            return Ok(visual_action_output("GUI window scrolled", visual, &result));
        }
        Ok(action_output(
            "GUI window scrolled",
            reference.as_deref(),
            &result,
        ))
    }

    async fn drag(&mut self, source: &Target, target: &Target) -> Result<StepOutput, DriverError> {
        let (
            Target::VisualPoint {
                snapshot: source_snapshot,
                x: source_x,
                y: source_y,
            },
            Target::VisualPoint {
                snapshot: target_snapshot,
                x: target_x,
                y: target_y,
            },
        ) = (source, target)
        else {
            return Err(DriverError::new(
                "test.driver.gui.visual_action_unsupported",
                "GUI drag currently requires two visual_point targets",
            ));
        };
        if source_snapshot != target_snapshot {
            return Err(DriverError::new(
                "test.driver.gui.stale_image",
                "GUI drag endpoints must belong to the same grounding image",
            ));
        }
        let source = self
            .semantics
            .resolve_visual(source_snapshot, *source_x, *source_y)?;
        let target = self
            .semantics
            .resolve_visual(target_snapshot, *target_x, *target_y)?;
        validate_grounding_image(&self.artifacts_dir, &source).await?;
        let result = self
            .dispatch_action(
                "drag",
                json!({
                    "session": self.api.session_id(),
                    "pid": self.application.pid,
                    "window_id": self.window.window_id,
                    "from_x": source.x,
                    "from_y": source.y,
                    "to_x": target.x,
                    "to_y": target.y,
                }),
            )
            .await?;
        self.semantics.invalidate();
        Ok(visual_drag_output(&source, &target, &result))
    }

    async fn validate_runtime_binding(&mut self) -> Result<(), DriverError> {
        let result = validate_runtime_binding(&self.api, &self.application, &self.window).await;
        if result.is_err() {
            self.semantics.invalidate();
        }
        result
    }

    async fn dispatch_action(
        &mut self,
        tool: &str,
        arguments: Value,
    ) -> Result<CuaActionResult, DriverError> {
        self.validate_runtime_binding().await?;
        self.api.action(tool, arguments).await
    }

    async fn assert(&mut self, expectation: &Expectation) -> Result<StepOutput, DriverError> {
        self.refresh(None).await?;
        match expectation {
            Expectation::TextVisible(text) => {
                if !self.semantics.text_visible(text)? {
                    return Err(DriverError::new(
                        "test.assert.text_visible",
                        format!("expected GUI text '{text}' is not visible"),
                    ));
                }
                Ok(StepOutput::new("GUI text is visible")
                    .with_data(json!({ "text": text, "visible": true })))
            }
            Expectation::Visible(target) => {
                let address = match self.semantics.resolve(target) {
                    Ok(address) => address,
                    Err(error)
                        if error.code() == "test.driver.gui.target_not_found"
                            && !matches!(target, Target::Ref { .. }) =>
                    {
                        return Err(DriverError::new(
                            "test.assert.visible",
                            "expected GUI target is not visible",
                        ));
                    }
                    Err(error) => return Err(error),
                };
                Ok(StepOutput::new("GUI target is visible").with_data(json!({
                    "target_ref": address.reference,
                    "visible": true,
                })))
            }
            Expectation::Value { target, value } => {
                let address = self.semantics.resolve(target)?;
                let actual = address.value.ok_or_else(|| {
                    DriverError::new(
                        "test.driver.gui.assertion_unsupported",
                        "the matched GUI element does not expose a value",
                    )
                })?;
                if actual != *value {
                    return Err(DriverError::new(
                        "test.assert.value",
                        format!("expected GUI value {value:?}, received {actual:?}"),
                    ));
                }
                Ok(StepOutput::new("GUI target value matched").with_data(json!({
                    "target_ref": address.reference,
                    "expected": value,
                    "actual": actual,
                })))
            }
            Expectation::Layout {
                target,
                relative_to,
                relation,
                tolerance_px,
            } => {
                if *tolerance_px > MAX_LAYOUT_TOLERANCE_PX {
                    return Err(DriverError::new(
                        "test.driver.gui.assertion_invalid",
                        format!(
                            "layout tolerance cannot exceed {MAX_LAYOUT_TOLERANCE_PX} pixels"
                        ),
                    ));
                }
                if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. })
                    || matches!(relative_to, Target::Ref { .. } | Target::VisualPoint { .. })
                {
                    return Err(DriverError::new(
                        "test.driver.gui.assertion_unsupported",
                        "layout assertions require two stable semantic or automation-ID targets",
                    ));
                }
                let target_address = self.semantics.resolve(target)?;
                let relative_address = self.semantics.resolve(relative_to)?;
                let target_rect = layout_frame(target_address.frame, "target")?;
                let relative_rect = layout_frame(relative_address.frame, "relative_to target")?;
                if !relation.matches(target_rect, relative_rect, *tolerance_px) {
                    return Err(DriverError::new(
                        "test.assert.layout",
                        format!(
                            "expected {target_rect:?} to be {relation:?} relative to {relative_rect:?} within {tolerance_px}px"
                        ),
                    ));
                }
                Ok(StepOutput::new("GUI layout relation matched").with_data(json!({
                    "target_ref": target_address.reference,
                    "relative_ref": relative_address.reference,
                    "relation": relation,
                    "tolerance_px": tolerance_px,
                    "target_rect": layout_rect_data(target_rect),
                    "relative_rect": layout_rect_data(relative_rect),
                    "matched": true,
                })))
            }
            Expectation::InViewport(_) | Expectation::PointerReachable(_) => {
                Err(DriverError::new(
                    "test.driver.gui.assertion_unsupported",
                    "the current CUA semantic protocol does not expose visual-viewport intersection or point-level pointer hit evidence",
                ))
            }
            Expectation::Url(_) => Err(DriverError::new(
                "test.driver.gui.assertion_unsupported",
                "URL assertions are not available on GUI surfaces",
            )),
            Expectation::RenderedText { .. }
            | Expectation::RenderedTexts { .. }
            | Expectation::VisibleCount { .. }
            | Expectation::State { .. }
            | Expectation::SelectedValues { .. } => {
                Err(DriverError::new(
                    "test.driver.gui.assertion_unsupported",
                    "the current CUA semantic protocol does not expose rendered text collections, locator cardinality, boolean state, or multi-selection state",
                ))
            }
        }
    }

    async fn screenshot(&mut self, requested: &str) -> Result<StepOutput, DriverError> {
        let path = prepare_png_artifact(&self.artifacts_dir, requested).await?;
        let data = self.refresh(Some(&path)).await?;
        Ok(StepOutput::new("GUI window screenshot captured")
            .with_data(data)
            .with_evidence(Evidence {
                name: requested.to_string(),
                path: path.display().to_string(),
                media_type: "image/png".to_string(),
            }))
    }
}

fn layout_frame(frame: Option<LayoutRect>, subject: &str) -> Result<LayoutRect, DriverError> {
    let frame = frame.ok_or_else(|| {
        DriverError::new(
            "test.driver.gui.assertion_unsupported",
            format!("the matched GUI {subject} does not expose a frame"),
        )
    })?;
    if !frame.is_valid() {
        return Err(DriverError::new(
            "test.driver.gui.cua_output_invalid",
            format!("the matched GUI {subject} exposes invalid layout geometry"),
        ));
    }
    Ok(frame)
}

fn layout_rect_data(rect: LayoutRect) -> Value {
    json!({
        "x": rect.x,
        "y": rect.y,
        "width": rect.width,
        "height": rect.height,
    })
}

impl Drop for GuiSession {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let api = Arc::clone(&self.api);
        let application = self.application.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = cleanup_resources(api, Some(application)).await;
            });
        }
    }
}

fn action_output(summary: &str, reference: Option<&str>, result: &CuaActionResult) -> StepOutput {
    StepOutput::new(summary).with_data(json!({
        "surface": "gui",
        "target_ref": reference,
        "verified": result.verified(),
    }))
}

fn visual_action_output(
    summary: &str,
    address: &VisualAddress,
    result: &CuaActionResult,
) -> StepOutput {
    StepOutput::new(summary)
        .with_data(json!({
            "surface": "gui",
            "visual_ref": address.reference,
            "point": { "x": address.x, "y": address.y },
            "grounding_sha256": address.digest,
            "verified": result.verified(),
        }))
        .with_evidence(Evidence {
            name: format!("grounding-{}", address.reference),
            path: address.evidence_path.clone(),
            media_type: "image/png".to_string(),
        })
}

fn visual_drag_output(
    source: &VisualAddress,
    target: &VisualAddress,
    result: &CuaActionResult,
) -> StepOutput {
    StepOutput::new("GUI visual drag completed")
        .with_data(json!({
            "surface": "gui",
            "visual_ref": source.reference,
            "source": { "x": source.x, "y": source.y },
            "target": { "x": target.x, "y": target.y },
            "grounding_sha256": source.digest,
            "verified": result.verified(),
        }))
        .with_evidence(Evidence {
            name: format!("grounding-{}", source.reference),
            path: source.evidence_path.clone(),
            media_type: "image/png".to_string(),
        })
}

fn validate_context_component(value: &str, field: &str) -> Result<(), DriverError> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(DriverError::new(
            "test.driver.gui.session_name_invalid",
            format!("{field} must contain only ASCII letters, digits, '-' or '_'"),
        ));
    }
    Ok(())
}

fn session_id(context: &ScenarioContext) -> String {
    let sequence = SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "a3s-test-{}-{}-p{}-{sequence}",
        compact_component(&context.run_id, 24),
        compact_component(&context.scenario_id, 24),
        std::process::id(),
    )
}

fn compact_component(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let prefix = value
        .chars()
        .take(max_bytes.saturating_sub(9))
        .collect::<String>();
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{prefix}-{:08x}", hasher.finish() as u32)
}
