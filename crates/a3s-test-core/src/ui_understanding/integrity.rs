use std::collections::{HashMap, HashSet};

use super::{
    has_duplicates, UiLayoutEdgeRelation, UiLayoutNode, UiUnderstandingSnapshot,
    UiUnderstandingValidationError,
};

pub(super) fn validate(
    snapshot: &UiUnderstandingSnapshot,
) -> Result<(), UiUnderstandingValidationError> {
    validate_evidence_references(snapshot)?;
    validate_layout_graph(snapshot)?;
    validate_component_references(snapshot)?;
    Ok(())
}

fn validate_evidence_references(
    snapshot: &UiUnderstandingSnapshot,
) -> Result<(), UiUnderstandingValidationError> {
    if has_duplicates(
        snapshot
            .evidence
            .source_kinds
            .iter()
            .map(|source| *source as u8),
    ) || !unique_non_empty(
        snapshot
            .evidence
            .sampled_node_ids
            .iter()
            .map(String::as_str),
    ) {
        return Err(UiUnderstandingValidationError::new(
            "UI understanding evidence references are inconsistent",
        ));
    }
    Ok(())
}

fn validate_layout_graph(
    snapshot: &UiUnderstandingSnapshot,
) -> Result<(), UiUnderstandingValidationError> {
    if snapshot.layout.nodes.len() > snapshot.budget.used.nodes as usize {
        return Err(UiUnderstandingValidationError::new(
            "UI understanding layout exceeds its sampled node count",
        ));
    }

    let mut nodes = HashMap::with_capacity(snapshot.layout.nodes.len());
    for node in &snapshot.layout.nodes {
        if node.node_id.is_empty()
            || node
                .parent_node_id
                .as_deref()
                .is_some_and(|parent| parent.is_empty() || parent == node.node_id)
            || nodes.insert(node.node_id.as_str(), node).is_some()
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding layout node identities are inconsistent",
            ));
        }
    }
    if nodes.values().any(|node| {
        node.parent_node_id
            .as_deref()
            .is_some_and(|parent| !nodes.contains_key(parent))
    }) {
        return Err(UiUnderstandingValidationError::new(
            "UI understanding layout parent identities are inconsistent",
        ));
    }

    let mut edges = HashSet::with_capacity(snapshot.layout.edges.len());
    let mut owners = HashSet::with_capacity(snapshot.layout.edges.len());
    let mut containment = HashMap::with_capacity(snapshot.layout.nodes.len());
    for edge in &snapshot.layout.edges {
        let relation = edge.relation as u8;
        let edge_key = (
            relation,
            edge.from_node_id.as_str(),
            edge.to_node_id.as_str(),
        );
        let owner_key = (relation, edge.to_node_id.as_str());
        let target = nodes.get(edge.to_node_id.as_str());
        if edge.from_node_id.is_empty()
            || edge.to_node_id.is_empty()
            || edge.from_node_id == edge.to_node_id
            || !nodes.contains_key(edge.from_node_id.as_str())
            || target.is_none()
            || !edges.insert(edge_key)
            || !owners.insert(owner_key)
            || (edge.relation == UiLayoutEdgeRelation::Contains
                && target.and_then(|node| node.parent_node_id.as_deref())
                    != Some(edge.from_node_id.as_str()))
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding layout edges are inconsistent",
            ));
        }
        if edge.relation == UiLayoutEdgeRelation::Contains {
            containment.insert(edge.to_node_id.as_str(), edge.from_node_id.as_str());
        }
    }
    if nodes.values().any(|node| {
        node.parent_node_id.as_deref() != containment.get(node.node_id.as_str()).copied()
    }) {
        return Err(UiUnderstandingValidationError::new(
            "UI understanding layout containment is incomplete",
        ));
    }
    if has_parent_cycle(&nodes) {
        return Err(UiUnderstandingValidationError::new(
            "UI understanding layout containment contains a cycle",
        ));
    }
    Ok(())
}

fn has_parent_cycle(nodes: &HashMap<&str, &UiLayoutNode>) -> bool {
    let mut complete = HashSet::with_capacity(nodes.len());
    for start in nodes.keys().copied() {
        if complete.contains(start) {
            continue;
        }
        let mut path = HashSet::new();
        let mut current = Some(start);
        while let Some(node_id) = current {
            if complete.contains(node_id) {
                break;
            }
            if !path.insert(node_id) {
                return true;
            }
            current = nodes
                .get(node_id)
                .and_then(|node| node.parent_node_id.as_deref());
        }
        complete.extend(path);
    }
    false
}

fn validate_component_references(
    snapshot: &UiUnderstandingSnapshot,
) -> Result<(), UiUnderstandingValidationError> {
    let mut cluster_ids = HashSet::with_capacity(snapshot.components.len());
    for cluster in &snapshot.components {
        let members = cluster.member_node_ids.iter().map(String::as_str);
        if cluster.id.is_empty()
            || cluster.representative_node_id.is_empty()
            || !cluster_ids.insert(cluster.id.as_str())
            || cluster.member_node_ids.len() < 2
            || !unique_non_empty(members)
            || !cluster
                .member_node_ids
                .iter()
                .any(|member| member == &cluster.representative_node_id)
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding component references are inconsistent",
            ));
        }
    }
    Ok(())
}

fn unique_non_empty<'a>(mut values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = HashSet::new();
    values.all(|value| !value.is_empty() && seen.insert(value))
}
