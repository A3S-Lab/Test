use std::ffi::OsString;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, Expectation, ScenarioContext, Surface, SurfaceDriver, Target, TestStep,
};
use a3s_test_driver_gui::{
    ApplicationIdentity, AttachSpec, CuaCompatibility, CuaEndpoint, CuaTransport,
    CuaTransportError, CuaTransportFactory, GuiAppTarget, GuiCaptureScope, GuiDriver,
    GuiDriverConfig, GuiHostPermission, GuiHostPermissionSource, GuiProfile, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, LaunchSpec, WindowSelector,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::sync::Mutex;

const APP_PID: i32 = 4_242;
const WINDOW_ID: u32 = 77;
const BUNDLE_ID: &str = "com.example.Editor";

#[derive(Clone, Copy)]
struct FakeOptions {
    initially_running: bool,
    accessibility: bool,
    screen_recording: bool,
    permission_attribution: &'static str,
    has_window: bool,
    ambiguous_elements: bool,
    launch_response_delay: Duration,
    kill_failures: usize,
    kill_visibility_polls: usize,
    end_session_failures: usize,
}

impl Default for FakeOptions {
    fn default() -> Self {
        Self {
            initially_running: false,
            accessibility: true,
            screen_recording: true,
            permission_attribution: "driver-daemon",
            has_window: true,
            ambiguous_elements: false,
            launch_response_delay: Duration::ZERO,
            kill_failures: 0,
            kill_visibility_polls: 0,
            end_session_failures: 0,
        }
    }
}

struct FakeState {
    options: FakeOptions,
    running: bool,
    bundle_id: String,
    snapshots: u64,
    tool_calls: Vec<(String, Value)>,
    notifications: Vec<String>,
    closed: bool,
    kill_failures_remaining: usize,
    shutdown_polls_remaining: Option<usize>,
    end_session_failures_remaining: usize,
}

struct FakeTransport {
    state: Mutex<FakeState>,
}

impl FakeTransport {
    fn new(options: FakeOptions) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(FakeState {
                options,
                running: options.initially_running,
                bundle_id: BUNDLE_ID.to_string(),
                snapshots: 0,
                tool_calls: Vec::new(),
                notifications: Vec::new(),
                closed: false,
                kill_failures_remaining: options.kill_failures,
                shutdown_polls_remaining: None,
                end_session_failures_remaining: options.end_session_failures,
            }),
        })
    }

    async fn tool_names(&self) -> Vec<String> {
        self.state
            .lock()
            .await
            .tool_calls
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    async fn calls_for(&self, name: &str) -> Vec<Value> {
        self.state
            .lock()
            .await
            .tool_calls
            .iter()
            .filter(|(tool, _)| tool == name)
            .map(|(_, arguments)| arguments.clone())
            .collect()
    }

    async fn closed(&self) -> bool {
        self.state.lock().await.closed
    }

    async fn running(&self) -> bool {
        self.state.lock().await.running
    }

    async fn replace_running_identity(&self, bundle_id: &str) {
        self.state.lock().await.bundle_id = bundle_id.to_string();
    }

    async fn set_window_available(&self, available: bool) {
        self.state.lock().await.options.has_window = available;
    }
}

