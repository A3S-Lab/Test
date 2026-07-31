use std::collections::HashSet;

use a3s_test_core::{Action, Surface, SurfaceObservation};
use url::{Origin, Url};

use crate::{ActionHistory, AgentError, AgentGoal};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActionKind {
    Navigate,
    Snapshot,
    Click,
    Fill,
    Press,
    Wait,
    Assert,
    Screenshot,
}

impl From<&Action> for ActionKind {
    fn from(action: &Action) -> Self {
        match action {
            Action::Navigate { .. } => Self::Navigate,
            Action::Snapshot { .. } => Self::Snapshot,
            Action::Click { .. } => Self::Click,
            Action::Fill { .. } => Self::Fill,
            Action::Press { .. } => Self::Press,
            Action::Wait { .. } => Self::Wait,
            Action::Assert { .. } => Self::Assert,
            Action::Screenshot { .. } => Self::Screenshot,
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
        let kind = ActionKind::from(action);
        if !self.allowed.contains(&kind) {
            return Err(AgentError::new(
                "test.agent.policy.action_denied",
                format!("{kind:?} actions are not allowed by this agent policy"),
            ));
        }

        let Action::Navigate { url } = action else {
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
