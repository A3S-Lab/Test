use std::collections::{BTreeMap, BTreeSet, HashMap};

use a3s_test_core::{
    AdmittedProvenance, ContractCitation, ContractElement, ContractProvenanceStatus,
    ContractVariant, SurfaceContractDraft,
};

use super::merge::{candidate_id, stable_id};
use super::{
    ContractCandidate, ContractCandidateElement, ContractCandidateVariant, ContractConflict,
    ContractConflictResolution, ContractConflictStatus, ContractGenerationReview,
    ContractReviewAction, GeneratedContractDraft, GeneratedContractProvenance,
    ReviewedContractDraft,
};
use crate::ContractGenerationError;

pub(super) fn review(
    mut draft: GeneratedContractDraft,
    review: ContractGenerationReview,
    max_string_bytes: usize,
) -> Result<ReviewedContractDraft, ContractGenerationError> {
    validate_review_identity(&review, max_string_bytes)?;
    let actions = review_actions(&review)?;
    let candidate_index = candidate_index(&draft.candidates);
    if let Some(candidate_id) = actions
        .keys()
        .find(|candidate_id| !candidate_index.contains_key(candidate_id.as_str()))
    {
        return Err(review_error(format!(
            "review references unknown candidate '{candidate_id}'"
        )));
    }

    resolve_conflicts(&mut draft.conflicts, &review, &actions)?;
    let selected_by_key = selected_candidates(&actions, &candidate_index)?;
    ensure_conflict_selections(&draft.conflicts, &selected_by_key)?;
    ensure_selected_candidates_are_decided(&selected_by_key)?;

    let mut variants = BTreeMap::<String, ReviewedVariant>::new();
    let source_by_id = draft
        .provenance
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<HashMap<_, _>>();
    let mut selected_sources = BTreeSet::new();
    for (candidate_id, candidate) in selected_by_key.values() {
        let Some(source) = source_by_id.get(candidate.source_id) else {
            return Err(review_error(format!(
                "selected candidate '{candidate_id}' references unknown source '{}'",
                candidate.source_id
            )));
        };
        selected_sources.insert(source.source_id.clone());
        let candidate_variant = find_variant(&draft.candidates, candidate_id)?;
        let reviewed_variant = variants
            .entry(candidate_variant.id.clone())
            .or_insert_with(|| ReviewedVariant::from_candidate(candidate_variant));
        reviewed_variant.ensure_same(candidate_variant)?;
        let mut element = candidate.element.element.clone();
        element.citations = citations(candidate_id, candidate.element, &source_by_id)?;
        reviewed_variant.elements.push(element);
    }
    if variants.is_empty() {
        return Err(review_error("review did not approve any contract elements"));
    }

    let provenance = draft
        .provenance
        .iter()
        .filter(|source| selected_sources.contains(&source.source_id))
        .map(|source| AdmittedProvenance {
            id: source.source_id.clone(),
            kind: source.kind.provenance_kind(),
            uri: source.uri.clone(),
            digest: source.sha256.clone(),
            status: ContractProvenanceStatus::Reviewed,
            confidence: 100,
        })
        .collect::<Vec<_>>();
    let contract = SurfaceContractDraft::new(
        draft.name.clone(),
        draft.version,
        draft.context.clone(),
        provenance,
        variants
            .into_values()
            .map(ReviewedVariant::into_contract)
            .collect(),
    )
    .map_err(|error| review_error(format!("reviewed contract draft is invalid: {error}")))?;
    Ok(ReviewedContractDraft {
        contract,
        generated: draft,
        review,
    })
}

fn validate_review_identity(
    review: &ContractGenerationReview,
    max_string_bytes: usize,
) -> Result<(), ContractGenerationError> {
    if review.reviewer.trim().is_empty() || review.reviewer.len() > max_string_bytes {
        return Err(review_error(
            "reviewer identity must be non-empty and bounded",
        ));
    }
    if review.decisions.is_empty() {
        return Err(review_error(
            "at least one explicit candidate review decision is required",
        ));
    }
    Ok(())
}

