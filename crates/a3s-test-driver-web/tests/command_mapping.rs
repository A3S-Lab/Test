use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{
    Action, CaptureOperation, ContractFinding, ContractOutcome, ContractReport, ContractSeverity,
    DialogOperation, FrameTarget, NetworkRoute, ScenarioContext, SurfaceDriver, TabOperation,
    Target, TestStep, VideoOperation, WaitCondition,
};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, BrowserCommand,
    BrowserIntegration, BrowserNetworkPolicy, CommandError, CommandExecutor, CommandInvocation,
    CommandOutput, WebCapability,
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
    cleanup_error: Option<CommandError>,
}

struct PageContextExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
    revisions: Mutex<Vec<u64>>,
}

struct QualityProjectionExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
    accepted: bool,
}

impl RecordingExecutor {
    fn with_version(version: impl Into<String>) -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            version: Mutex::new(Some(version.into())),
        }
    }
}

impl PageContextExecutor {
    fn stable(revision: u64) -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            revisions: Mutex::new(vec![revision, revision]),
        }
    }

    fn changing() -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            revisions: Mutex::new(vec![1, 2, 3, 4]),
        }
    }
}

impl QualityProjectionExecutor {
    fn new(accepted: bool) -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            accepted,
        }
    }
}

fn materialize_external_artifact(invocation: &CommandInvocation) {
    let args = &invocation.args;
    let path = args.iter().enumerate().find_map(|(index, argument)| {
        let argument = argument.to_str()?;
        let offset = match argument {
            "screenshot" => Some(1),
            "download" => Some(2),
            "network"
                if args.get(index + 1).and_then(|value| value.to_str()) == Some("har")
                    && args.get(index + 2).and_then(|value| value.to_str()) == Some("stop") =>
            {
                Some(3)
            }
            "trace" if args.get(index + 1).and_then(|value| value.to_str()) == Some("stop") => {
                Some(2)
            }
            "record" if args.get(index + 1).and_then(|value| value.to_str()) == Some("start") => {
                Some(2)
            }
            _ => None,
        }?;
        args.get(index + offset).map(PathBuf::from)
    });
    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("fake artifact parent");
    }
    std::fs::write(path, b"fake-browser-artifact").expect("fake browser artifact");
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
        if !version_probe {
            materialize_external_artifact(&invocation);
        }
        let is_eval = invocation.args.iter().any(|argument| argument == "eval");
        let is_repair_watch = invocation
            .args
            .iter()
            .any(|argument| argument.to_string_lossy().contains("batchWindowMs"));
        self.invocations.lock().unwrap().push(invocation);
        Ok(CommandOutput {
            exit_code: 0,
            stdout: if version_probe {
                self.version
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| default_version.to_string())
            } else if is_repair_watch {
                r#"{"success":true,"data":{"result":[]}}"#.to_string()
            } else if is_eval {
                r#"{"success":true,"data":{"result":{"present":false}}}"#.to_string()
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
        if let Some(error) = &self.cleanup_error {
            return Err(error.clone());
        }
        Ok(CommandOutput {
            exit_code: 0,
            stdout: r#"{"success":true}"#.to_string(),
            stderr: String::new(),
        })
    }
}

#[async_trait]
impl CommandExecutor for PageContextExecutor {
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
            let revision = self.revisions.lock().unwrap().remove(0);
            page_context_response(revision)
        } else {
            r#"{"success":true,"data":{"snapshot":"accessibility"}}"#.to_string()
        };
        Ok(CommandOutput {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        })
    }
}