#[async_trait]
impl CuaTransport for FakeTransport {
    async fn request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, CuaTransportError> {
        match request.method.as_str() {
            "initialize" => Ok(JsonRpcResponse::success(
                request.id,
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "cua-driver", "version": "0.10.0" },
                }),
            )),
            "tools/list" => Ok(JsonRpcResponse::success(request.id, tools_list())),
            "tools/call" => {
                let params = request
                    .params
                    .and_then(|value| value.as_object().cloned())
                    .ok_or_else(|| CuaTransportError::protocol("missing tools/call params"))?;
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| CuaTransportError::protocol("missing tool name"))?
                    .to_string();
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let (result, response_delay) = {
                    let mut state = self.state.lock().await;
                    let result = dispatch_tool(&mut state, &name, &arguments)?;
                    let response_delay = if name == "launch_app" {
                        state.options.launch_response_delay
                    } else {
                        Duration::ZERO
                    };
                    (result, response_delay)
                };
                if !response_delay.is_zero() {
                    tokio::time::sleep(response_delay).await;
                }
                Ok(JsonRpcResponse::success(request.id, result))
            }
            method => Err(CuaTransportError::protocol(format!(
                "unexpected method {method}"
            ))),
        }
    }

    async fn notify(&self, notification: JsonRpcNotification) -> Result<(), CuaTransportError> {
        self.state
            .lock()
            .await
            .notifications
            .push(notification.method);
        Ok(())
    }

    async fn close(&self) -> Result<(), CuaTransportError> {
        self.state.lock().await.closed = true;
        Ok(())
    }
}

struct FakeFactory {
    transport: Arc<FakeTransport>,
}

#[async_trait]
impl CuaTransportFactory for FakeFactory {
    async fn connect(
        &self,
        _config: &GuiDriverConfig,
    ) -> Result<Arc<dyn CuaTransport>, a3s_test_core::DriverError> {
        Ok(self.transport.clone())
    }
}

