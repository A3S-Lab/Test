use std::path::{Path, PathBuf};

use a3s_test_core::DriverError;
use a3s_test_driver_web::TestKitHandshake;
use anyhow::Result;
use serde::Serialize;

use super::args::StartArgs;
use super::browser::{connect, BrowserConnectionPurpose};
use super::store::AgentSessionStore;
use super::{abort_session, start_session, unix_ms};
use crate::{BrowserDriverKind, BrowserMicrophoneArg};

pub(crate) struct DevSessionRequest {
    pub(crate) workspace: PathBuf,
    pub(crate) url: String,
    pub(crate) session_prefix: String,
    pub(crate) browser_driver: BrowserDriverKind,
    pub(crate) browser_executable: Option<PathBuf>,
    pub(crate) headed: bool,
    pub(crate) command_timeout_ms: u64,
    pub(crate) idle_timeout_ms: u64,
    pub(crate) testkit_required: bool,
    pub(crate) testkit_install_command: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DevSession {
    pub(crate) session: String,
    pub(crate) artifacts_dir: PathBuf,
    pub(crate) testkit: Option<TestKitHandshake>,
}

pub(crate) async fn start_dev_session(request: DevSessionRequest) -> Result<DevSession> {
    let session = next_dev_session_id(&request.workspace, &request.session_prefix)?;
    let result = start_session(
        StartArgs {
            url: request.url,
            session: session.clone(),
            goal: "Process explicitly submitted Web review findings".to_string(),
            success_criteria: vec![
                "Every claimed finding reaches a verified, clarified, failed, or cancelled state"
                    .to_string(),
            ],
            auto_resolve_repairs: false,
            allowed_origins: Vec::new(),
            allowed_domains: Vec::new(),
            browser_driver: request.browser_driver,
            browser_executable: request.browser_executable,
            browser_microphone: BrowserMicrophoneArg::Disabled,
            headed: request.headed,
            command_timeout_ms: request.command_timeout_ms,
            idle_timeout_ms: request.idle_timeout_ms,
            json: false,
        },
        Some(&request.workspace),
    )
    .await?;

    let handshake = live_testkit_handshake(
        &result.state,
        request.testkit_required,
        &request.testkit_install_command,
    )
    .await;
    let testkit = match handshake {
        Ok(handshake) => handshake,
        Err(error) => {
            let cleanup = abort_session(&session, Some(&request.workspace)).await;
            return match cleanup {
                Ok(result) => match result.cleanup_error {
                    None => Err(error.context("live Test Kit handshake failed")),
                    Some(cleanup_error) => Err(error.context(format!(
                        "live Test Kit handshake failed and exact browser cleanup also failed: {}",
                        cleanup_error.message()
                    ))),
                },
                Err(cleanup) => Err(error.context(format!(
                    "live Test Kit handshake failed and browser cleanup could not start: {cleanup:#}"
                ))),
            };
        }
    };

    Ok(DevSession {
        session,
        artifacts_dir: result.state.artifacts_dir,
        testkit,
    })
}

pub(crate) async fn abort_dev_session(workspace: &Path, session: &str) -> Result<()> {
    let result = abort_session(session, Some(workspace)).await?;
    match result.cleanup_error {
        Some(error) => Err(anyhow::Error::new(error)),
        None => Ok(()),
    }
}

async fn live_testkit_handshake(
    state: &super::store::AgentSessionState,
    required: bool,
    install_command: &str,
) -> Result<Option<TestKitHandshake>> {
    let mut browser = connect(state, BrowserConnectionPurpose::Turn).await?;
    let handshake = browser
        .testkit_handshake(required)
        .await
        .map_err(|error| exact_testkit_repair(error, install_command))?;
    if required && handshake.is_none() {
        return Err(exact_testkit_repair(
            DriverError::new(
                "test.driver.web.testkit_bridge_missing",
                "the page does not expose the required live Test Kit bridge",
            ),
            install_command,
        ));
    }
    Ok(handshake)
}

fn exact_testkit_repair(error: DriverError, install_command: &str) -> anyhow::Error {
    let repair = if error.code() == "test.driver.web.testkit_review_overlay_missing" {
        "render <A3SReviewOverlay /> inside <A3STestKit> on the development page".to_string()
    } else {
        format!("run `{install_command}`, then mount <A3STestKit> on the development page")
    };
    anyhow::Error::new(DriverError::new(
        error.code().to_string(),
        format!("{}; repair: {repair}", error.message()),
    ))
}

fn next_dev_session_id(workspace: &Path, prefix: &str) -> Result<String> {
    let direct = AgentSessionStore::for_workspace(workspace, prefix);
    if !direct.exists() {
        return Ok(prefix.to_string());
    }
    let prefix = prefix.chars().take(42).collect::<String>();
    let timestamp = unix_ms();
    for attempt in 0..1_000_u16 {
        let candidate = if attempt == 0 {
            format!("{prefix}-{timestamp}")
        } else {
            format!("{prefix}-{timestamp}-{attempt}")
        };
        if !AgentSessionStore::for_workspace(workspace, &candidate).exists() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("could not allocate a unique development session identifier")
}