#[async_trait]
impl CommandExecutor for QualityProjectionExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version_probe = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        self.invocations.lock().unwrap().push(invocation);
        Ok(CommandOutput {
            exit_code: 0,
            stdout: if version_probe {
                "agent-browser 0.26.0".to_string()
            } else {
                serde_json::json!({
                    "success": true,
                    "data": { "result": self.accepted }
                })
                .to_string()
            },
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
        network_policy: BrowserNetworkPolicy::restricted_to_domains([
            "example.test",
            "*.cdn.example.test",
        ])
        .expect("network policy"),
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
            "--headed",
            "false",
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
            "--headed",
            "false",
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
        os(&[
            "use",
            "browser",
            "--session",
            "word",
            "--json",
            "--headed",
            "false",
            "close",
        ])
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
                OsString::from("A3S_USE_BROWSER_ARGS"),
                expected_headless_args(&["A3S_USE_BROWSER_ARGS", "AGENT_BROWSER_ARGS"])
            ),
            (
                OsString::from("A3S_USE_BROWSER_SOCKET_DIR"),
                runtime.clone()
            ),
            (
                OsString::from("A3S_USE_BROWSER_ALLOWED_DOMAINS"),
                OsString::from("*.cdn.example.test,example.test")
            ),
        ])
    );
}

#[tokio::test]
async fn standalone_driver_uses_the_upstream_policy_contract_and_a_safe_idle_floor() {
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
            network_policy: BrowserNetworkPolicy::restricted_to_domains(["127.0.0.1"])
                .expect("network policy"),
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

    session
        .execute(&step(
            "open",
            Action::Navigate {
                url: "http://127.0.0.1/".to_string(),
            },
        ))
        .await
        .expect("navigate");
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(
        invocations[1].args,
        os(&[
            "--session",
            "home",
            "--json",
            "--headed",
            "false",
            "--allowed-domains",
            "127.0.0.1",
            "--engine",
            "chrome",
            "open",
            "http://127.0.0.1/",
        ])
    );
    assert_eq!(
        invocations[2].args,
        os(&["--session", "home", "--json", "--headed", "false", "close",])
    );
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
                OsString::from("5000")
            ),
            (
                OsString::from("AGENT_BROWSER_ARGS"),
                expected_headless_args(&["AGENT_BROWSER_ARGS"])
            ),
            (OsString::from("AGENT_BROWSER_SOCKET_DIR"), runtime.clone()),
            (
                OsString::from("AGENT_BROWSER_ALLOWED_DOMAINS"),
                OsString::from("127.0.0.1")
            ),
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
            network_policy: Default::default(),
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
    assert_eq!(
        observation.page_context.as_ref().map(|value| value.present),
        Some(false)
    );
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(
        invocations[2].args,
        os(&[
            "--session",
            "agent",
            "--json",
            "--headed",
            "false",
            "snapshot",
        ])
    );
    assert_eq!(strip_session_prefix(&invocations[1].args)[0], "eval");
    assert_eq!(strip_session_prefix(&invocations[3].args)[0], "eval");
}

#[tokio::test]
async fn captures_a_typed_testkit_context_at_the_same_stable_revision() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(PageContextExecutor::stable(7));
    let driver = AgentBrowserDriver::with_executor(standalone_config("page-context"), executor);
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "context".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let observation = session.observe().await.expect("observation");
    let page_context = observation.page_context.expect("page context");
    assert!(page_context.present);
    assert_eq!(page_context.revision, Some(7));
    let snapshot = page_context.snapshot.expect("typed snapshot");
    assert_eq!(snapshot.page.expect("page").route, "/checkout");
    assert_eq!(snapshot.nodes[0].role.as_deref(), Some("button"));
    assert_eq!(
        snapshot.nodes[0].locators[0],
        a3s_test_core::PageContextLocator::TestId {
            value: "pay".to_string()
        }
    );
}

#[tokio::test]
async fn projects_a_bounded_quality_report_through_the_testkit_bridge() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(QualityProjectionExecutor::new(true));
    let driver =
        AgentBrowserDriver::with_executor(standalone_config("quality-report"), executor.clone());
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "quality".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let accepted = session
        .project_quality_report(&quality_report())
        .await
        .expect("quality projection");

    assert!(accepted);
    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(invocations.len(), 2);
    let args = strip_session_prefix(&invocations[1].args);
    assert_eq!(args[0], "eval");
    let script = args[1].to_string_lossy();
    assert!(
        script.contains("Symbol.for(\"a3s.test.page-context\")"),
        "{script}"
    );
    assert!(
        script.contains("typeof bridge.reportQuality !== \"function\""),
        "{script}"
    );
    assert!(
        script.contains("bridge.reportQuality(report) === true"),
        "{script}"
    );
    assert!(
        script.contains("finding:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        "{script}"
    );
    assert!(script.contains("contract.element.role"), "{script}");
}