fn review_actions(
    review: &ContractGenerationReview,
) -> Result<HashMap<String, ContractReviewAction>, ContractGenerationError> {
    let mut actions = HashMap::new();
    for decision in &review.decisions {
        validate_candidate_id(&decision.candidate_id)?;
        if actions
            .insert(decision.candidate_id.clone(), decision.action)
            .is_some()
        {
            return Err(review_error(format!(
                "candidate '{}' has duplicate review decisions",
                decision.candidate_id
            )));
        }
    }
    Ok(actions)
}

fn resolve_conflicts(
    conflicts: &mut [ContractConflict],
    review: &ContractGenerationReview,
    actions: &HashMap<String, ContractReviewAction>,
) -> Result<(), ContractGenerationError> {
    let resolutions = review
        .conflict_resolutions
        .iter()
        .map(|resolution| (resolution.conflict_id.as_str(), resolution))
        .collect::<HashMap<_, _>>();
    if resolutions.len() != review.conflict_resolutions.len() {
        return Err(review_error(
            "conflict resolution identifiers must be unique",
        ));
    }
    for conflict in conflicts.iter_mut() {
        let Some(resolution) = resolutions.get(conflict.id.as_str()) else {
            return Err(ContractGenerationError::new(
                "test.agent.contract_generation.conflict_unresolved",
                format!("conflict '{}' requires an explicit resolution", conflict.id),
                false,
            ));
        };
        validate_resolution(conflict, resolution, actions)?;
        conflict.status = ContractConflictStatus::Resolved;
        conflict.resolution = Some((*resolution).clone());
    }
    if let Some(conflict_id) = resolutions
        .keys()
        .find(|conflict_id| !conflicts.iter().any(|value| value.id == **conflict_id))
    {
        return Err(review_error(format!(
            "review resolves unknown conflict '{conflict_id}'"
        )));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct IndexedCandidate<'a> {
    source_id: &'a str,
    variant_id: &'a str,
    element: &'a ContractCandidateElement,
}

type SelectedCandidates<'a> = BTreeMap<(String, String), (String, IndexedCandidate<'a>)>;

fn candidate_index(candidates: &[ContractCandidate]) -> HashMap<String, IndexedCandidate<'_>> {
    let mut index = HashMap::new();
    for candidate in candidates {
        for variant in &candidate.variants {
            for element in &variant.elements {
                index.insert(
                    candidate_id(candidate, variant, element),
                    IndexedCandidate {
                        source_id: &candidate.source_id,
                        variant_id: &variant.id,
                        element,
                    },
                );
            }
        }
    }
    index
}

fn selected_candidates<'a>(
    actions: &HashMap<String, ContractReviewAction>,
    index: &'a HashMap<String, IndexedCandidate<'a>>,
) -> Result<SelectedCandidates<'a>, ContractGenerationError> {
    let mut selected = BTreeMap::new();
    for (candidate_id, action) in actions {
        if *action != ContractReviewAction::Approve {
            continue;
        }
        let indexed = *index
            .get(candidate_id)
            .ok_or_else(|| review_error("approved candidate is unknown"))?;
        let key = (
            indexed.variant_id.to_string(),
            indexed.element.element.id.clone(),
        );
        if selected
            .insert(key.clone(), (candidate_id.clone(), indexed))
            .is_some()
        {
            return Err(review_error(format!(
                "review approves multiple candidates for variant '{}' element '{}'",
                key.0, key.1
            )));
        }
    }
    Ok(selected)
}

fn validate_resolution(
    conflict: &ContractConflict,
    resolution: &ContractConflictResolution,
    actions: &HashMap<String, ContractReviewAction>,
) -> Result<(), ContractGenerationError> {
    if resolution.rationale.trim().is_empty()
        || !conflict
            .candidate_ids
            .contains(&resolution.selected_candidate_id)
        || actions.get(&resolution.selected_candidate_id) != Some(&ContractReviewAction::Approve)
    {
        return Err(review_error(format!(
            "conflict '{}' resolution must select an approved candidate with rationale",
            conflict.id
        )));
    }
    Ok(())
}

