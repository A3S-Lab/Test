use std::collections::BTreeMap;

use crate::{UiContextScope, UiObservedToken, UiUnderstandingSnapshot};

pub(super) fn project_ui_evidence_refs(
    snapshot: &mut UiUnderstandingSnapshot,
    actionable_refs: &BTreeMap<String, String>,
) -> bool {
    let mut projector = UiRefProjector::new(actionable_refs);

    // Layout order is the stable visual traversal supplied by Test Kit. Reserve
    // every layout identity before visiting cross-cutting evidence so @uN
    // allocation does not depend on which optional profiles are present.
    for node in &snapshot.layout.nodes {
        projector.reserve(&node.node_id);
    }

    if let UiContextScope::Node { node_id } = &mut snapshot.scope {
        projector.project(node_id);
    }
    project_ids(&mut snapshot.evidence.sampled_node_ids, &mut projector);

    project_observed_tokens(&mut snapshot.style.colors, &mut projector);
    project_ids_in_typography(&mut snapshot.style.typography, &mut projector);
    project_observed_tokens(&mut snapshot.style.spacing, &mut projector);
    project_observed_tokens(&mut snapshot.style.radii, &mut projector);
    project_observed_tokens(&mut snapshot.style.shadows, &mut projector);
    project_observed_tokens(&mut snapshot.style.z_indices, &mut projector);

    for node in &mut snapshot.layout.nodes {
        projector.project(&mut node.node_id);
        if let Some(parent_node_id) = node.parent_node_id.as_mut() {
            projector.project(parent_node_id);
        }
    }
    for edge in &mut snapshot.layout.edges {
        projector.project(&mut edge.from_node_id);
        projector.project(&mut edge.to_node_id);
    }

    for component in &mut snapshot.components {
        projector.project(&mut component.representative_node_id);
        project_ids(&mut component.member_node_ids, &mut projector);
    }
    for state_diff in &mut snapshot.state_diffs {
        projector.project(&mut state_diff.node_id);
    }

    for transition in &mut snapshot.motion.transitions {
        projector.project(&mut transition.node_id);
    }
    for animation in &mut snapshot.motion.animations {
        projector.project(&mut animation.node_id);
    }
    project_ids(&mut snapshot.motion.sticky_node_ids, &mut projector);
    project_ids(
        &mut snapshot.motion.scroll_container_node_ids,
        &mut projector,
    );
    project_ids(&mut snapshot.motion.canvas_node_ids, &mut projector);
    project_ids(&mut snapshot.motion.media_node_ids, &mut projector);

    refresh_encoded_bytes(snapshot)
}

fn project_observed_tokens(tokens: &mut [UiObservedToken], projector: &mut UiRefProjector) {
    for token in tokens {
        project_ids(&mut token.node_ids, projector);
    }
}

fn project_ids_in_typography(
    tokens: &mut [crate::UiTypographyToken],
    projector: &mut UiRefProjector,
) {
    for token in tokens {
        project_ids(&mut token.node_ids, projector);
    }
}

fn project_ids(ids: &mut [String], projector: &mut UiRefProjector) {
    for id in ids {
        projector.project(id);
    }
}

fn refresh_encoded_bytes(snapshot: &mut UiUnderstandingSnapshot) -> bool {
    for _ in 0..8 {
        let Some(encoded_bytes) = encoded_size(snapshot) else {
            return false;
        };
        if snapshot.budget.used.encoded_bytes == encoded_bytes {
            return encoded_bytes <= snapshot.budget.limits.encoded_bytes;
        }
        snapshot.budget.used.encoded_bytes = encoded_bytes;
    }

    encoded_size(snapshot).is_some_and(|encoded_bytes| {
        snapshot.budget.used.encoded_bytes == encoded_bytes
            && encoded_bytes <= snapshot.budget.limits.encoded_bytes
    })
}

fn encoded_size(snapshot: &UiUnderstandingSnapshot) -> Option<u64> {
    serde_json::to_vec(snapshot)
        .ok()
        .and_then(|encoded| u64::try_from(encoded.len()).ok())
}

struct UiRefProjector {
    refs: BTreeMap<String, String>,
    next_ui_ref: usize,
}

impl UiRefProjector {
    fn new(actionable_refs: &BTreeMap<String, String>) -> Self {
        Self {
            refs: actionable_refs.clone(),
            next_ui_ref: 1,
        }
    }

    fn reserve(&mut self, raw_id: &str) {
        let _ = self.reference(raw_id);
    }

    fn project(&mut self, raw_id: &mut String) {
        *raw_id = self.reference(raw_id);
    }

    fn reference(&mut self, raw_id: &str) -> String {
        if let Some(reference) = self.refs.get(raw_id) {
            return reference.clone();
        }
        let reference = format!("@u{}", self.next_ui_ref);
        self.next_ui_ref += 1;
        self.refs.insert(raw_id.to_string(), reference.clone());
        reference
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_ids_that_look_like_refs_still_receive_unique_projection() {
        let actionable = BTreeMap::from([("private-action".to_string(), "@c1".to_string())]);
        let mut projector = UiRefProjector::new(&actionable);

        assert_eq!(projector.reference("plain"), "@u1");
        assert_eq!(projector.reference("@u1"), "@u2");
        assert_eq!(projector.reference("@c1"), "@u3");
        assert_eq!(projector.reference("private-action"), "@c1");
        assert_eq!(projector.reference("plain"), "@u1");
    }
}
