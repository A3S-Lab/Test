use std::collections::HashSet;

use a3s_test_core::{Action, Surface, SurfaceObservation, TabOperation, VideoOperation};
use url::{Origin, Url};

use crate::{ActionHistory, AgentError, AgentGoal};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActionKind {
    Navigate,
    Snapshot,
    Click,
    Hover,
    Focus,
    DoubleClick,
    ContextClick,
    Fill,
    Type,
    Check,
    Uncheck,
    Select,
    Drag,
    Press,
    TerminalPaste,
    TerminalResize,
    TerminalRecording,
    Wheel,
    Viewport,
    Wait,
    Assert,
    Screenshot,
    Tab,
    Frame,
    Dialog,
    Upload,
    Download,
    NetworkRoute,
    NetworkUnroute,
    Har,
    Trace,
    Video,
    Accessibility,
    Console,
    PageErrors,
    VerifyContract,
}

impl From<&Action> for ActionKind {
    fn from(action: &Action) -> Self {
        match action {
            Action::Navigate { .. } => Self::Navigate,
            Action::Snapshot { .. } => Self::Snapshot,
            Action::Click { .. } => Self::Click,
            Action::Hover { .. } => Self::Hover,
            Action::Focus { .. } => Self::Focus,
            Action::DoubleClick { .. } => Self::DoubleClick,
            Action::ContextClick { .. } => Self::ContextClick,
            Action::Fill { .. } => Self::Fill,
            Action::Type { .. } => Self::Type,
            Action::Check { .. } => Self::Check,
            Action::Uncheck { .. } => Self::Uncheck,
            Action::Select { .. } => Self::Select,
            Action::Drag { .. } => Self::Drag,
            Action::Press { .. } => Self::Press,
            Action::TerminalPaste { .. } => Self::TerminalPaste,
            Action::TerminalResize { .. } => Self::TerminalResize,
            Action::TerminalRecording { .. } => Self::TerminalRecording,
            Action::Wheel { .. } => Self::Wheel,
            Action::Viewport { .. } => Self::Viewport,
            Action::Wait { .. } => Self::Wait,
            Action::Assert { .. } => Self::Assert,
            Action::Screenshot { .. } => Self::Screenshot,
            Action::Tab { .. } => Self::Tab,
            Action::Frame { .. } => Self::Frame,
            Action::Dialog { .. } => Self::Dialog,
            Action::Upload { .. } => Self::Upload,
            Action::Download { .. } => Self::Download,
            Action::NetworkRoute { .. } => Self::NetworkRoute,
            Action::NetworkUnroute { .. } => Self::NetworkUnroute,
            Action::Har { .. } => Self::Har,
            Action::Trace { .. } => Self::Trace,
            Action::Video { .. } => Self::Video,
            Action::Accessibility { .. } => Self::Accessibility,
            Action::Console { .. } => Self::Console,
            Action::PageErrors { .. } => Self::PageErrors,
            Action::VerifyContract { .. } => Self::VerifyContract,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NavigationScope {
    Denied,
    Origins(Vec<Url>),
    Any,
}

pub struct PolicyContext<'a> {
    pub goal: &'a AgentGoal,
    pub surface: Surface,
    pub observation: &'a SurfaceObservation,
    pub history: &'a [ActionHistory],
}

pub trait ActionPolicy: Send + Sync {
    fn validate(&self, context: &PolicyContext<'_>, action: &Action) -> Result<(), AgentError>;
}

pub struct CapabilityPolicy {
    allowed: HashSet<ActionKind>,
    navigation: NavigationScope,
}

impl CapabilityPolicy {
    #[must_use]
    pub fn new(allowed: impl IntoIterator<Item = ActionKind>, navigation: NavigationScope) -> Self {
        Self {
            allowed: allowed.into_iter().collect(),
            navigation,
        }
    }
}

impl ActionPolicy for CapabilityPolicy {
    fn validate(&self, _context: &PolicyContext<'_>, action: &Action) -> Result<(), AgentError> {
        if matches!(action, Action::VerifyContract { .. }) {
            return Err(AgentError::new(
                "test.agent.policy.runner_action_denied",
                "verify_contract belongs to deterministic ACL runs and cannot be proposed by an interactive agent",
            ));
        }
        let kind = ActionKind::from(action);
        if !self.allowed.contains(&kind) {
            return Err(AgentError::new(
                "test.agent.policy.action_denied",
                format!("{kind:?} actions are not allowed by this agent policy"),
            ));
        }

        let Some(url) = action_navigation_url(action) else {
            return Ok(());
        };
        let parsed = Url::parse(url).map_err(|error| {
            AgentError::new(
                "test.agent.policy.navigation_url_invalid",
                format!("proposed navigation URL is invalid: {error}"),
            )
        })?;

        match &self.navigation {
            NavigationScope::Denied => Err(AgentError::new(
                "test.agent.policy.navigation_denied",
                "navigation is disabled by this agent policy",
            )),
            NavigationScope::Any => Ok(()),
            NavigationScope::Origins(allowed) => {
                let proposed = parsed.origin();
                if matches!(&proposed, Origin::Opaque(_)) {
                    return Err(AgentError::new(
                        "test.agent.policy.navigation_origin_denied",
                        "opaque navigation origins are not allowed",
                    ));
                }
                if allowed
                    .iter()
                    .map(Url::origin)
                    .any(|origin| origin == proposed)
                {
                    Ok(())
                } else {
                    Err(AgentError::new(
                        "test.agent.policy.navigation_origin_denied",
                        "proposed navigation origin is outside the allowed origin set",
                    ))
                }
            }
        }
    }
}

fn action_navigation_url(action: &Action) -> Option<&str> {
    match action {
        Action::Navigate { url }
        | Action::Tab {
            operation: TabOperation::New { url: Some(url), .. },
        }
        | Action::Video {
            operation: VideoOperation::Start { url: Some(url), .. },
        } => Some(url),
        _ => None,
    }
}
