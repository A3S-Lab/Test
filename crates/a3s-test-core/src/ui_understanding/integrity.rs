use std::collections::{HashMap, HashSet};

use super::{
    has_duplicates, UiLayoutEdgeRelation, UiUnderstandingSnapshot, UiUnderstandingValidationError,
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

    let mut edges = HashSet::with_capacity(snapshot.layout.edges.len());
    let mut owners = HashSet::with_capacity(snapshot.layout.edges.len());
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
    }
    Ok(())
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