fn tools_list() -> Value {
    let compatibility = CuaCompatibility::locked().expect("compatibility lock");
    let tools = compatibility
        .tools()
        .iter()
        .map(|(name, requirement)| {
            json!({
                "name": name,
                "description": format!("fake {name}"),
                "inputSchema": { "type": "object" },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false,
                    "idempotentHint": false,
                    "openWorldHint": false,
                },
                "capabilities": requirement.capabilities(),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "tools": tools,
        "capability_version": compatibility.capability_vocabulary(),
        "schema_version": compatibility.tools_schema(),
    })
}

fn dispatch_tool(
    state: &mut FakeState,
    name: &str,
    arguments: &Value,
) -> Result<Value, CuaTransportError> {
    state.tool_calls.push((name.to_string(), arguments.clone()));
    let session = arguments
        .get("session")
        .and_then(Value::as_str)
        .unwrap_or("missing-session");
    let structured = match name {
        "start_session" => json!({
            "session": session,
            "capture_scope": "window",
            "effective_scope": "window",
            "active": true,
        }),
        "get_session_state" => json!({
            "session": session,
            "capture_scope": "window",
            "effective_scope": "window",
        }),
        "end_session" => {
            if state.end_session_failures_remaining > 0 {
                state.end_session_failures_remaining -= 1;
                return Ok(json!({
                    "content": [{ "type": "text", "text": "transient end failure" }],
                    "isError": true,
                    "structuredContent": { "code": "temporary_failure" },
                }));
            }
            json!({ "session": session, "active": false })
        }
        "check_permissions" => json!({
            "accessibility": state.options.accessibility,
            "screen_recording": state.options.screen_recording,
            "source": { "attribution": state.options.permission_attribution },
        }),
        "list_apps" => {
            if state.shutdown_polls_remaining == Some(0) {
                state.running = false;
                state.shutdown_polls_remaining = None;
            }
            let running = state.running;
            if let Some(remaining) = &mut state.shutdown_polls_remaining {
                *remaining = remaining.saturating_sub(1);
            }
            if running {
                json!({
                    "apps": [{
                        "pid": APP_PID,
                        "name": "Editor",
                        "bundle_id": state.bundle_id,
                        "running": true,
                    }]
                })
            } else {
                json!({
                    "apps": [{
                        "pid": 0,
                        "name": "Editor",
                        "bundle_id": BUNDLE_ID,
                        "running": false,
                    }]
                })
            }
        }
        "launch_app" => {
            state.running = true;
            state.shutdown_polls_remaining = None;
            json!({
                "pid": APP_PID,
                "bundle_id": BUNDLE_ID,
                "name": "Editor",
                "windows": [],
            })
        }
        "list_windows" => {
            let windows = if state.options.has_window {
                vec![json!({
                    "window_id": WINDOW_ID,
                    "pid": APP_PID,
                    "title": "Document",
                    "z_index": 10,
                })]
            } else {
                Vec::new()
            };
            json!({ "windows": windows, "current_space_id": null })
        }
        "get_window_state" => window_state(state, arguments)?,
        "click" | "double_click" | "right_click" | "set_value" | "type_text" | "press_key"
        | "scroll" | "drag" => json!({ "verified": true }),
        "kill_app" => {
            if arguments.get("pid").and_then(Value::as_i64) != Some(i64::from(APP_PID)) {
                return Err(CuaTransportError::protocol(
                    "attempted to kill unexpected pid",
                ));
            }
            if state.kill_failures_remaining > 0 {
                state.kill_failures_remaining -= 1;
                return Ok(json!({
                    "content": [{ "type": "text", "text": "transient kill failure" }],
                    "isError": true,
                    "structuredContent": { "code": "temporary_failure" },
                }));
            }
            state.shutdown_polls_remaining = Some(state.options.kill_visibility_polls);
            return Ok(tool_result(None));
        }
        other => {
            return Ok(json!({
                "content": [{ "type": "text", "text": format!("unsupported {other}") }],
                "isError": true,
                "structuredContent": { "code": "unsupported" },
            }));
        }
    };
    Ok(tool_result(Some(structured)))
}

fn window_state(state: &mut FakeState, arguments: &Value) -> Result<Value, CuaTransportError> {
    state.snapshots += 1;
    let snapshot = state.snapshots;
    let second_label = if state.options.ambiguous_elements {
        "Save"
    } else {
        "Email"
    };
    let second_role = if state.options.ambiguous_elements {
        "AXButton"
    } else {
        "AXTextField"
    };
    let mut structured = json!({
        "window_id": WINDOW_ID,
        "pid": APP_PID,
        "element_count": 2,
        "tree_markdown": "fake tree",
        "elements": [
            {
                "element_index": 1,
                "element_token": format!("cua:{snapshot}:1"),
                "role": "AXButton",
                "label": "Save",
                "automation_id": "save-button",
                "frame": { "x": 10.0, "y": 10.0, "w": 80.0, "h": 30.0 },
                "depth": 1,
            },
            {
                "element_index": 2,
                "element_token": format!("cua:{snapshot}:2"),
                "role": second_role,
                "label": second_label,
                "value": "draft@example.test",
                "frame": { "x": 10.0, "y": 50.0, "w": 180.0, "h": 30.0 },
                "depth": 1,
            }
        ],
        "snapshot_id": format!("cua:{snapshot}"),
    });
    if let Some(path) = arguments.get("screenshot_out_file").and_then(Value::as_str) {
        std::fs::write(path, b"fake-png")
            .map_err(|error| CuaTransportError::protocol(error.to_string()))?;
        structured["screenshot_width"] = Value::from(800);
        structured["screenshot_height"] = Value::from(600);
        structured["screenshot_mime_type"] = Value::String("image/png".to_string());
        structured["screenshot_file_path"] = Value::String(path.to_string());
    }
    Ok(structured)
}

fn tool_result(structured: Option<Value>) -> Value {
    let mut result = json!({
        "content": [{ "type": "text", "text": "ok" }],
    });
    if let Some(structured) = structured {
        result["structuredContent"] = structured;
    }
    result
}

fn launch_config(temp: &TempDir) -> GuiDriverConfig {
    GuiDriverConfig {
        endpoint: CuaEndpoint::InstalledDaemon {
            proxy_executable: PathBuf::from("cua-driver"),
        },
        policy_file: temp.path().join("policy.yaml"),
        target: GuiAppTarget::Launch(LaunchSpec {
            application: ApplicationIdentity::MacOsBundle {
                bundle_id: BUNDLE_ID.to_string(),
            },
            arguments: vec![OsString::from("--safe-mode")],
            working_directory: None,
        }),
        window: WindowSelector::Primary,
        capture_scope: GuiCaptureScope::Window,
        profile: GuiProfile::Semantic,
        command_timeout: Duration::from_secs(2),
        removed_environment: Default::default(),
    }
}

fn attach_config(temp: &TempDir) -> GuiDriverConfig {
    let mut config = launch_config(temp);
    config.target = GuiAppTarget::Attach(AttachSpec {
        application: ApplicationIdentity::MacOsBundle {
            bundle_id: BUNDLE_ID.to_string(),
        },
        process_id: NonZeroU32::new(APP_PID as u32),
    });
    config
}

fn embedded_config(temp: &TempDir) -> GuiDriverConfig {
    let mut config = launch_config(temp);
    config.endpoint = CuaEndpoint::EmbeddedSocket {
        proxy_executable: PathBuf::from("cua-driver"),
        socket: temp.path().join("cua.sock"),
    };
    config
}

fn context(temp: &TempDir) -> ScenarioContext {
    ScenarioContext {
        run_id: "run_1".to_string(),
        scenario_id: "editor".to_string(),
        artifacts_dir: temp.path().join("artifacts"),
    }
}

fn driver(config: GuiDriverConfig, transport: Arc<FakeTransport>) -> GuiDriver {
    GuiDriver::with_transport_factory(config, Arc::new(FakeFactory { transport }))
}

#[tokio::test]
async fn host_probe_is_read_only_and_returns_exact_permissions() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let probe = driver(launch_config(&temp), Arc::clone(&transport))
        .probe_host()
        .await
        .expect("GUI host probe");

    assert_eq!(
        probe.permissions.source,
        GuiHostPermissionSource::DriverDaemon
    );
    assert_eq!(
        probe.permissions.permissions,
        [
            GuiHostPermission::Accessibility,
            GuiHostPermission::ScreenRecording,
        ]
    );
    assert_eq!(probe.driver_version, "0.10.0");
    assert!(probe.permissions.digest().starts_with("sha256:"));
    let names = transport.tool_names().await;
    assert_eq!(names, ["check_permissions"]);
    assert!(transport.closed().await);
    assert!(!transport.running().await);
}

