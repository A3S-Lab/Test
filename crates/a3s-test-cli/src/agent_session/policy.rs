use a3s_test_core::{
    Action, DriverError, Expectation, StepOutput, TabOperation, Target, VideoOperation,
};
use anyhow::{Context, Result};
use url::Url;

use super::store::AgentSessionState;
use super::web_origin;

pub(super) fn validate_action(
    state: &AgentSessionState,
    action: &Action,
    observation: Option<u64>,
) -> Result<()> {
    if action_uses_ref(action) {
        let latest = state.latest_observation.ok_or_else(|| {
            anyhow::anyhow!("ref targets require a fresh `a3s-test agent observe` result")
        })?;
        if observation != Some(latest) {
            anyhow::bail!(
                "ref target belongs to observation {latest}; pass `--observation {latest}`"
            );
        }
    }

    for url in action_navigation_urls(action) {
        let parsed =
            Url::parse(url).with_context(|| format!("action navigation URL '{url}' is invalid"))?;
        let origin = web_origin(&parsed)?;
        if !state
            .allowed_origins
            .iter()
            .any(|allowed| allowed == &origin)
        {
            anyhow::bail!("navigation origin '{origin}' is outside this session's allowed origins");
        }
    }
    Ok(())
}

pub(super) fn validate_observation_origin(
    state: &AgentSessionState,
    output: &StepOutput,
) -> std::result::Result<(), DriverError> {
    let observed_url = observed_url(&output.data).ok_or_else(|| {
        DriverError::new(
            "test.driver.web.output_invalid",
            "browser snapshot did not report its page URL",
        )
    })?;
    let parsed = Url::parse(observed_url).map_err(|_| {
        DriverError::new(
            "test.driver.web.session_origin_lost",
            "browser snapshot returned an invalid page origin",
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(DriverError::new(
            "test.driver.web.session_origin_lost",
            format!(
                "browser session left its Web page and reported the '{}' scheme",
                parsed.scheme()
            ),
        ));
    }
    let origin = parsed.origin().ascii_serialization();
    if !state
        .allowed_origins
        .iter()
        .any(|allowed| allowed == &origin)
    {
        return Err(DriverError::new(
            "test.driver.web.navigation_origin_denied",
            format!("browser observed navigation to unapproved origin '{origin}'"),
        ));
    }
    Ok(())
}

fn observed_url(value: &serde_json::Value) -> Option<&str> {
    [
        "/data/origin",
        "/data/url",
        "/data/value/origin",
        "/data/value/url",
        "/origin",
        "/url",
    ]
    .into_iter()
    .find_map(|pointer| value.pointer(pointer).and_then(serde_json::Value::as_str))
}

fn action_uses_ref(action: &Action) -> bool {
    match action {
        Action::Click { target }
        | Action::Hover { target }
        | Action::Focus { target }
        | Action::DoubleClick { target }
        | Action::ContextClick { target }
        | Action::Fill { target, .. }
        | Action::Type { target, .. }
        | Action::Check { target }
        | Action::Uncheck { target }
        | Action::Select { target, .. }
        | Action::Upload { target, .. }
        | Action::Download { target, .. } => target_uses_ref(target),
        Action::Drag { source, target } => target_uses_ref(source) || target_uses_ref(target),
        Action::Wheel {
            target: Some(target),
            ..
        } => target_uses_ref(target),
        Action::Assert {
            expectation: Expectation::Visible(target),
        } => target_uses_ref(target),
        _ => false,
    }
}

fn target_uses_ref(target: &Target) -> bool {
    matches!(target, Target::Ref { .. })
}

fn action_navigation_urls(action: &Action) -> Vec<&str> {
    match action {
        Action::Navigate { url } => vec![url],
        Action::Tab {
            operation: TabOperation::New { url: Some(url), .. },
        } => vec![url],
        Action::Video {
            operation: VideoOperation::Start { url: Some(url), .. },
        } => vec![url],
        _ => Vec::new(),
    }
}
