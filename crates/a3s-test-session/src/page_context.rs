use std::collections::BTreeMap;

use a3s_test_core::{
    Action, Expectation, PageContextLocator, SurfaceObservation, Target, WaitCondition,
};
use serde::{Deserialize, Serialize};

use crate::SessionError;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PageContextBindings {
    pub revision: Option<u64>,
    pub targets: BTreeMap<String, Target>,
}

impl PageContextBindings {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

#[must_use]
pub fn bind_page_context_refs(observation: &mut SurfaceObservation) -> PageContextBindings {
    let mut bindings = PageContextBindings::default();
    let Some(context) = observation.page_context.as_mut() else {
        return bindings;
    };
    bindings.revision = context.revision;
    let Some(snapshot) = context.snapshot.as_mut() else {
        return bindings;
    };
    for (index, node) in snapshot.nodes.iter_mut().enumerate() {
        let reference = format!("@c{}", index + 1);
        if let Some(target) = preferred_page_context_target(&node.locators) {
            bindings.targets.insert(reference.clone(), target);
            node.r#ref = Some(reference);
        }
        node.id.clear();
        node.parent_id = None;
        if let Some(geometry) = node.geometry.as_mut() {
            geometry.scroll_container_node_id = None;
        }
    }
    snapshot.removed_node_ids.clear();
    bindings
}

#[must_use]
pub fn action_uses_observation_target(action: &Action) -> bool {
    action_targets(action).any(target_uses_observation)
}

#[must_use]
pub fn action_uses_page_context_ref(action: &Action) -> bool {
    action_targets(action).any(target_is_page_context_ref)
}

pub fn resolve_page_context_refs(
    mut action: Action,
    bindings: &PageContextBindings,
) -> Result<Action, SessionError> {
    visit_action_targets(&mut action, |target| {
        let Target::Ref { value } = target else {
            return Ok(());
        };
        if !is_page_context_ref(value) {
            return Ok(());
        }
        *target = bindings.targets.get(value).cloned().ok_or_else(|| {
            SessionError::new(
                "test.session.context_ref_invalid",
                format!("page context ref '{value}' is not available in the latest observation"),
            )
        })?;
        Ok(())
    })?;
    Ok(action)
}

pub fn preferred_page_context_target(locators: &[PageContextLocator]) -> Option<Target> {
    let preferred = ["test_id", "role", "label", "placeholder", "text", "css"];
    preferred.into_iter().find_map(|kind| {
        locators.iter().find_map(|locator| match (kind, locator) {
            ("test_id", PageContextLocator::TestId { value }) => Some(Target::TestId {
                value: value.clone(),
            }),
            ("role", PageContextLocator::Role { role, name }) => Some(Target::Role {
                role: role.clone(),
                name: name.clone(),
            }),
            ("label", PageContextLocator::Label { value }) => Some(Target::Label {
                value: value.clone(),
            }),
            ("placeholder", PageContextLocator::Placeholder { value }) => {
                Some(Target::Placeholder {
                    value: value.clone(),
                })
            }
            ("text", PageContextLocator::Text { value, exact }) => Some(Target::Text {
                value: value.clone(),
                exact: *exact,
            }),
            ("css", PageContextLocator::Css { value }) => Some(Target::Css {
                selector: value.clone(),
            }),
            _ => None,
        })
    })
}

fn action_targets(action: &Action) -> impl Iterator<Item = &Target> {
    let mut targets = Vec::with_capacity(2);
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
        | Action::Download { target, .. }
        | Action::Wait {
            condition: WaitCondition::Visible(target),
        }
        | Action::Assert {
            expectation: Expectation::Visible(target),
        } => targets.push(target),
        Action::Drag { source, target } => {
            targets.push(source);
            targets.push(target);
        }
        Action::Wheel {
            target: Some(target),
            ..
        } => targets.push(target),
        _ => {}
    }
    targets.into_iter()
}

fn visit_action_targets(
    action: &mut Action,
    mut visitor: impl FnMut(&mut Target) -> Result<(), SessionError>,
) -> Result<(), SessionError> {
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
        | Action::Download { target, .. }
        | Action::Wait {
            condition: WaitCondition::Visible(target),
        }
        | Action::Assert {
            expectation: Expectation::Visible(target),
        } => visitor(target)?,
        Action::Drag { source, target } => {
            visitor(source)?;
            visitor(target)?;
        }
        Action::Wheel {
            target: Some(target),
            ..
        } => visitor(target)?,
        _ => {}
    }
    Ok(())
}

fn target_uses_observation(target: &Target) -> bool {
    matches!(target, Target::Ref { .. } | Target::VisualPoint { .. })
}

fn target_is_page_context_ref(target: &Target) -> bool {
    matches!(target, Target::Ref { value } if is_page_context_ref(value))
}

fn is_page_context_ref(value: &str) -> bool {
    value.strip_prefix("@c").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_waits_and_assertions_are_observation_bound() {
        for action in [
            Action::Wait {
                condition: WaitCondition::Visible(Target::Ref {
                    value: "@c1".to_string(),
                }),
            },
            Action::Assert {
                expectation: Expectation::Visible(Target::Ref {
                    value: "@c1".to_string(),
                }),
            },
        ] {
            assert!(action_uses_observation_target(&action));
            assert!(action_uses_page_context_ref(&action));
        }
    }
}