#[tokio::test]
async fn missing_testkit_bridge_declines_quality_projection_without_an_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(QualityProjectionExecutor::new(false));
    let driver = AgentBrowserDriver::with_executor(
        standalone_config("quality-report-missing"),
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "quality-missing".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let accepted = session
        .project_quality_report(&quality_report())
        .await
        .expect("missing bridge is optional");

    assert!(!accepted);
    assert_eq!(executor.invocations.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn bounds_repair_watch_below_the_browser_command_deadline() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::with_version("agent-browser 0.26.0"));
    let mut config = standalone_config("repair-watch");
    config.command_timeout = Duration::from_millis(1_000);
    let driver = AgentBrowserDriver::with_executor(config, executor.clone());
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "watch".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let repairs = session
        .wait_for_repairs(7, 30_000, 250)
        .await
        .expect("watch");
    assert!(repairs.is_empty());
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    let script = strip_session_prefix(&invocations[1].args)[1]
        .to_string_lossy()
        .into_owned();
    assert!(script.contains("limit: 7"), "{script}");
    assert!(script.contains("timeoutMs: 900"), "{script}");
    assert!(script.contains("batchWindowMs: 250"), "{script}");
}

#[tokio::test]
async fn rejects_context_that_keeps_changing_during_observation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(PageContextExecutor::changing());
    let driver =
        AgentBrowserDriver::with_executor(standalone_config("page-context-race"), executor);
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "changing".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let error = session.observe().await.expect_err("unstable context");
    assert_eq!(error.code(), "test.driver.web.page_context_changed");
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
            network_policy: Default::default(),
        },
        executor.clone(),
    );

    let capabilities = driver.capabilities().await.expect("capabilities");
    assert_eq!(capabilities.integration, BrowserIntegration::Standalone);
    assert_eq!(capabilities.version, "0.26.0");
    assert_eq!(capabilities.protocol_revision, 6);
    assert_eq!(capabilities.page_context_protocol, None);
    assert!(capabilities.features.contains(&WebCapability::Tabs));
    assert!(capabilities.features.contains(&WebCapability::Har));
    assert!(capabilities.features.contains(&WebCapability::Video));
    assert!(capabilities
        .features
        .contains(&WebCapability::ContextClicks));
    assert!(capabilities
        .features
        .contains(&WebCapability::DomainContainment));
    assert!(capabilities.features.contains(&WebCapability::MouseWheel));
    assert!(capabilities.features.contains(&WebCapability::Viewport));

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
            network_policy: Default::default(),
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
            cleanup_error: None,
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
                network_policy: Default::default(),
            },
            executor.clone(),
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
        assert_eq!(
            executor.calls.load(Ordering::SeqCst),
            3,
            "a failed initial command must dispatch one exact cleanup command"
        );
    }
}

