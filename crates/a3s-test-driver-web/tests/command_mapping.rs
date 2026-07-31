use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{Action, ScenarioContext, SurfaceDriver, Target, TestStep};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserDriver, BrowserCommand, CommandExecutor, CommandInvocation,
    CommandOutput,
};
use async_trait::async_trait;

#[derive(Default)]
struct RecordingExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
}

#[cfg(unix)]
#[derive(Default)]
struct DaemonExecutor {
    child: Mutex<Option<std::process::Child>>,
}

#[async_trait]
impl CommandExecutor for RecordingExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, String> {
        self.invocations.lock().unwrap().push(invocation);
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
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, String> {
        use std::fs;
        use std::os::unix::process::CommandExt;
        use std::process::Command;

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
        Err("driver is unresponsive".to_string())
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
    assert_eq!(invocations.len(), 3);
    assert_eq!(invocations[0].program, PathBuf::from("/opt/a3s"));
    assert_eq!(
        invocations[0].args,
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
        invocations[1].args,
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
        invocations[2].args,
        os(&["use", "browser", "--session", "word", "--json", "close"])
    );
    let runtime = invocations[0]
        .env
        .get(&OsString::from("A3S_USE_BROWSER_SOCKET_DIR"))
        .expect("runtime");
    assert_short_runtime(runtime);
    assert_eq!(
        invocations[0].env,
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
    let runtime = invocations[0]
        .env
        .get(&OsString::from("AGENT_BROWSER_SOCKET_DIR"))
        .expect("runtime");
    assert_short_runtime(runtime);
    assert_eq!(
        invocations[0].env,
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
    assert!(executor.invocations.lock().unwrap().is_empty());
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
            if !executor.invocations.lock().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("emergency close");

    let invocations = executor.invocations.lock().unwrap();
    assert_eq!(
        invocations[0].args,
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
