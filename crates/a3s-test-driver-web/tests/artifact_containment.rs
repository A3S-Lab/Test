use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{Action, ScenarioContext, SurfaceDriver, TestStep, VideoOperation};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, BrowserCommand,
    CommandError, CommandExecutor, CommandInvocation, CommandOutput,
};
use async_trait::async_trait;

#[derive(Default)]
struct ArtifactExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
    materialize_outputs: bool,
    artifact_size: Option<u64>,
}

impl ArtifactExecutor {
    fn materializing() -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            materialize_outputs: true,
            artifact_size: None,
        }
    }

    fn with_artifact_size(artifact_size: u64) -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            materialize_outputs: true,
            artifact_size: Some(artifact_size),
        }
    }

    fn invocation_count(&self) -> usize {
        self.invocations.lock().unwrap().len()
    }
}

#[async_trait]
impl CommandExecutor for ArtifactExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version_probe = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        if self.materialize_outputs && !version_probe {
            if let Some(path) = output_path(&invocation.args) {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).expect("fake artifact parent");
                }
                if let Some(size) = self.artifact_size {
                    let artifact = std::fs::File::create(path).expect("fake artifact");
                    artifact.set_len(size).expect("size fake artifact");
                } else {
                    std::fs::write(path, b"fake-browser-artifact").expect("fake artifact");
                }
            }
        }
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

fn output_path(args: &[OsString]) -> Option<PathBuf> {
    for (index, argument) in args.iter().enumerate() {
        match argument.to_str()? {
            "screenshot" => return args.get(index + 1).map(PathBuf::from),
            "record" if args.get(index + 1).and_then(|value| value.to_str()) == Some("start") => {
                return args.get(index + 2).map(PathBuf::from);
            }
            _ => {}
        }
    }
    None
}

