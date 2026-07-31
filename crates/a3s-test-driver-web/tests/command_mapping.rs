use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{
    Action, CaptureOperation, DialogOperation, FrameTarget, NetworkRoute, ScenarioContext,
    SurfaceDriver, TabOperation, Target, TestStep, VideoOperation,
};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserDriver, BrowserCommand, BrowserIntegration, CommandError,
    CommandExecutor, CommandInvocation, CommandOutput, WebCapability,
};
use async_trait::async_trait;

#[derive(Default)]
struct RecordingExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
    version: Mutex<Option<String>>,
}

struct FailingActionExecutor {
    calls: AtomicUsize,
    error: CommandError,
}

impl RecordingExecutor {
    fn with_version(version: impl Into<String>) -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            version: Mutex::new(Some(version.into())),
        }
    }
}

#[cfg(unix)]
#[derive(Default)]
struct DaemonExecutor {
    child: Mutex<Option<std::process::Child>>,
}

#[async_trait]
impl CommandExecutor for RecordingExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version_probe = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        let default_version = if invocation.args.starts_with(&os(&["use", "browser"])) {
            "a3s use browser 0.1.1"
        } else {
            "agent-browser 0.26.0"
        };
        self.invocations.lock().unwrap().push(invocation);
        Ok(CommandOutput {
            exit_code: 0,
            stdout: if version_probe {
                self.version
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| default_version.to_string())
            } else {
                r#"{"success":true}"#.to_string()
            },
            stderr: String::new(),
        })
    }
}

#[async_trait]
impl CommandExecutor for FailingActionExecutor {
    async fn run(&self, _invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Ok(CommandOutput {
                exit_code: 0,
                stdout: "agent-browser 0.26.0".to_string(),
                stderr: String::new(),
            });
        }
        if call == 1 {
            return Err(self.error.clone());
        }
        Ok(CommandOutput {
            exit_code: 0,
            stdout: r#"{"success":true}"#.to_string(),
            stderr: String::new(),
        })
    }
}

#[cfg(unix)]
#[async_trait]
impl CommandExecutor for DaemonExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        use std::fs;
        use std::os::unix::process::CommandExt;
        use std::process::Command;

        if invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version")
        {
            return Ok(CommandOutput {
                exit_code: 0,
                stdout: "agent-browser 0.26.0".to_string(),
                stderr: String::new(),
            });
        }

        let runtime = invocation
            .env
            .get(&OsString::from("AGENT_BROWSER_SOCKET_DIR"))
            .map(PathBuf::from)
            .expect("runtime directory");
        let session_index = invocation
            .args
            .iter()
            .position(|argument| argument == "--session")
            .expect("session argument");
        let session = invocation
            .args
            .get(session_index + 1)
            .and_then(|value| value.to_str())
            .expect("session value");
        let child = Command::new("/bin/sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .expect("fake daemon");
        fs::write(
            runtime.join(format!("{session}.pid")),
            child.id().to_string(),
        )
        .expect("pid file");
        self.child.lock().unwrap().replace(child);
        Err(CommandError::timed_out("driver is unresponsive"))
    }
}

fn step(id: &str, action: Action) -> TestStep {
    TestStep {
        id: id.to_string(),
        action,
    }
}