#[tokio::test]
async fn failed_initial_command_does_not_mask_failed_exact_cleanup() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(FailingActionExecutor {
        calls: AtomicUsize::new(0),
        error: CommandError::output("initial browser command failed"),
        cleanup_error: Some(CommandError::timed_out("exact close timed out")),
    });
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "failed-cleanup".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
            network_policy: Default::default(),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "failed-cleanup".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    session
        .execute(&step(
            "open",
            Action::Navigate {
                url: "https://example.test".to_string(),
            },
        ))
        .await
        .expect_err("planned initial command failure");
    let cleanup_error = session
        .close()
        .await
        .expect_err("failed exact cleanup must be reported");

    assert_eq!(cleanup_error.code(), "test.driver.web.command_unavailable");
    assert_eq!(executor.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn maps_extended_web_actions_and_records_evidence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let artifacts = temp.path().join("artifacts");
    let absolute_upload = temp.path().join("two.txt");
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
            network_policy: Default::default(),
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
            paths: vec![
                "one.txt".to_string(),
                absolute_upload.to_string_lossy().into_owned(),
            ],
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
        Action::Wait {
            condition: WaitCondition::Visible(Target::Css {
                selector: "[data-editor-ready]".to_string(),
            }),
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
    let canonical_artifacts = canonical_test_path(&artifacts);
    let action_args = invocations
        .iter()
        .skip(1)
        .map(|invocation| strip_session_prefix(&invocation.args))
        .collect::<Vec<_>>();
    let working_directory = std::env::current_dir().expect("working directory");
    assert_eq!(
        action_args[0],
        os(&["tab", "new", "--label", "docs", "https://example.test/docs"])
    );
    assert_eq!(action_args[1], os(&["tab", "docs"]));
    assert_eq!(action_args[2], os(&["frame", "#payment"]));
    assert_eq!(action_args[3], os(&["dialog", "accept", "approved"]));
    assert_eq!(
        action_args[4],
        vec![
            OsString::from("upload"),
            OsString::from("@e5"),
            working_directory.join("one.txt").into_os_string(),
            absolute_upload.into_os_string(),
        ]
    );
    assert_eq!(
        action_args[5],
        vec![
            OsString::from("download"),
            OsString::from("#download"),
            canonical_artifacts
                .join("downloads")
                .join("report.pdf")
                .into_os_string(),
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
            canonical_artifacts
                .join("network")
                .join("session.har")
                .into_os_string(),
        ]
    );
    assert_eq!(action_args[10], os(&["trace", "start"]));
    assert_eq!(
        action_args[11],
        vec![
            OsString::from("trace"),
            OsString::from("stop"),
            canonical_artifacts
                .join("traces")
                .join("session.zip")
                .into_os_string(),
        ]
    );
    assert_eq!(
        action_args[12],
        vec![
            OsString::from("record"),
            OsString::from("start"),
            canonical_artifacts
                .join("video")
                .join("session.webm")
                .into_os_string(),
        ]
    );
    assert_eq!(action_args[13], os(&["record", "stop"]));
    assert_eq!(action_args[14], os(&["snapshot", "-i"]));
    assert_eq!(action_args[15], os(&["console", "--clear"]));
    assert_eq!(action_args[16], os(&["errors"]));
    assert_eq!(action_args[17], os(&["wait", "[data-editor-ready]"]));
    assert_eq!(action_args[18], os(&["close"]));

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
            network_policy: Default::default(),
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
            network_policy: Default::default(),
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
        os(&["--session", "home", "--json", "--headed", "false", "close",])
    );
}

#[tokio::test]
async fn persistent_connection_survives_handle_drop_until_explicit_close() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "interactive".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
            network_policy: Default::default(),
        },
        executor.clone(),
    );
    let connection = AgentBrowserConnectionConfig {
        namespace: "interactive".to_string(),
        session: "agent-checkout".to_string(),
        runtime_dir: temp.path().join("runtime"),
        artifacts_dir: temp.path().join("artifacts"),
        active_video_path: None,
    };

    let mut first = driver.connect(connection.clone()).await.expect("connect");
    first
        .execute_action(
            "open",
            Action::Navigate {
                url: "https://example.test".to_string(),
            },
        )
        .await
        .expect("navigate");
    drop(first);

    {
        let invocations = executor.invocations.lock().unwrap();
        assert_eq!(invocations.len(), 2);
        assert_eq!(
            strip_session_prefix(&invocations[1].args),
            os(&["open", "https://example.test"])
        );
    }

    let mut second = driver.connect(connection).await.expect("reconnect");
    second.observe_surface().await.expect("observe");
    second.close_surface().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(invocations.len(), 6);
    assert_eq!(
        strip_session_prefix(&invocations[3].args),
        os(&["snapshot"])
    );
    assert_eq!(strip_session_prefix(&invocations[2].args)[0], "eval");
    assert_eq!(strip_session_prefix(&invocations[4].args)[0], "eval");
    assert_eq!(strip_session_prefix(&invocations[5].args), os(&["close"]));
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
            network_policy: Default::default(),
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

fn standalone_config(namespace: &str) -> AgentBrowserConfig {
    AgentBrowserConfig {
        command: BrowserCommand::Standalone {
            executable: PathBuf::from("/opt/agent-browser"),
        },
        namespace: namespace.to_string(),
        headed: false,
        command_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(30),
        network_policy: Default::default(),
    }
}

