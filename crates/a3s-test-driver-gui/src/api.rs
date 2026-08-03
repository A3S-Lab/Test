use std::path::Path;
use std::sync::Arc;

use a3s_test_core::DriverError;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::admission::CuaToolOutput;
use crate::CuaClient;

const MAX_SEMANTIC_ELEMENTS: u32 = 2_000;
const MAX_SEMANTIC_DEPTH: u32 = 25;

pub(crate) struct CuaApi {
    client: Arc<CuaClient>,
    session_id: String,
}

impl CuaApi {
    pub(crate) fn new(client: Arc<CuaClient>, session_id: String) -> Self {
        Self { client, session_id }
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    pub(crate) async fn start_window_session(&self) -> Result<(), DriverError> {
        let started: SessionState = self
            .structured_call(
                "start_session",
                json!({
                    "session": self.session_id,
                    "capture_scope": "window",
                }),
            )
            .await?;
        if let Err(error) = validate_session_state(&started, &self.session_id, true) {
            let _ = self.end_session().await;
            return Err(error);
        }

        let current: SessionState = match self
            .structured_call("get_session_state", json!({ "session": self.session_id }))
            .await
        {
            Ok(current) => current,
            Err(error) => {
                let _ = self.end_session().await;
                return Err(error);
            }
        };
        if let Err(error) = validate_session_state(&current, &self.session_id, false) {
            let _ = self.end_session().await;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) async fn permissions(&self) -> Result<CuaPermissions, DriverError> {
        self.structured_call("check_permissions", json!({ "prompt": false }))
            .await
    }

    pub(crate) async fn list_apps(&self) -> Result<Vec<CuaApp>, DriverError> {
        let result: AppList = self.structured_call("list_apps", json!({})).await?;
        validate_apps(result.apps)
    }

    pub(crate) async fn launch_macos_app(
        &self,
        bundle_id: &str,
        arguments: &[String],
    ) -> Result<CuaLaunchResult, DriverError> {
        self.structured_call(
            "launch_app",
            json!({
                "bundle_id": bundle_id,
                "additional_arguments": arguments,
                "creates_new_application_instance": true,
            }),
        )
        .await
    }

    pub(crate) async fn list_windows(&self, pid: i32) -> Result<Vec<CuaWindow>, DriverError> {
        let result: WindowList = self
            .structured_call("list_windows", json!({ "pid": pid }))
            .await?;
        validate_windows(result.windows, pid)
    }

    pub(crate) async fn window_state(
        &self,
        pid: i32,
        window_id: u32,
        screenshot_path: Option<&Path>,
    ) -> Result<CuaWindowState, DriverError> {
        let mut arguments = json!({
            "session": self.session_id,
            "pid": pid,
            "window_id": window_id,
            "include_screenshot": screenshot_path.is_some(),
            "max_elements": MAX_SEMANTIC_ELEMENTS,
            "max_depth": MAX_SEMANTIC_DEPTH,
        });
        if let Some(path) = screenshot_path {
            let path = path.to_str().ok_or_else(|| {
                DriverError::new(
                    "test.driver.gui.artifact_path_invalid",
                    "GUI artifact path must be valid Unicode",
                )
            })?;
            arguments["screenshot_out_file"] = Value::String(path.to_string());
        }
        let state: CuaWindowState = self.structured_call("get_window_state", arguments).await?;
        state.validate(pid, window_id)?;
        Ok(state)
    }

    pub(crate) async fn action(
        &self,
        tool: &str,
        arguments: Value,
    ) -> Result<CuaActionResult, DriverError> {
        let output = self.client.call_tool(tool, arguments).await?;
        Ok(CuaActionResult {
            structured: output.structured,
        })
    }

    pub(crate) async fn kill_app(&self, pid: i32) -> Result<(), DriverError> {
        self.client
            .call_tool("kill_app", json!({ "pid": pid }))
            .await
            .map(|_| ())
    }

    pub(crate) async fn end_session(&self) -> Result<(), DriverError> {
        self.client
            .call_tool("end_session", json!({ "session": self.session_id }))
            .await
            .map(|_| ())
    }

    pub(crate) async fn close(&self) -> Result<(), DriverError> {
        self.client.close().await
    }

    async fn structured_call<T>(&self, tool: &str, arguments: Value) -> Result<T, DriverError>
    where
        T: DeserializeOwned,
    {
        let output = self.client.call_tool(tool, arguments).await?;
        parse_structured(tool, output)
    }
}

fn parse_structured<T>(tool: &str, output: CuaToolOutput) -> Result<T, DriverError>
where
    T: DeserializeOwned,
{
    let value = output.structured.ok_or_else(|| {
        DriverError::new(
            "test.driver.gui.cua_output_invalid",
            format!("CUA {tool} omitted structuredContent"),
        )
    })?;
    serde_json::from_value(value).map_err(|error| {
        DriverError::new(
            "test.driver.gui.cua_output_invalid",
            format!("CUA {tool} returned invalid structuredContent: {error}"),
        )
    })
}

#[derive(Deserialize)]
struct SessionState {
    session: String,
    capture_scope: String,
    effective_scope: String,
    #[serde(default)]
    active: Option<bool>,
}

fn validate_session_state(
    state: &SessionState,
    expected_session: &str,
    require_active: bool,
) -> Result<(), DriverError> {
    if state.session != expected_session
        || state.capture_scope != "window"
        || state.effective_scope != "window"
        || (require_active && state.active != Some(true))
    {
        return Err(DriverError::new(
            "test.driver.gui.session_contract_invalid",
            "CUA did not bind the requested strict window-scoped session",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CuaPermissions {
    pub accessibility: bool,
    pub screen_recording: bool,
    pub source: PermissionSource,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PermissionSource {
    pub attribution: String,
}

#[derive(Deserialize)]
struct AppList {
    apps: Vec<CuaApp>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CuaApp {
    pub pid: i32,
    pub name: String,
    #[serde(default)]
    pub bundle_id: Option<String>,
    pub running: bool,
}

fn validate_apps(apps: Vec<CuaApp>) -> Result<Vec<CuaApp>, DriverError> {
    let mut running_pids = std::collections::BTreeSet::new();
    for app in &apps {
        if app.running && app.pid <= 0 {
            return Err(output_error(
                "CUA list_apps returned a non-positive running pid",
            ));
        }
        if app.running && !running_pids.insert(app.pid) {
            return Err(output_error(
                "CUA list_apps returned a duplicate running pid",
            ));
        }
    }
    Ok(apps)
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CuaLaunchResult {
    pub pid: i32,
    pub bundle_id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct WindowList {
    windows: Vec<CuaWindow>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CuaWindow {
    pub window_id: u32,
    pub pid: i32,
    pub title: String,
    pub z_index: i64,
    #[serde(default, alias = "automationId", alias = "identifier")]
    pub automation_id: Option<String>,
}

fn validate_windows(windows: Vec<CuaWindow>, pid: i32) -> Result<Vec<CuaWindow>, DriverError> {
    let mut ids = std::collections::BTreeSet::new();
    for window in &windows {
        if window.pid != pid || window.window_id == 0 || !ids.insert(window.window_id) {
            return Err(output_error(
                "CUA list_windows returned an invalid or mismatched window identity",
            ));
        }
    }
    Ok(windows)
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CuaWindowState {
    pub window_id: u32,
    pub pid: i32,
    pub element_count: usize,
    #[serde(rename = "tree_markdown")]
    _tree_markdown: String,
    pub elements: Vec<CuaElement>,
    pub snapshot_id: String,
    #[serde(default)]
    pub degraded: bool,
    #[serde(rename = "degraded_reason", default)]
    _degraded_reason: Option<String>,
    #[serde(default)]
    pub screenshot_width: Option<u32>,
    #[serde(default)]
    pub screenshot_height: Option<u32>,
    #[serde(default)]
    pub screenshot_mime_type: Option<String>,
    #[serde(default)]
    pub screenshot_file_path: Option<String>,
}

impl CuaWindowState {
    fn validate(&self, expected_pid: i32, expected_window: u32) -> Result<(), DriverError> {
        if self.pid != expected_pid
            || self.window_id != expected_window
            || self.snapshot_id.trim().is_empty()
            || self.element_count != self.elements.len()
        {
            return Err(output_error(
                "CUA get_window_state returned an inconsistent window snapshot",
            ));
        }
        let visual_fields = [
            self.screenshot_width.is_some(),
            self.screenshot_height.is_some(),
            self.screenshot_mime_type.is_some(),
            self.screenshot_file_path.is_some(),
        ];
        if visual_fields.iter().any(|present| *present)
            && !visual_fields.iter().all(|present| *present)
        {
            return Err(output_error(
                "CUA get_window_state returned partial screenshot metadata",
            ));
        }
        let mut indices = std::collections::BTreeSet::new();
        let mut tokens = std::collections::BTreeSet::new();
        for element in &self.elements {
            if element.element_token.trim().is_empty()
                || element.role.trim().is_empty()
                || !indices.insert(element.element_index)
                || !tokens.insert(element.element_token.as_str())
            {
                return Err(output_error(
                    "CUA get_window_state returned duplicate or invalid elements",
                ));
            }
            if let Some(frame) = &element.frame {
                if !frame.is_valid() {
                    return Err(output_error(
                        "CUA get_window_state returned an invalid element frame",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CuaElement {
    pub element_index: u64,
    pub element_token: String,
    pub role: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default, alias = "automationId", alias = "identifier")]
    pub automation_id: Option<String>,
    #[serde(default)]
    pub frame: Option<CuaFrame>,
    #[serde(default)]
    pub parent_index: Option<u64>,
    pub depth: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CuaFrame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl CuaFrame {
    fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.w.is_finite()
            && self.h.is_finite()
            && self.w >= 0.0
            && self.h >= 0.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CuaActionResult {
    pub structured: Option<Value>,
}

impl CuaActionResult {
    pub(crate) fn verified(&self) -> Option<bool> {
        self.structured
            .as_ref()
            .and_then(|value| value.get("verified"))
            .and_then(Value::as_bool)
    }
}

fn output_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.gui.cua_output_invalid", message)
}