#[tokio::test]
async fn maps_typed_actions_and_scopes_browser_lifecycle() {
    let temp = tempfile::tempdir().expect("tempdir");
    let artifacts = temp.path().join("artifacts");
    let executor = Arc::new(RecordingExecutor::default());
    let config = AgentBrowserConfig {
        command: BrowserCommand::A3s {
            executable: PathBuf::from("/opt/a3s"),
        },
        namespace: "run-42".to_string(),
        headed: false,
        command_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(30),
    };
    let driver = AgentBrowserDriver::with_executor(config, executor.clone());
    let context = ScenarioContext {
        run_id: "run-42".to_string(),
        scenario_id: "word".to_string(),
        artifacts_dir: artifacts.clone(),
    };
    let mut session = driver.open(&context).await.expect("session");

    session
        .execute(&step(
            "open",
            Action::Navigate {
                url: "https://example.test".to_string(),
            },
        ))
        .await
        .expect("navigate");
    session
        .execute(&step(
            "word",
            Action::Click {
                target: Target::Role {
                    role: "button".to_string(),
                    name: "Word".to_string(),
                },
            },
        ))
        .await
        .expect("click");
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(invocations.len(), 4);
    assert_eq!(invocations[0].program, PathBuf::from("/opt/a3s"));
    assert_eq!(invocations[0].args, os(&["use", "browser", "--version"]));
    assert_eq!(
        invocations[1].args,
        os(&[
            "use",
            "browser",
            "--session",
            "word",
            "--json",
            "open",
            "https://example.test",
        ])
    );
    assert_eq!(
        invocations[2].args,
        os(&[
            "use",
            "browser",
            "--session",
            "word",
            "--json",
            "find",
            "role",
            "button",
            "click",
            "--name",
            "Word",
        ])
    );
    assert_eq!(
        invocations[3].args,
        os(&["use", "browser", "--session", "word", "--json", "close"])
    );
    let runtime = invocations[1]
        .env
        .get(&OsString::from("A3S_USE_BROWSER_SOCKET_DIR"))
        .expect("runtime");
    assert_short_runtime(runtime);
    assert_eq!(
        invocations[1].env,
        BTreeMap::from([
            (
                OsString::from("A3S_USE_BROWSER_NAMESPACE"),
                OsString::from("run-42")
            ),
            (
                OsString::from("A3S_USE_BROWSER_IDLE_TIMEOUT_MS"),
                OsString::from("30000")
            ),
            (
                OsString::from("A3S_USE_BROWSER_SOCKET_DIR"),
                runtime.clone()
            ),
        ])
    );
}

#[tokio::test]
async fn standalone_driver_uses_upstream_environment_names() {
    let temp = tempfile::tempdir().expect("tempdir");
    let artifacts = temp.path().join("artifacts");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "isolated".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "ignored".to_string(),
            scenario_id: "home".to_string(),
            artifacts_dir: artifacts.clone(),
        })
        .await
        .expect("session");

    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    let runtime = invocations[1]
        .env
        .get(&OsString::from("AGENT_BROWSER_SOCKET_DIR"))
        .expect("runtime");
    assert_short_runtime(runtime);
    assert_eq!(
        invocations[1].env,
        BTreeMap::from([
            (
                OsString::from("AGENT_BROWSER_NAMESPACE"),
                OsString::from("isolated")
            ),
            (
                OsString::from("AGENT_BROWSER_IDLE_TIMEOUT_MS"),
                OsString::from("2000")
            ),
            (OsString::from("AGENT_BROWSER_SOCKET_DIR"), runtime.clone()),
        ])
    );
}

#[tokio::test]
async fn exposes_a_full_browser_snapshot_as_the_agent_observation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "observe".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "agent".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let observation = session.observe().await.expect("observation");
    assert_eq!(observation.summary, "browser accessibility snapshot");
    assert_eq!(observation.data["success"], true);
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(
        invocations[1].args,
        os(&["--session", "agent", "--json", "snapshot"])
    );
}

#[tokio::test]
async fn discovers_and_admits_the_typed_browser_protocol() {
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "capabilities".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );

    let capabilities = driver.capabilities().await.expect("capabilities");
    assert_eq!(capabilities.integration, BrowserIntegration::Standalone);
    assert_eq!(capabilities.version, "0.26.0");
    assert_eq!(capabilities.protocol_revision, 1);
    assert!(capabilities.features.contains(&WebCapability::Tabs));
    assert!(capabilities.features.contains(&WebCapability::Har));
    assert!(capabilities.features.contains(&WebCapability::Video));

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].args, os(&["--version"]));
}

