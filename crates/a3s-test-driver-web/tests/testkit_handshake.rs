use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, BrowserCommand,
    CommandError, CommandExecutor, CommandInvocation, CommandOutput,
};
use async_trait::async_trait;
use serde_json::{json, Value};

const REQUIRED_CAPABILITIES: [&str; 7] = [
    "bounded_snapshot",
    "component_boundaries",
    "design_references",
    "geometry",
    "repair_queue",
    "revision_wait",
    "scoped_inspection",
];

struct HandshakeExecutor {
    response: Value,
    invocations: Mutex<Vec<CommandInvocation>>,
}

impl HandshakeExecutor {
    fn new(response: Value) -> Self {
        Self {
            response,
            invocations: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl CommandExecutor for HandshakeExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version_probe = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        let is_eval = invocation.args.iter().any(|argument| argument == "eval");
        self.invocations.lock().unwrap().push(invocation);
        let stdout = if version_probe {
            "agent-browser 0.26.0".to_string()
        } else if is_eval {
            json!({ "success": true, "data": { "result": self.response } }).to_string()
        } else {
            json!({ "success": true }).to_string()
        };
        Ok(CommandOutput {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        })
    }
}

#[tokio::test]
async fn returns_a_typed_compatible_live_testkit_handshake() {
    let executor = Arc::new(HandshakeExecutor::new(compatible_response(true)));
    let (mut session, _temp) = session(executor.clone()).await;

    let handshake = session
        .testkit_handshake(true)
        .await
        .expect("compatible handshake")
        .expect("present Test Kit");

    assert_eq!(handshake.protocol, "a3s.test.testkit-handshake/1");
    assert_eq!(handshake.package_name, "@a3s-lab/testkit");
    assert_eq!(handshake.sdk_version.to_string(), "0.4.2");
    assert_eq!(handshake.page_context_protocol, "a3s.test.page-context/1");
    assert_eq!(
        handshake.capabilities,
        REQUIRED_CAPABILITIES.map(str::to_string).to_vec()
    );
    assert!(handshake.review_overlay_mounted);

    let invocations = executor.invocations.lock().unwrap();
    let args = strip_session_prefix(&invocations[1].args);
    assert_eq!(args[0], "eval");
    let script = args[1].to_string_lossy();
    assert!(script.contains("bridge.handshake()"), "{script}");
    assert!(script.contains("data-a3s-testkit-overlay"), "{script}");
    assert!(script.contains("requestAnimationFrame"), "{script}");
    assert!(script.contains("timeoutMs"), "{script}");
}

#[tokio::test]
async fn returns_none_only_when_the_page_context_bridge_is_absent() {
    let executor = Arc::new(HandshakeExecutor::new(json!({ "state": "absent" })));
    let (mut session, _temp) = session(executor).await;

    assert_eq!(
        session
            .testkit_handshake(false)
            .await
            .expect("optional bridge"),
        None
    );
}

#[tokio::test]
async fn reports_each_incompatible_testkit_boundary() {
    let cases = [
        (
            json!({ "state": "bridge_invalid" }),
            "test.driver.web.testkit_bridge_invalid",
        ),
        (
            json!({ "state": "handshake_missing" }),
            "test.driver.web.testkit_handshake_missing",
        ),
        (
            json!({ "state": "handshake_failed" }),
            "test.driver.web.testkit_handshake_failed",
        ),
        (
            response_with("protocol", json!("a3s.test.testkit-handshake/2")),
            "test.driver.web.testkit_handshake_protocol_unsupported",
        ),
        (
            response_with("packageName", json!("untrusted-package")),
            "test.driver.web.testkit_package_unsupported",
        ),
        (
            response_with("sdkVersion", json!("not-semver")),
            "test.driver.web.testkit_sdk_version_invalid",
        ),
        (
            response_with("sdkVersion", json!("0.5.0")),
            "test.driver.web.testkit_sdk_version_unsupported",
        ),
        (
            response_with("sdkVersion", json!("0.4.3-beta.1")),
            "test.driver.web.testkit_sdk_version_unsupported",
        ),
        (
            response_with("pageContextProtocol", json!("a3s.test.page-context/2")),
            "test.driver.web.testkit_page_context_protocol_unsupported",
        ),
        (
            response_with("capabilities", json!(["bounded_snapshot", 7])),
            "test.driver.web.testkit_capabilities_invalid",
        ),
        (
            response_with(
                "capabilities",
                json!([
                    "component_boundaries",
                    "bounded_snapshot",
                    "design_references",
                    "geometry",
                    "repair_queue",
                    "revision_wait",
                    "scoped_inspection"
                ]),
            ),
            "test.driver.web.testkit_capabilities_invalid",
        ),
        (
            response_with(
                "capabilities",
                json!([
                    "bounded_snapshot",
                    "component_boundaries",
                    "design_references",
                    "geometry",
                    "repair_queue",
                    "revision_wait"
                ]),
            ),
            "test.driver.web.testkit_capability_missing",
        ),
        (
            compatible_response(false),
            "test.driver.web.testkit_review_overlay_missing",
        ),
    ];

    for (response, expected_code) in cases {
        let executor = Arc::new(HandshakeExecutor::new(response));
        let (mut session, _temp) = session(executor).await;
        let error = session
            .testkit_handshake(true)
            .await
            .expect_err(expected_code);
        assert_eq!(error.code(), expected_code);
    }
}

#[tokio::test]
async fn never_reflects_untrusted_handshake_fields_in_driver_errors() {
    let marker = "page-secret\u{1b}[31m";
    let cases = [
        json!({ "state": marker }),
        response_with("protocol", json!(marker)),
        response_with("packageName", json!(marker)),
        response_with("sdkVersion", json!(marker)),
        response_with("pageContextProtocol", json!(marker)),
        response_with(
            "capabilities",
            json!([
                "bounded_snapshot",
                "component_boundaries",
                "design_references",
                "geometry",
                marker,
                "repair_queue",
                "revision_wait",
                "scoped_inspection"
            ]),
        ),
    ];

    for response in cases {
        let executor = Arc::new(HandshakeExecutor::new(response));
        let (mut session, _temp) = session(executor).await;
        let error = session
            .testkit_handshake(true)
            .await
            .expect_err("untrusted handshake must fail");
        assert!(!error.message().contains("page-secret"), "{error}");
        assert!(!error.message().contains('\u{1b}'), "{error}");
    }
}

fn compatible_response(review_overlay_mounted: bool) -> Value {
    json!({
        "state": "present",
        "handshake": {
            "protocol": "a3s.test.testkit-handshake/1",
            "packageName": "@a3s-lab/testkit",
            "sdkVersion": "0.4.2",
            "pageContextProtocol": "a3s.test.page-context/1",
            "capabilities": REQUIRED_CAPABILITIES,
        },
        "reviewOverlayMounted": review_overlay_mounted,
    })
}

fn response_with(field: &str, value: Value) -> Value {
    let mut response = compatible_response(true);
    response["handshake"][field] = value;
    response
}

async fn session(
    executor: Arc<HandshakeExecutor>,
) -> (a3s_test_driver_web::AgentBrowserSession, tempfile::TempDir) {
    let temp = tempfile::tempdir().expect("tempdir");
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "testkit-handshake".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            microphone: Default::default(),
            network_policy: Default::default(),
        },
        executor,
    );
    let session = driver
        .connect(AgentBrowserConnectionConfig {
            namespace: "testkit-handshake".to_string(),
            session: "handshake".to_string(),
            runtime_dir: temp.path().join("runtime"),
            artifacts_dir: temp.path().join("artifacts"),
            active_video_path: None,
        })
        .await
        .expect("session");
    (session, temp)
}

fn strip_session_prefix(args: &[OsString]) -> Vec<OsString> {
    args.iter().skip(5).cloned().collect()
}