fn page_context_response(revision: u64) -> String {
    serde_json::json!({
        "success": true,
        "data": {
            "result": {
                "present": true,
                "protocol": "a3s.test.page-context/1",
                "sdkVersion": "0.1.0",
                "revision": revision,
                "page": {
                    "id": "checkout",
                    "url": "http://127.0.0.1/checkout",
                    "route": "/checkout",
                    "title": "Checkout",
                    "ready": true,
                    "viewport": { "width": 1280.0, "height": 720.0, "dpr": 2.0 },
                    "document": { "width": 1280.0, "height": 900.0 },
                    "scroll": { "x": 0.0, "y": 0.0 },
                    "language": "en",
                    "theme": "light"
                },
                "components": [],
                "nodes": [{
                    "id": "n1",
                    "tag": "button",
                    "role": "button",
                    "name": "Pay",
                    "text": "Pay",
                    "testId": "pay",
                    "geometry": {
                        "viewport": { "x": 10.0, "y": 20.0, "width": 100.0, "height": 40.0 },
                        "document": { "x": 10.0, "y": 20.0, "width": 100.0, "height": 40.0 },
                        "normalized": { "x": 0.01, "y": 0.03, "width": 0.08, "height": 0.06 },
                        "visibleRatio": 1.0,
                        "occluded": false,
                        "position": "static",
                        "transformed": false
                    },
                    "state": { "visible": true, "focused": false },
                    "locators": [{ "type": "test_id", "value": "pay" }]
                }],
                "facts": {},
                "removedNodeIds": [],
                "truncated": false,
                "nextCursor": null
            }
        }
    })
    .to_string()
}

fn quality_report() -> ContractReport {
    ContractReport {
        contract: "checkout".to_string(),
        variant: "desktop".to_string(),
        state: "ready".to_string(),
        outcome: ContractOutcome::Failed,
        observation_revision: Some(7),
        matches: Vec::new(),
        findings: vec![ContractFinding {
            id: "finding:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            dimension: "semantics".to_string(),
            rule_id: "contract.element.role".to_string(),
            severity: ContractSeverity::Blocking,
            message: "the observed role does not match the contract".to_string(),
            expected: serde_json::json!("button"),
            actual: serde_json::json!("link"),
            element_id: Some("submit".to_string()),
            observed_node_id: Some("n1".to_string()),
            confidence: 100,
        }],
    }
}

fn strip_session_prefix(args: &[OsString]) -> Vec<OsString> {
    args.iter().skip(5).cloned().collect()
}

fn expected_headless_args(names: &[&str]) -> OsString {
    let mut arguments = names
        .iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_default();
    if !arguments.is_empty() {
        arguments.push(',');
    }
    arguments.push_str("--headless=new");
    OsString::from(arguments)
}

fn assert_short_runtime(runtime: &OsString) {
    let path = PathBuf::from(runtime);
    #[cfg(unix)]
    let expected_parent = canonical_test_path(std::path::Path::new("/tmp"));
    #[cfg(not(unix))]
    let expected_parent = canonical_test_path(&std::env::temp_dir());
    assert_eq!(path.parent(), Some(expected_parent.as_path()));
    assert!(
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("a3st-")),
        "{}",
        path.display()
    );
}

fn canonical_test_path(path: &std::path::Path) -> PathBuf {
    let canonical = path.canonicalize().expect("canonical test path");
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};

        const VERBATIM_PREFIX: &[u16] = &[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
        const UNC_PREFIX: &[u16] = &[b'U' as u16, b'N' as u16, b'C' as u16, b'\\' as u16];

        let wide = canonical.as_os_str().encode_wide().collect::<Vec<_>>();
        if wide.starts_with(VERBATIM_PREFIX) {
            if wide[VERBATIM_PREFIX.len()..].starts_with(UNC_PREFIX) {
                let mut normalized = vec![b'\\' as u16, b'\\' as u16];
                normalized.extend_from_slice(&wide[VERBATIM_PREFIX.len() + UNC_PREFIX.len()..]);
                return PathBuf::from(OsString::from_wide(&normalized));
            }
            return PathBuf::from(OsString::from_wide(&wide[VERBATIM_PREFIX.len()..]));
        }
    }
    canonical
}
