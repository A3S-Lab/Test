use std::time::Duration;

use a3s_test_core::DriverError;
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, AgentBrowserSession,
    BrowserCommand, BrowserMicrophone, BrowserNetworkPolicy,
};
use anyhow::Result;

use super::runtime::remove_runtime_directory;
use super::store::{
    AgentSessionState, StoredBrowserContainment, StoredBrowserDriver, StoredBrowserMicrophone,
};
use crate::validate_timeout;

#[derive(Clone, Copy)]
pub(super) enum BrowserConnectionPurpose {
    Turn,
    Cleanup,
}

pub(super) async fn connect(
    state: &AgentSessionState,
    purpose: BrowserConnectionPurpose,
) -> Result<AgentBrowserSession> {
    validate_timeout(state.browser.command_timeout_ms, "command timeout")?;
    validate_timeout(state.browser.idle_timeout_ms, "idle timeout")?;
    let command = match state.browser.driver {
        StoredBrowserDriver::A3s => BrowserCommand::A3s {
            executable: state.browser.executable.clone(),
        },
        StoredBrowserDriver::Standalone => BrowserCommand::Standalone {
            executable: state.browser.executable.clone(),
        },
    };
    let driver = AgentBrowserDriver::new(AgentBrowserConfig {
        command,
        namespace: state.namespace.clone(),
        headed: state.browser.headed,
        command_timeout: Duration::from_millis(state.browser.command_timeout_ms),
        idle_timeout: Duration::from_millis(state.browser.idle_timeout_ms),
        microphone: match state.browser.microphone {
            StoredBrowserMicrophone::Disabled => BrowserMicrophone::Disabled,
            StoredBrowserMicrophone::Synthetic => BrowserMicrophone::Synthetic,
        },
        network_policy: stored_browser_network_policy(state, purpose)?,
    });
    driver
        .connect(AgentBrowserConnectionConfig {
            namespace: state.namespace.clone(),
            session: state.driver_session.clone(),
            runtime_dir: state.runtime_dir.clone(),
            artifacts_dir: state.artifacts_dir.clone(),
            active_video_path: state.active_video_path.clone(),
        })
        .await
        .map_err(anyhow::Error::new)
}

pub(super) async fn close_and_remove_runtime(
    browser: &mut AgentBrowserSession,
    state: &AgentSessionState,
) -> Option<DriverError> {
    if let Err(error) = browser.close_surface().await {
        return Some(error);
    }
    remove_runtime_directory(&state.runtime_dir, &state.workspace, &state.session)
        .await
        .err()
        .map(|error| {
            DriverError::new(
                "test.session.runtime_cleanup_failed",
                format!("browser closed but runtime cleanup failed: {error:#}"),
            )
        })
}

pub(super) fn validate_turn_browser_network_policy(
    state: &AgentSessionState,
) -> Result<(), DriverError> {
    stored_browser_network_policy(state, BrowserConnectionPurpose::Turn).map(drop)
}

pub(super) fn containment_for_driver(driver: StoredBrowserDriver) -> StoredBrowserContainment {
    match driver {
        StoredBrowserDriver::A3s => StoredBrowserContainment::ExactOriginV1,
        StoredBrowserDriver::Standalone => StoredBrowserContainment::HostnameV1,
    }
}

pub(super) fn stored_browser_network_policy(
    state: &AgentSessionState,
    purpose: BrowserConnectionPurpose,
) -> Result<BrowserNetworkPolicy, DriverError> {
    if matches!(purpose, BrowserConnectionPurpose::Cleanup) {
        return Ok(BrowserNetworkPolicy::default());
    }
    match (
        state.browser_containment,
        &state.browser_allowed_origins,
        &state.browser_allowed_domains,
    ) {
        (Some(containment), Some(origins), Some(domains))
            if containment == containment_for_driver(state.browser.driver) =>
        {
            let policy = BrowserNetworkPolicy::restricted(origins.clone(), domains.clone())?;
            if policy.allowed_origins() != origins
                || policy.allowed_domains() != domains
                || policy.allowed_origins() != state.allowed_origins
            {
                return Err(DriverError::new(
                    "test.session.browser_network_policy_mismatch",
                    "stored browser policy is not canonical or no longer matches the session origins; abort this session and start a new one",
                ));
            }
            Ok(policy)
        }
        (Some(_), Some(_), Some(_)) => Err(DriverError::new(
            "test.session.browser_containment_mismatch",
            "stored browser containment mode does not match the selected driver; abort this session and start a new one",
        )),
        _ => Err(DriverError::new(
            "test.session.browser_network_policy_missing",
            "agent session predates typed browser containment; abort it and start a new session before executing another turn",
        )),
    }
}
