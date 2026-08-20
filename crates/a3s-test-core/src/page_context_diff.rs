use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{PageContextComponent, PageContextNode};

pub const PAGE_CONTEXT_DIFF_PROTOCOL: &str = "a3s.test.page-context-diff/1";
const MAX_INVALIDATED_IDS: usize = 10_000;
const MAX_ID_BYTES: usize = 256;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextDelta {
    pub protocol: String,
    #[serde(rename = "fromRevision")]
    pub from_revision: u64,
    #[serde(rename = "toRevision")]
    pub to_revision: u64,
    pub status: PageContextDeltaStatus,
    pub invalidated: PageContextInvalidation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageContextDeltaStatus {
    Complete,
    ResetRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextInvalidation {
    pub all: bool,
    pub page: bool,
    pub facts: bool,
    pub ui: bool,
    #[serde(rename = "nodeIds")]
    pub node_ids: Vec<String>,
    #[serde(rename = "componentIds")]
    pub component_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct PageContextDeltaValidationError {
    message: String,
}

impl PageContextDelta {
    pub fn validate(
        &self,
        snapshot_revision: Option<u64>,
        changed_nodes: &[PageContextNode],
        changed_components: &[PageContextComponent],
        removed_node_ids: &[String],
    ) -> Result<(), PageContextDeltaValidationError> {
        if self.protocol != PAGE_CONTEXT_DIFF_PROTOCOL {
            return Err(invalid("page context delta protocol is unsupported"));
        }
        if self.from_revision == 0
            || self.to_revision == 0
            || self.from_revision > self.to_revision
            || snapshot_revision != Some(self.to_revision)
        {
            return Err(invalid(
                "page context delta revisions must be positive, ordered, and match the snapshot",
            ));
        }
        validate_sorted_ids("node", &self.invalidated.node_ids)?;
        let invalidated_components =
            validate_sorted_ids("component", &self.invalidated.component_ids)?;
        let changed = validate_changed_nodes(changed_nodes)?;
        let changed_components = validate_changed_components(changed_components)?;
        let removed = validate_sorted_ids("removed node", removed_node_ids)?;
        if changed.iter().any(|id| removed.contains(id)) {
            return Err(invalid(
                "page context delta names one node as both changed and removed",
            ));
        }

        match self.status {
            PageContextDeltaStatus::ResetRequired => {
                if self.from_revision == self.to_revision
                    || !self.invalidated.all
                    || !self.invalidated.page
                    || !self.invalidated.facts
                    || !self.invalidated.ui
                    || !self.invalidated.node_ids.is_empty()
                    || !self.invalidated.component_ids.is_empty()
                {
                    return Err(invalid(
                        "reset-required deltas must advance the revision and invalidate all evidence without partial identifiers",
                    ));
                }
            }
            PageContextDeltaStatus::Complete => {
                if self.invalidated.all {
                    return Err(invalid("complete deltas cannot declare a whole-page reset"));
                }
                let invalidated = self
                    .invalidated
                    .node_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                if changed_nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .chain(removed_node_ids.iter().map(String::as_str))
                    .any(|id| !invalidated.contains(id))
                {
                    return Err(invalid(
                        "complete delta omitted changed or removed node evidence",
                    ));
                }
                if changed_components
                    .iter()
                    .any(|id| !invalidated_components.contains(id))
                {
                    return Err(invalid("complete delta omitted changed component evidence"));
                }
                if self.from_revision == self.to_revision
                    && (self.invalidated.page
                        || self.invalidated.facts
                        || self.invalidated.ui
                        || !self.invalidated.node_ids.is_empty()
                        || !self.invalidated.component_ids.is_empty()
                        || !changed_nodes.is_empty()
                        || !changed_components.is_empty()
                        || !removed_node_ids.is_empty())
                {
                    return Err(invalid("same-revision deltas cannot invalidate evidence"));
                }
                if self.from_revision < self.to_revision && !self.invalidated.ui {
                    return Err(invalid(
                        "a newer page revision must invalidate revision-bound UI evidence",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_sorted_ids<'a>(
    kind: &str,
    values: &'a [String],
) -> Result<BTreeSet<&'a str>, PageContextDeltaValidationError> {
    if values.len() > MAX_INVALIDATED_IDS {
        return Err(invalid(format!(
            "page context delta contains too many invalidated {kind} identifiers"
        )));
    }
    let mut seen = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for value in values {
        if value.is_empty()
            || value.len() > MAX_ID_BYTES
            || value.chars().any(char::is_control)
            || previous.is_some_and(|previous| previous >= value.as_str())
            || !seen.insert(value.as_str())
        {
            return Err(invalid(format!(
                "page context delta contains an invalid, duplicate, or unsorted {kind} identifier"
            )));
        }
        previous = Some(value);
    }
    Ok(seen)
}

fn validate_changed_nodes(
    nodes: &[PageContextNode],
) -> Result<BTreeSet<&str>, PageContextDeltaValidationError> {
    if nodes.len() > MAX_INVALIDATED_IDS {
        return Err(invalid(
            "page context delta contains too many changed nodes",
        ));
    }
    let mut seen = BTreeSet::new();
    for node in nodes {
        if node.id.is_empty()
            || node.id.len() > MAX_ID_BYTES
            || node.id.chars().any(char::is_control)
            || !seen.insert(node.id.as_str())
        {
            return Err(invalid(
                "page context delta contains an invalid or duplicate changed node identifier",
            ));
        }
    }
    Ok(seen)
}

fn validate_changed_components(
    components: &[PageContextComponent],
) -> Result<BTreeSet<&str>, PageContextDeltaValidationError> {
    if components.len() > MAX_INVALIDATED_IDS {
        return Err(invalid(
            "page context delta contains too many changed components",
        ));
    }
    let mut seen = BTreeSet::new();
    for component in components {
        if component.id.is_empty()
            || component.id.len() > MAX_ID_BYTES
            || component.id.chars().any(char::is_control)
            || !seen.insert(component.id.as_str())
        {
            return Err(invalid(
                "page context delta contains an invalid or duplicate changed component identifier",
            ));
        }
    }
    Ok(seen)
}

fn invalid(message: impl Into<String>) -> PageContextDeltaValidationError {
    PageContextDeltaValidationError {
        message: message.into(),
    }
}