fn driver(executor: Arc<ArtifactExecutor>) -> AgentBrowserDriver {
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

fn context(temp: &tempfile::TempDir) -> ScenarioContext {
    ScenarioContext {
        run_id: "run".to_string(),
        scenario_id: "web".to_string(),
        artifacts_dir: temp.path().join("artifacts"),
    }
}

fn screenshot(path: &str) -> TestStep {
    TestStep {
        id: "screenshot".to_string(),
        action: Action::Screenshot {
            path: path.to_string(),
        },
        stability: None,
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

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

fn unavailable_without_host_privilege(error: &std::io::Error) -> bool {
    cfg!(windows)
        && matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        )
}

#[tokio::test]
async fn directory_link_escape_is_rejected_before_browser_dispatch_or_write() {
    let temp = tempfile::tempdir().expect("temp dir");
    let context = context(&temp);
    let outside = temp.path().join("outside");
    tokio::fs::create_dir_all(&outside)
        .await
        .expect("outside directory");
    let executor = Arc::new(ArtifactExecutor::materializing());
    let mut session = driver(Arc::clone(&executor))
        .open(&context)
        .await
        .expect("session");
    if let Err(error) = symlink_directory(&outside, &context.artifacts_dir.join("escape")) {
        if unavailable_without_host_privilege(&error) {
            session.close().await.expect("close skipped session");
            return;
        }
        panic!("failed to create directory link: {error}");
    }

    let error = session
        .execute(&screenshot("escape/nested/page.png"))
        .await
        .expect_err("directory link must be rejected");

    assert_eq!(error.code(), "test.driver.web.artifact_path_invalid");
    assert_eq!(executor.invocation_count(), 1);
    assert!(!outside.join("nested").exists());
    session.close().await.expect("close session");
}

#[tokio::test]
async fn existing_file_link_is_rejected_without_overwriting_its_target() {
    let temp = tempfile::tempdir().expect("temp dir");
    let context = context(&temp);
    let outside = temp.path().join("outside.png");
    tokio::fs::write(&outside, b"do-not-overwrite")
        .await
        .expect("outside file");
    let executor = Arc::new(ArtifactExecutor::materializing());
    let mut session = driver(Arc::clone(&executor))
        .open(&context)
        .await
        .expect("session");
    if let Err(error) = symlink_file(&outside, &context.artifacts_dir.join("linked.png")) {
        if unavailable_without_host_privilege(&error) {
            session.close().await.expect("close skipped session");
            return;
        }
        panic!("failed to create file link: {error}");
    }

    let error = session
        .execute(&screenshot("linked.png"))
        .await
        .expect_err("file link must be rejected");

    assert_eq!(error.code(), "test.driver.web.artifact_path_invalid");
    assert_eq!(executor.invocation_count(), 1);
    assert_eq!(
        tokio::fs::read(&outside).await.expect("outside contents"),
        b"do-not-overwrite"
    );
    session.close().await.expect("close session");
}

#[tokio::test]
async fn successful_browser_command_without_an_artifact_is_rejected() {
    let temp = tempfile::tempdir().expect("temp dir");
    let context = context(&temp);
    let executor = Arc::new(ArtifactExecutor::default());
    let mut session = driver(Arc::clone(&executor))
        .open(&context)
        .await
        .expect("session");
    let stale = context.artifacts_dir.join("missing.png");
    tokio::fs::write(&stale, b"stale-output")
        .await
        .expect("stale output");

    let error = session
        .execute(&screenshot("missing.png"))
        .await
        .expect_err("missing browser output must be rejected");

    assert_eq!(error.code(), "test.driver.web.artifact_output_invalid");
    assert_eq!(executor.invocation_count(), 2);
    assert!(!stale.exists());
    session.close().await.expect("close session");
}

#[tokio::test]
async fn oversized_screenshot_is_rejected_and_removed_after_browser_dispatch() {
    const OVERSIZED_SCREENSHOT_BYTES: u64 = 32 * 1_024 * 1_024 + 1;

    let temp = tempfile::tempdir().expect("temp dir");
    let context = context(&temp);
    let executor = Arc::new(ArtifactExecutor::with_artifact_size(
        OVERSIZED_SCREENSHOT_BYTES,
    ));
    let mut session = driver(Arc::clone(&executor))
        .open(&context)
        .await
        .expect("session");
    let path = context.artifacts_dir.join("oversized.png");

    let error = session
        .execute(&screenshot("oversized.png"))
        .await
        .expect_err("oversized screenshot must be rejected");

    assert_eq!(error.code(), "test.driver.web.artifact_output_invalid");
    assert_eq!(executor.invocation_count(), 2);
    assert!(!path.exists(), "invalid screenshot must not be retained");
    session.close().await.expect("close session");
}

#[tokio::test]
async fn empty_screenshot_is_rejected_and_removed_after_browser_dispatch() {
    let temp = tempfile::tempdir().expect("temp dir");
    let context = context(&temp);
    let executor = Arc::new(ArtifactExecutor::with_artifact_size(0));
    let mut session = driver(Arc::clone(&executor))
        .open(&context)
        .await
        .expect("session");
    let path = context.artifacts_dir.join("empty.png");

    let error = session
        .execute(&screenshot("empty.png"))
        .await
        .expect_err("empty screenshot must be rejected");

    assert_eq!(error.code(), "test.driver.web.artifact_output_invalid");
    assert_eq!(executor.invocation_count(), 2);
    assert!(!path.exists(), "invalid screenshot must not be retained");
    session.close().await.expect("close session");
}

#[tokio::test]
async fn reconnect_admits_but_does_not_delete_an_active_video_file() {
    let temp = tempfile::tempdir().expect("temp dir");
    let artifacts = temp.path().join("artifacts");
    let video = artifacts.join("video/session.webm");
    tokio::fs::create_dir_all(video.parent().expect("video parent"))
        .await
        .expect("video directory");
    tokio::fs::write(&video, b"in-progress-video")
        .await
        .expect("active video");
    let executor = Arc::new(ArtifactExecutor::default());
    let browser = driver(Arc::clone(&executor));

    let mut session = browser
        .connect(AgentBrowserConnectionConfig {
            namespace: "contained".to_string(),
            session: "web".to_string(),
            runtime_dir: temp.path().join("runtime"),
            artifacts_dir: artifacts,
            active_video_path: Some("video/session.webm".to_string()),
        })
        .await
        .expect("persistent session");

    assert_eq!(session.active_video_path(), Some("video/session.webm"));
    assert_eq!(
        tokio::fs::read(&video).await.expect("preserved video"),
        b"in-progress-video"
    );
    session.close_surface().await.expect("close session");
}

#[tokio::test]
async fn linked_video_output_is_rejected_when_recording_stops() {
    let temp = tempfile::tempdir().expect("temp dir");
    let context = context(&temp);
    let executor = Arc::new(ArtifactExecutor::materializing());
    let mut session = driver(Arc::clone(&executor))
        .open(&context)
        .await
        .expect("session");
    session
        .execute(&TestStep {
            id: "video-start".to_string(),
            action: Action::Video {
                operation: VideoOperation::Start {
                    path: "video/session.webm".to_string(),
                    url: None,
                },
            },
            stability: None,
        })
        .await
        .expect("video start");
    let video = context.artifacts_dir.join("video/session.webm");
    let outside = temp.path().join("outside-video.webm");
    tokio::fs::write(&outside, b"fake-browser-artifact")
        .await
        .expect("outside video");
    tokio::fs::remove_file(&video)
        .await
        .expect("remove video output");
    if let Err(error) = symlink_file(&outside, &video) {
        if unavailable_without_host_privilege(&error) {
            session.close().await.expect("close skipped session");
            return;
        }
        panic!("failed to link video output: {error}");
    }

    let error = session
        .execute(&TestStep {
            id: "video-stop".to_string(),
            action: Action::Video {
                operation: VideoOperation::Stop,
            },
            stability: None,
        })
        .await
        .expect_err("linked video output must be rejected");

    assert_eq!(error.code(), "test.driver.web.artifact_output_invalid");
    session.close().await.expect("close session");
}
