use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Action, Expectation, PageContextLocator, PageContextObservation, SurfaceObservation, Target,
    WaitCondition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod ui_projection;

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

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct PageContextRefError {
    message: String,
}

impl PageContextRefError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Replaces driver-private page-context node identity with observation-scoped
/// references and records the preferred stable target for each reference.
/// Nested UI evidence reuses an unambiguous actionable `@cN` reference and
/// receives a non-actionable `@uN` reference for every other node identity.
#[must_use]
pub fn bind_page_context_refs(observation: &mut SurfaceObservation) -> PageContextBindings {
    let Some(context) = observation.page_context.as_mut() else {
        return PageContextBindings::default();
    };
    bind_page_context_observation_refs(context)
}

/// Projects one typed Page Context payload for a public observation or step
/// output and returns the actionable bindings retained for that observation.
#[must_use]
pub fn bind_page_context_observation_refs(
    context: &mut PageContextObservation,
) -> PageContextBindings {
    let mut bindings = PageContextBindings {
        revision: context.revision,
        ..PageContextBindings::default()
    };
    let Some(snapshot) = context.snapshot.as_mut() else {
        return bindings;
    };
    let mut actionable_refs = BTreeMap::new();
    let mut seen_node_ids = BTreeSet::new();
    let mut ambiguous_node_ids = BTreeSet::new();
    for (index, node) in snapshot.nodes.iter_mut().enumerate() {
        let raw_id = node.id.clone();
        let reference = format!("@c{}", index + 1);
        node.r#ref = None;
        if !raw_id.is_empty() && !seen_node_ids.insert(raw_id.clone()) {
            actionable_refs.remove(&raw_id);
            ambiguous_node_ids.insert(raw_id.clone());
        }
        if let Some(target) = preferred_page_context_target(&node.locators) {
            bindings.targets.insert(reference.clone(), target);
            if !raw_id.is_empty() && !ambiguous_node_ids.contains(&raw_id) {
                actionable_refs.insert(raw_id, reference.clone());
            }
            node.r#ref = Some(reference);
        }
        node.id.clear();
        node.parent_id = None;
        if let Some(geometry) = node.geometry.as_mut() {
            geometry.scroll_container_node_id = None;
        }
    }
    let page_revision = snapshot.revision;
    let page_viewport = snapshot.page.as_ref().map(|page| page.viewport.clone());
    // UI understanding is optional evidence. Omit it instead of exposing a
    // private identity when projection cannot preserve its admitted bounds.
    let keep_ui = snapshot.ui.as_mut().is_none_or(|ui| {
        ui_projection::project_ui_evidence_refs(ui, &actionable_refs)
            && ui.validate(page_revision, page_viewport.as_ref()).is_ok()
    });
    if !keep_ui {
        snapshot.ui = None;
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
) -> Result<Action, PageContextRefError> {
    validate_action_page_context_refs(&action)?;
    visit_action_targets(&mut action, |target| {
        let Target::Ref { value } = target else {
            return Ok(());
        };
        if !is_actionable_page_context_ref(value) {
            return Ok(());
        }
        *target = bindings.targets.get(value).cloned().ok_or_else(|| {
            PageContextRefError::new(format!(
                "page context ref '{value}' is not available in the latest observation"
            ))
        })?;
        Ok(())
    })?;
    Ok(action)
}

/// Rejects observation-only UI evidence refs before an action reaches any
/// surface driver. Actionable Page Context refs remain revision-bound and are
/// resolved separately by [`resolve_page_context_refs`].
pub fn validate_action_page_context_refs(action: &Action) -> Result<(), PageContextRefError> {
    if let Some(value) = action_targets(action).find_map(|target| match target {
        Target::Ref { value } if is_ui_evidence_ref(value) => Some(value),
        _ => None,
    }) {
        return Err(PageContextRefError::new(format!(
            "UI evidence ref '{value}' is observation-only and not actionable"
        )));
    }
    Ok(())
}

#[must_use]
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
        } => targets.push(target),
        Action::Assert { expectation } => {
            if let Some(target) = expectation_target(expectation) {
                targets.push(target);
            }
        }
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
    mut visitor: impl FnMut(&mut Target) -> Result<(), PageContextRefError>,
) -> Result<(), PageContextRefError> {
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
        } => visitor(target)?,
        Action::Assert { expectation } => {
            if let Some(target) = expectation_target_mut(expectation) {
                visitor(target)?;
            }
        }
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

fn expectation_target(expectation: &Expectation) -> Option<&Target> {
    match expectation {
        Expectation::Visible(target)
        | Expectation::RenderedText { target, .. }
        | Expectation::VisibleCount { target, .. }
        | Expectation::State { target, .. }
        | Expectation::Value { target, .. }
        | Expectation::SelectedValues { target, .. } => Some(target),
        Expectation::TextVisible(_) | Expectation::Url(_) => None,
    }
}

fn expectation_target_mut(expectation: &mut Expectation) -> Option<&mut Target> {
    match expectation {
        Expectation::Visible(target)
        | Expectation::RenderedText { target, .. }
        | Expectation::VisibleCount { target, .. }
        | Expectation::State { target, .. }
        | Expectation::Value { target, .. }
        | Expectation::SelectedValues { target, .. } => Some(target),
        Expectation::TextVisible(_) | Expectation::Url(_) => None,
    }
}

fn target_uses_observation(target: &Target) -> bool {
    matches!(target, Target::Ref { .. } | Target::VisualPoint { .. })
}

fn target_is_page_context_ref(target: &Target) -> bool {
    matches!(target, Target::Ref { value } if is_page_context_ref(value))
}

fn is_page_context_ref(value: &str) -> bool {
    is_actionable_page_context_ref(value) || is_ui_evidence_ref(value)
}

fn is_actionable_page_context_ref(value: &str) -> bool {
    numbered_ref(value, "@c")
}

pub(crate) fn is_ui_evidence_ref(value: &str) -> bool {
    numbered_ref(value, "@u")
}

fn numbered_ref(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
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
