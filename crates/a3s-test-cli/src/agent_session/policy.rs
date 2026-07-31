use a3s_test_core::{Action, Expectation, TabOperation, Target, VideoOperation};
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

fn action_uses_ref(action: &Action) -> bool {
    match action {
        Action::Click { target }
        | Action::Fill { target, .. }
        | Action::Upload { target, .. }
        | Action::Download { target, .. } => target_uses_ref(target),
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
