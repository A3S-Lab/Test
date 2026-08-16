use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::Action;
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, BrowserCommand,
    CommandError, CommandExecutor, CommandInvocation, CommandOutput,
};
use async_trait::async_trait;

#[derive(Default)]
struct RecordingExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
}

impl RecordingExecutor {
    fn invocation_count(&self) -> usize {
        self.invocations.lock().unwrap().len()
    }
}

#[async_trait]
impl CommandExecutor for RecordingExecutor {
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
                r#"{"success":true}"#.to_string()
            },
            stderr: String::new(),
        })
    }
}

fn driver(executor: Arc<RecordingExecutor>) -> AgentBrowserDriver {
    AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "contained".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(5),
            microphone: Default::default(),
            network_policy: Default::default(),
        },
        executor,
    )
}

fn connection(temp: &tempfile::TempDir, runtime_dir: PathBuf) -> AgentBrowserConnectionConfig {
    AgentBrowserConnectionConfig {
        namespace: "contained".to_string(),
        session: "agent-runtime".to_string(),
        runtime_dir,
        artifacts_dir: temp.path().join("artifacts"),
        active_video_path: None,
    }
}

#[cfg(unix)]
fn symlink_directory(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_directory(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

fn unavailable_without_host_privilege(error: &std::io::Error) -> bool {
    cfg!(windows)
        && matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        )
}

#[tokio::test]
async fn persistent_connection_rejects_a_linked_runtime_before_dispatch() {
    let temp = tempfile::tempdir().expect("temp dir");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).expect("outside runtime");
    let runtime = temp.path().join("runtime");
    if let Err(error) = symlink_directory(&outside, &runtime) {
        if unavailable_without_host_privilege(&error) {
            return;
        }
        panic!("failed to create runtime link: {error}");
    }
    let executor = Arc::new(RecordingExecutor::default());

    let error = match driver(Arc::clone(&executor))
        .connect(connection(&temp, runtime))
        .await
    {
        Ok(_) => panic!("linked runtime must be rejected"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "test.driver.web.runtime_path_invalid");
    assert_eq!(executor.invocation_count(), 0);
}

#[tokio::test]
async fn runtime_link_replacement_is_rejected_before_action_dispatch() {
    let temp = tempfile::tempdir().expect("temp dir");
    let runtime = temp.path().join("runtime");
    let displaced = temp.path().join("original-runtime");
    let outside = temp.path().join("outside");
    let executor = Arc::new(RecordingExecutor::default());
    let mut session = driver(Arc::clone(&executor))
        .connect(connection(&temp, runtime.clone()))
        .await
        .expect("connect");
    std::fs::rename(&runtime, &displaced).expect("displace runtime");
    std::fs::create_dir(&outside).expect("outside runtime");
    if let Err(error) = symlink_directory(&outside, &runtime) {
        if unavailable_without_host_privilege(&error) {
            return;
        }
        panic!("failed to replace runtime with link: {error}");
    }

    let error = session
        .execute_action(
            "open",
            Action::Navigate {
                url: "https://example.test".to_string(),
            },
        )
        .await
        .expect_err("runtime replacement must be rejected");

    assert_eq!(error.code(), "test.driver.web.runtime_binding_lost");
    assert_eq!(executor.invocation_count(), 1);
}

#[tokio::test]
async fn same_path_runtime_replacement_is_rejected_before_close_dispatch() {
    let temp = tempfile::tempdir().expect("temp dir");
    let runtime = temp.path().join("runtime");
    let displaced = temp.path().join("original-runtime");
    let executor = Arc::new(RecordingExecutor::default());
    let mut session = driver(Arc::clone(&executor))
        .connect(connection(&temp, runtime.clone()))
        .await
        .expect("connect");
    std::fs::rename(&runtime, &displaced).expect("displace runtime");
    std::fs::create_dir(&runtime).expect("replacement runtime");

    let error = session
        .close_surface()
        .await
        .expect_err("replacement runtime must be rejected");

    assert_eq!(error.code(), "test.driver.web.runtime_binding_lost");
    assert_eq!(executor.invocation_count(), 1);
}