#[tokio::test]
async fn rejects_an_unadmitted_browser_version_before_opening_a_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::with_version("agent-browser 0.25.9"));
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "capabilities".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );

    let result = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "version".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await;
    let error = match result {
        Ok(_) => panic!("unsupported version was admitted"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "test.driver.web.version_unsupported");
    assert_eq!(executor.invocations.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn marks_only_pre_dispatch_command_failures_as_retryable() {
    for (command_error, expected_retryable) in [
        (CommandError::unavailable("executable unavailable"), true),
        (CommandError::timed_out("command timed out"), false),
        (CommandError::output("output failed"), false),
    ] {
        let temp = tempfile::tempdir().expect("tempdir");
        let executor = Arc::new(FailingActionExecutor {
            calls: AtomicUsize::new(0),
            error: command_error,
        });
        let driver = AgentBrowserDriver::with_executor(
            AgentBrowserConfig {
                command: BrowserCommand::Standalone {
                    executable: PathBuf::from("/opt/agent-browser"),
                },
                namespace: "retry".to_string(),
                headed: false,
                command_timeout: Duration::from_secs(5),
                idle_timeout: Duration::from_secs(2),
            },
            executor,
        );
        let mut session = driver
            .open(&ScenarioContext {
                run_id: "run".to_string(),
                scenario_id: "retry".to_string(),
                artifacts_dir: temp.path().join("artifacts"),
            })
            .await
            .expect("session");

        let error = session
            .execute(&step(
                "open",
                Action::Navigate {
                    url: "https://example.test".to_string(),
                },
            ))
            .await
            .expect_err("planned command failure");
        assert_eq!(error.retryable(), expected_retryable);
        session.close().await.expect("close");
    }
}

#[tokio::test]
async fn maps_extended_web_actions_and_records_evidence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let artifacts = temp.path().join("artifacts");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "extended".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "web".to_string(),
            artifacts_dir: artifacts.clone(),
        })
        .await
        .expect("session");

    let actions = [
        Action::Tab {
            operation: TabOperation::New {
                url: Some("https://example.test/docs".to_string()),
                label: Some("docs".to_string()),
            },
        },
        Action::Tab {
            operation: TabOperation::Switch {
                tab: "docs".to_string(),
            },
        },
        Action::Frame {
            target: FrameTarget::Selector("#payment".to_string()),
        },
        Action::Dialog {
            operation: DialogOperation::Accept {
                text: Some("approved".to_string()),
            },
        },
        Action::Upload {
            target: Target::Ref {
                value: "@e5".to_string(),
            },
            paths: vec!["one.txt".to_string(), "two.txt".to_string()],
        },
        Action::Download {
            target: Target::Css {
                selector: "#download".to_string(),
            },
            path: "downloads/report.pdf".to_string(),
        },
        Action::NetworkRoute {
            pattern: "**/api/users".to_string(),
            route: NetworkRoute::Body("{\"users\":[]}".to_string()),
        },
        Action::NetworkUnroute {
            pattern: Some("**/api/users".to_string()),
        },
        Action::Har {
            operation: CaptureOperation::Start,
        },
        Action::Har {
            operation: CaptureOperation::Stop {
                path: "network/session.har".to_string(),
            },
        },
        Action::Trace {
            operation: CaptureOperation::Start,
        },
        Action::Trace {
            operation: CaptureOperation::Stop {
                path: "traces/session.zip".to_string(),
            },
        },
        Action::Video {
            operation: VideoOperation::Start {
                path: "video/session.webm".to_string(),
                url: None,
            },
        },
        Action::Video {
            operation: VideoOperation::Stop,
        },
        Action::Accessibility {
            path: "evidence/tree.json".to_string(),
            interactive: true,
        },
        Action::Console {
            path: "evidence/console.json".to_string(),
            clear: true,
        },
        Action::PageErrors {
            path: "evidence/errors.json".to_string(),
            clear: false,
        },
    ];

    let mut outputs = Vec::new();
    for (index, action) in actions.into_iter().enumerate() {
        outputs.push(
            session
                .execute(&step(&format!("action-{index}"), action))
                .await
                .expect("action"),
        );
    }
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    let action_args = invocations
        .iter()
        .skip(1)
        .map(|invocation| strip_session_prefix(&invocation.args))
        .collect::<Vec<_>>();
    assert_eq!(
        action_args[0],
        os(&["tab", "new", "--label", "docs", "https://example.test/docs"])
    );
    assert_eq!(action_args[1], os(&["tab", "docs"]));
    assert_eq!(action_args[2], os(&["frame", "#payment"]));
    assert_eq!(action_args[3], os(&["dialog", "accept", "approved"]));
    assert_eq!(action_args[4], os(&["upload", "@e5", "one.txt", "two.txt"]));
    assert_eq!(
        action_args[5],
        vec![
            OsString::from("download"),
            OsString::from("#download"),
            artifacts.join("downloads/report.pdf").into_os_string(),
        ]
    );
    assert_eq!(
        action_args[6],
        os(&[
            "network",
            "route",
            "**/api/users",
            "--body",
            "{\"users\":[]}",
        ])
    );
    assert_eq!(action_args[7], os(&["network", "unroute", "**/api/users"]));
    assert_eq!(action_args[8], os(&["network", "har", "start"]));
    assert_eq!(
        action_args[9],
        vec![
            OsString::from("network"),
            OsString::from("har"),
            OsString::from("stop"),
            artifacts.join("network/session.har").into_os_string(),
        ]
    );
    assert_eq!(action_args[10], os(&["trace", "start"]));
    assert_eq!(
        action_args[11],
        vec![
            OsString::from("trace"),
            OsString::from("stop"),
            artifacts.join("traces/session.zip").into_os_string(),
        ]
    );
    assert_eq!(
        action_args[12],
        vec![
            OsString::from("record"),
            OsString::from("start"),
            artifacts.join("video/session.webm").into_os_string(),
        ]
    );
    assert_eq!(action_args[13], os(&["record", "stop"]));
    assert_eq!(action_args[14], os(&["snapshot", "-i"]));
    assert_eq!(action_args[15], os(&["console", "--clear"]));
    assert_eq!(action_args[16], os(&["errors"]));
    assert_eq!(action_args[17], os(&["close"]));

    assert_eq!(outputs[5].evidence[0].media_type, "application/pdf");
    assert_eq!(outputs[9].evidence[0].media_type, "application/json");
    assert_eq!(outputs[11].evidence[0].media_type, "application/zip");
    assert_eq!(outputs[13].evidence[0].media_type, "video/webm");
    assert!(artifacts.join("evidence/tree.json").is_file());
    assert!(artifacts.join("evidence/console.json").is_file());
    assert!(artifacts.join("evidence/errors.json").is_file());
}

