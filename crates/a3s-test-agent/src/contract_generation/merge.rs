use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    ContractCandidate, ContractCandidateElement, ContractCandidateVariant, ContractConflict,
    ContractConflictStatus, ProductDecision,
};
use crate::ContractGenerationError;

pub(super) fn merge_decisions(
    candidates: &[ContractCandidate],
) -> Result<Vec<ProductDecision>, ContractGenerationError> {
    let mut merged = BTreeMap::<String, ProductDecision>::new();
    for candidate in candidates {
        for decision in &candidate.unresolved_decisions {
            if let Some(existing) = merged.get(&decision.id) {
                if existing != decision {
                    return Err(response_error(format!(
                        "product decision '{}' has conflicting definitions",
                        decision.id
                    )));
                }
            } else {
                merged.insert(decision.id.clone(), decision.clone());
            }
        }
    }
    Ok(merged.into_values().collect())
}

pub(super) fn detect_conflicts(candidates: &[ContractCandidate]) -> Vec<ContractConflict> {
    let mut grouped = BTreeMap::<(String, String, String), Vec<(String, Value)>>::new();
    for candidate in candidates {
        for variant in &candidate.variants {
            for element in &variant.elements {
                let candidate_id = candidate_id(candidate, variant, element);
                for (field, value) in element_values(element) {
                    grouped
                        .entry((variant.id.clone(), element.element.id.clone(), field))
                        .or_default()
                        .push((candidate_id.clone(), value));
                }
            }
        }
    }
    grouped
        .into_iter()
        .filter_map(|((variant_id, element_id, field), values)| {
            let unique = values
                .iter()
                .map(|(_, value)| canonical_value(value))
                .collect::<BTreeSet<_>>();
            (unique.len() > 1).then(|| ContractConflict {
                id: stable_conflict_id(&variant_id, &element_id, &field),
                variant_id,
                element_id,
                field,
                candidate_ids: values.iter().map(|(id, _)| id.clone()).collect(),
                values: values.into_iter().map(|(_, value)| value).collect(),
                status: ContractConflictStatus::Unresolved,
                resolution: None,
            })
        })
        .collect()
}

pub(super) fn candidate_id(
    candidate: &ContractCandidate,
    variant: &ContractCandidateVariant,
    element: &ContractCandidateElement,
) -> String {
    format!(
        "{}:{}:{}",
        candidate.source_id, variant.id, element.element.id
    )
}

pub(super) fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    format!("{prefix}:{:x}", digest.finalize())
}

fn element_values(element: &ContractCandidateElement) -> Vec<(String, Value)> {
    let value = &element.element;
    let mut values = vec![
        ("required".to_string(), json!(value.required)),
        ("severity".to_string(), json!(value.severity)),
    ];
    macro_rules! optional {
        ($field:ident) => {
            if let Some(field_value) = &value.$field {
                values.push((stringify!($field).to_string(), json!(field_value)));
            }
        };
    }
    optional!(test_id);
    optional!(component_id);
    optional!(role);
    optional!(name);
    optional!(description);
    optional!(visible);
    optional!(enabled);
    optional!(checked);
    optional!(selected);
    optional!(expanded);
    optional!(readonly);
    optional!(form_required);
    optional!(invalid);
    optional!(parent);
    values
}

fn canonical_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

fn stable_conflict_id(variant: &str, element: &str, field: &str) -> String {
    stable_id("conflict", &[variant, element, field])
}

fn response_error(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.response_invalid",
        message,
        false,
    )
}