#[tokio::test]
async fn semantic_actions_use_opaque_refs_and_owned_cleanup() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let driver = driver(launch_config(&temp), Arc::clone(&transport));
    assert_eq!(driver.surface(), Surface::Gui);
    let mut session = driver.open(&context(&temp)).await.expect("GUI session");

    let observation = session.observe().await.expect("semantic observation");
    let first_ref = observation.data["elements"][0]["ref"]
        .as_str()
        .expect("opaque reference")
        .to_string();
    assert!(first_ref.starts_with("@g1."));
    assert!(!observation.data.to_string().contains("element_token"));
    assert!(!observation.data.to_string().contains("cua:1"));

    session
        .execute(&TestStep {
            id: "save".to_string(),
            action: Action::Click {
                target: Target::AutomationId {
                    value: "save-button".to_string(),
                },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect("semantic click");
    let click = transport.calls_for("click").await;
    assert_eq!(click.len(), 1);
    assert_eq!(click[0]["element_token"], "cua:1:1");

    let stale = session
        .execute(&TestStep {
            id: "stale".to_string(),
            action: Action::Click {
                target: Target::Ref { value: first_ref },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("expired reference must fail");
    assert_eq!(stale.code(), "test.driver.gui.stale_reference");

    let screenshot = session
        .execute(&TestStep {
            id: "capture".to_string(),
            action: Action::Screenshot {
                path: "screens/window.png".to_string(),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect("window screenshot");
    assert_eq!(screenshot.evidence.len(), 1);
    assert!(PathBuf::from(&screenshot.evidence[0].path).is_file());

    session.close().await.expect("close GUI session");
    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 1);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
    assert!(transport.closed().await);
}

#[tokio::test]
async fn attached_application_is_never_terminated() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        initially_running: true,
        ..FakeOptions::default()
    });
    let mut session = driver(attach_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("attached GUI session");

    session.close().await.expect("close attached session");
    let names = transport.tool_names().await;
    assert!(!names.iter().any(|name| name == "kill_app"));
    assert!(transport.closed().await);
}

#[tokio::test]
async fn ambiguous_semantic_target_fails_without_input() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        ambiguous_elements: true,
        ..FakeOptions::default()
    });
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");
    session.observe().await.expect("semantic observation");

    let error = session
        .execute(&TestStep {
            id: "ambiguous".to_string(),
            action: Action::Click {
                target: Target::Label {
                    value: "Save".to_string(),
                },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("ambiguous target must fail");
    assert_eq!(error.code(), "test.driver.gui.target_ambiguous");
    assert!(transport.calls_for("click").await.is_empty());

    let assertion_error = session
        .execute(&TestStep {
            id: "ambiguous-assertion".to_string(),
            action: Action::Assert {
                expectation: Expectation::Visible(Target::Label {
                    value: "Save".to_string(),
                }),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("ambiguous assertion target must remain a driver error");
    assert_eq!(assertion_error.code(), "test.driver.gui.target_ambiguous");
    session.close().await.expect("close session");
}

#[tokio::test]
async fn visible_assertions_classify_missing_targets_without_hiding_reference_errors() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");

    let missing = session
        .execute(&TestStep {
            id: "missing-target".to_string(),
            action: Action::Assert {
                expectation: Expectation::Visible(Target::AutomationId {
                    value: "missing-button".to_string(),
                }),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("missing semantic target must be an assertion mismatch");
    assert_eq!(missing.code(), "test.assert.visible");

    let observation = session.observe().await.expect("semantic observation");
    let generation = observation.data["snapshot"]["generation"]
        .as_u64()
        .expect("snapshot generation");
    let invalid_ref = session
        .execute(&TestStep {
            id: "missing-ref".to_string(),
            action: Action::Assert {
                expectation: Expectation::Visible(Target::Ref {
                    value: format!("@g{generation}.999"),
                }),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("stale observation ref must remain a driver error");
    assert_eq!(invalid_ref.code(), "test.driver.gui.stale_reference");

    session.close().await.expect("close GUI session");
}

#[tokio::test]
async fn missing_permission_ends_session_without_launching() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        accessibility: false,
        ..FakeOptions::default()
    });
    let error = match driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
    {
        Ok(_) => panic!("permission failure must reject the session"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "test.driver.gui.permission_missing");
    let names = transport.tool_names().await;
    assert!(!names.iter().any(|name| name == "launch_app"));
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
    assert!(transport.closed().await);
}

#[tokio::test]
async fn window_binding_failure_cleans_up_only_the_owned_process() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        has_window: false,
        ..FakeOptions::default()
    });
    let error = match driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
    {
        Ok(_) => panic!("missing window must reject the session"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "test.driver.gui.window_not_found");
    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 1);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
    assert!(transport.closed().await);
}

#[tokio::test]
async fn launch_refuses_to_claim_a_preexisting_process() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        initially_running: true,
        ..FakeOptions::default()
    });
    let error = match driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
    {
        Ok(_) => panic!("preexisting process must not become owned"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "test.driver.gui.app_ownership_unproven");
    let names = transport.tool_names().await;
    assert!(!names.iter().any(|name| name == "kill_app"));
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
    assert!(transport.closed().await);
}

#[tokio::test]
async fn window_vision_points_are_grounded_in_verified_image_evidence() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut config = launch_config(&temp);
    config.profile = GuiProfile::WindowVision;
    let mut session = driver(config, Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("visual GUI session");

    let observation = session.observe().await.expect("visual observation");
    let visual_ref = observation.data["visual"]["ref"]
        .as_str()
        .expect("visual ref")
        .to_string();
    assert_eq!(visual_ref, "@v1");
    assert_eq!(observation.evidence.len(), 1);
    assert!(observation.data["visual"]["sha256"].is_string());

    let output = session
        .execute(&TestStep {
            id: "visual-click".to_string(),
            action: Action::Click {
                target: Target::VisualPoint {
                    snapshot: visual_ref,
                    x: 320,
                    y: 240,
                },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect("grounded visual click");
    assert_eq!(output.evidence.len(), 1);
    let click = transport.calls_for("click").await;
    assert_eq!(click[0]["x"], 320);
    assert_eq!(click[0]["y"], 240);
    assert!(click[0].get("element_token").is_none());
    session.close().await.expect("close session");
}

#[tokio::test]
async fn visual_actions_reject_old_or_modified_grounding_images() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut config = launch_config(&temp);
    config.profile = GuiProfile::WindowVision;
    let mut session = driver(config, Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("visual GUI session");

    let first = session.observe().await.expect("first visual observation");
    let first_ref = first.data["visual"]["ref"]
        .as_str()
        .expect("first visual ref")
        .to_string();
    session.observe().await.expect("second visual observation");
    let stale = session
        .execute(&TestStep {
            id: "stale-image".to_string(),
            action: Action::Click {
                target: Target::VisualPoint {
                    snapshot: first_ref,
                    x: 10,
                    y: 10,
                },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("old image must fail");
    assert_eq!(stale.code(), "test.driver.gui.stale_image");

    let current = session.observe().await.expect("current visual observation");
    let current_ref = current.data["visual"]["ref"]
        .as_str()
        .expect("current visual ref")
        .to_string();
    tokio::fs::write(&current.evidence[0].path, b"modified")
        .await
        .expect("modify grounding image");
    let modified = session
        .execute(&TestStep {
            id: "modified-image".to_string(),
            action: Action::Click {
                target: Target::VisualPoint {
                    snapshot: current_ref,
                    x: 10,
                    y: 10,
                },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("modified image must fail");
    assert_eq!(modified.code(), "test.driver.gui.stale_image");
    assert!(transport.calls_for("click").await.is_empty());
    session.close().await.expect("close session");
}

#[tokio::test]
async fn visual_drag_requires_one_current_grounding_image() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut config = launch_config(&temp);
    config.profile = GuiProfile::WindowVision;
    let mut session = driver(config, Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("visual GUI session");
    let observation = session.observe().await.expect("visual observation");
    let visual_ref = observation.data["visual"]["ref"]
        .as_str()
        .expect("visual ref")
        .to_string();

    session
        .execute(&TestStep {
            id: "drag".to_string(),
            action: Action::Drag {
                source: Target::VisualPoint {
                    snapshot: visual_ref.clone(),
                    x: 20,
                    y: 30,
                },
                target: Target::VisualPoint {
                    snapshot: visual_ref,
                    x: 200,
                    y: 220,
                },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect("visual drag");
    let drag = transport.calls_for("drag").await;
    assert_eq!(drag[0]["from_x"], 20);
    assert_eq!(drag[0]["to_y"], 220);
    session.close().await.expect("close session");
}

#[tokio::test]
async fn embedded_endpoint_requires_host_permission_attribution() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        permission_attribution: "host",
        ..FakeOptions::default()
    });
    let mut session = driver(embedded_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("embedded GUI session");

    session.close().await.expect("close embedded session");
    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 1);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
}

#[path = "session/artifact_containment.rs"]
mod artifact_containment;
#[path = "session/lifecycle_cases.rs"]
mod lifecycle_cases;
#[path = "session/runtime_binding.rs"]
mod runtime_binding;