#[tokio::test]
async fn rejects_artifact_parent_traversal_before_invoking_browser() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "isolated".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "ignored".to_string(),
            scenario_id: "home".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let error = session
        .execute(&step(
            "escape",
            Action::Screenshot {
                path: "../outside.png".to_string(),
            },
        ))
        .await
        .expect_err("path traversal must fail");
    assert_eq!(error.code(), "test.driver.web.artifact_path_invalid");
    assert_eq!(executor.invocations.lock().unwrap().len(), 1);
    session.close().await.expect("close");
}

#[tokio::test]
async fn dropped_session_schedules_emergency_close() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "isolated".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );
    let session = driver
        .open(&ScenarioContext {
            run_id: "ignored".to_string(),
            scenario_id: "home".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    drop(session);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if executor.invocations.lock().unwrap().len() >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("emergency close");

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(
        invocations[1].args,
        os(&["--session", "home", "--json", "close"])
    );
}

#[cfg(unix)]
#[tokio::test]
async fn failed_close_kills_the_exact_daemon_from_its_private_pid_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let artifacts = temp.path().join("artifacts");
    let executor = Arc::new(DaemonExecutor::default());

    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/bin/sleep"),
            },
            namespace: "run".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "home".to_string(),
            artifacts_dir: artifacts,
        })
        .await
        .expect("session");

    session.close().await.expect("forced cleanup");
    let mut daemon = executor.child.lock().unwrap().take().expect("fake daemon");
    let status = daemon.wait().expect("reap daemon");
    assert!(!status.success());
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn strip_session_prefix(args: &[OsString]) -> Vec<OsString> {
    args.iter().skip(3).cloned().collect()
}

fn assert_short_runtime(runtime: &OsString) {
    let path = PathBuf::from(runtime);
    assert_eq!(path.parent(), Some(std::path::Path::new("/tmp")));
    assert!(
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("a3st-")),
        "{}",
        path.display()
    );
}