fn ensure_conflict_selections(
    conflicts: &[ContractConflict],
    selected: &SelectedCandidates<'_>,
) -> Result<(), ContractGenerationError> {
    for conflict in conflicts {
        let key = (conflict.variant_id.clone(), conflict.element_id.clone());
        let selected_id = selected
            .get(&key)
            .map(|(id, _)| id)
            .ok_or_else(|| review_error("resolved conflict has no approved candidate"))?;
        if conflict
            .resolution
            .as_ref()
            .is_none_or(|resolution| resolution.selected_candidate_id != *selected_id)
        {
            return Err(review_error(
                "approved candidate does not match the explicit conflict resolution",
            ));
        }
    }
    Ok(())
}

fn ensure_selected_candidates_are_decided(
    selected: &SelectedCandidates<'_>,
) -> Result<(), ContractGenerationError> {
    let mut unresolved = selected.values().flat_map(|(candidate_id, candidate)| {
        candidate
            .element
            .unresolved_decision_ids
            .iter()
            .map(move |decision_id| (candidate_id, decision_id))
    });
    if let Some((candidate_id, decision_id)) = unresolved.next() {
        return Err(ContractGenerationError::new(
            "test.agent.contract_generation.decision_unresolved",
            format!(
                "approved candidate '{candidate_id}' depends on unresolved product decision '{decision_id}'"
            ),
            false,
        ));
    }
    Ok(())
}

fn find_variant<'a>(
    candidates: &'a [ContractCandidate],
    selected_candidate_id: &str,
) -> Result<&'a ContractCandidateVariant, ContractGenerationError> {
    for candidate in candidates {
        for variant in &candidate.variants {
            if variant
                .elements
                .iter()
                .any(|element| selected_candidate_id == candidate_id(candidate, variant, element))
            {
                return Ok(variant);
            }
        }
    }
    Err(review_error("approved candidate variant is missing"))
}

fn citations(
    candidate_id: &str,
    candidate: &ContractCandidateElement,
    sources: &HashMap<&str, &GeneratedContractProvenance>,
) -> Result<Vec<ContractCitation>, ContractGenerationError> {
    candidate
        .source_spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            if !sources.contains_key(span.source_id.as_str()) {
                return Err(review_error(format!(
                    "candidate '{candidate_id}' citation source is unknown"
                )));
            }
            Ok(ContractCitation {
                id: citation_id(candidate_id, index),
                provenance_id: span.source_id.clone(),
                quote: span.quote.clone(),
                start: span.start,
                end: span.end,
            })
        })
        .collect()
}

fn citation_id(candidate_id: &str, index: usize) -> String {
    let digest = stable_id("citation", &[candidate_id, &index.to_string()]);
    format!("citation-{}", &digest[9..25])
}

fn validate_candidate_id(value: &str) -> Result<(), ContractGenerationError> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|part| !valid_identifier(part)) {
        return Err(review_error("candidate ID is invalid"));
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[derive(Clone)]
struct ReviewedVariant {
    id: String,
    state: String,
    min_width: Option<u32>,
    max_width: Option<u32>,
    theme: Option<a3s_test_core::PageContextTheme>,
    language: Option<String>,
    elements: Vec<ContractElement>,
}

impl ReviewedVariant {
    fn from_candidate(value: &ContractCandidateVariant) -> Self {
        Self {
            id: value.id.clone(),
            state: value.state.clone(),
            min_width: value.min_width,
            max_width: value.max_width,
            theme: value.theme,
            language: value.language.clone(),
            elements: Vec::new(),
        }
    }

    fn ensure_same(&self, value: &ContractCandidateVariant) -> Result<(), ContractGenerationError> {
        if self.state != value.state
            || self.min_width != value.min_width
            || self.max_width != value.max_width
            || self.theme != value.theme
            || self.language != value.language
        {
            return Err(review_error(format!(
                "approved candidates disagree on variant '{}' metadata",
                value.id
            )));
        }
        Ok(())
    }

    fn into_contract(self) -> ContractVariant {
        ContractVariant {
            id: self.id,
            state: self.state,
            min_width: self.min_width,
            max_width: self.max_width,
            theme: self.theme,
            language: self.language,
            elements: self.elements,
        }
    }
}

fn review_error(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.review_invalid",
        message,
        false,
    )
}
