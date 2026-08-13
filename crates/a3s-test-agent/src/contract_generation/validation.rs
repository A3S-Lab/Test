use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Component, Path};
use std::time::Duration;

use a3s_test_core::{ContractContext, ContractElement};
use sha2::{Digest, Sha256};

use super::merge::candidate_id;
use super::{
    ContractCandidateElement, ContractCandidateVariant, ContractGenerationOptions,
    ContractGenerationProviderIdentity, ContractGenerationProviderResponse, ContractSource,
    ContractSourceKind, ContractSourceSpan, DesignCoordinateSpace, GeneratedContractProvenance,
    ProductDecision,
};
use crate::ContractGenerationError;

const MIN_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_SOURCES: usize = 32;
const MAX_SOURCE_BYTES: usize = 32 * 1_024 * 1_024;
const MAX_CANDIDATES: usize = 256;
const MAX_ELEMENTS: usize = 5_000;
const MAX_STRING_BYTES: usize = 64 * 1_024;
const MAX_ID_BYTES: usize = 64;
const MAX_PATH_BYTES: usize = 16 * 1_024;
const MAX_REQUEST_ID_BYTES: usize = 4 * 1_024;
const MAX_DIMENSION: u32 = 32_768;
const MAX_SPANS_PER_ITEM: usize = 64;

pub(super) type VerifiedSourceBytes = HashMap<String, Vec<u8>>;

pub(super) fn validate_options(
    options: &ContractGenerationOptions,
) -> Result<(), ContractGenerationError> {
    if options.timeout < MIN_TIMEOUT
        || options.timeout > MAX_TIMEOUT
        || options.max_sources == 0
        || options.max_sources > MAX_SOURCES
        || options.max_source_bytes == 0
        || options.max_source_bytes > MAX_SOURCE_BYTES
        || options.max_candidates == 0
        || options.max_candidates > MAX_CANDIDATES
        || options.max_elements == 0
        || options.max_elements > MAX_ELEMENTS
        || options.max_string_bytes == 0
        || options.max_string_bytes > MAX_STRING_BYTES
    {
        return Err(config_error("contract generation limits are invalid"));
    }
    Ok(())
}

pub(super) fn validate_identity(
    identity: &ContractGenerationProviderIdentity,
) -> Result<(), ContractGenerationError> {
    if identity.provider.trim().is_empty()
        || identity.model.trim().is_empty()
        || identity.provider.len() > 1_024
        || identity.model.len() > 1_024
    {
        return Err(config_error(
            "provider and model identities must be non-empty and bounded",
        ));
    }
    Ok(())
}

pub(super) fn validate_context(
    context: &ContractContext,
    max_string_bytes: usize,
) -> Result<(), ContractGenerationError> {
    if context.audience.is_empty()
        || context.audience.len() > 64
        || context
            .audience
            .iter()
            .any(|value| value.trim().is_empty() || value.len() > max_string_bytes)
        || context.primary_outcome.trim().is_empty()
        || context.primary_outcome.len() > max_string_bytes
    {
        return Err(source_error("contract context is empty or unbounded"));
    }
    Ok(())
}

pub(super) fn validate_identifier(
    value: &str,
    description: &str,
) -> Result<(), ContractGenerationError> {
    if valid_identifier(value) {
        Ok(())
    } else {
        Err(source_error(format!("{description} is invalid")))
    }
}

pub(super) fn validate_sources(
    sources: &[ContractSource],
    options: &ContractGenerationOptions,
) -> Result<(), ContractGenerationError> {
    if sources.is_empty() || sources.len() > options.max_sources {
        return Err(source_error(format!(
            "source count must be between 1 and {}",
            options.max_sources
        )));
    }
    let mut ids = HashSet::new();
    for source in sources {
        validate_identifier(&source.id, "source identifier")?;
        if !ids.insert(source.id.as_str()) {
            return Err(source_error(format!(
                "source identifier '{}' is duplicated",
                source.id
            )));
        }
        if source.uri.trim().is_empty()
            || source.uri.len() > options.max_string_bytes
            || !valid_relative_uri(&source.uri)
            || source.path.trim().is_empty()
            || source.path.len() > MAX_PATH_BYTES
            || !valid_sha256(&source.sha256)
        {
            return Err(source_error(format!(
                "source '{}' has an invalid URI, path, or SHA-256 digest",
                source.id
            )));
        }
        match source.kind {
            ContractSourceKind::Prd => {
                if source.media_type.is_some() || source.width.is_some() || source.height.is_some()
                {
                    return Err(source_error(
                        "PRD sources do not accept image media type or dimensions",
                    ));
                }
            }
            ContractSourceKind::Design => {
                if !source.media_type.as_deref().is_some_and(|value| {
                    value.starts_with("image/") && value.len() > "image/".len()
                }) || !valid_dimensions(source.width, source.height)
                {
                    return Err(source_error(
                        "design sources require an image media type and positive bounded dimensions",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) async fn read_verified_sources(
    sources: &[ContractSource],
    max_source_bytes: usize,
) -> Result<VerifiedSourceBytes, ContractGenerationError> {
    let mut verified = HashMap::with_capacity(sources.len());
    for source in sources {
        let metadata = tokio::fs::symlink_metadata(&source.path)
            .await
            .map_err(|error| source_error(format!("failed to inspect source: {error}")))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(source_error(
                "contract source must be a regular non-symbolic-link file",
            ));
        }
        let bytes = tokio::fs::read(&source.path)
            .await
            .map_err(|error| source_error(format!("failed to read source: {error}")))?;
        if bytes.is_empty() || bytes.len() > max_source_bytes {
            return Err(source_error(format!(
                "contract source must contain 1 to {max_source_bytes} bytes"
            )));
        }
        let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
        if digest != source.sha256 {
            return Err(ContractGenerationError::new(
                "test.agent.contract_generation.source_mismatch",
                format!(
                    "source '{}' bytes do not match its SHA-256 digest",
                    source.id
                ),
                false,
            ));
        }
        if source.kind == ContractSourceKind::Prd {
            std::str::from_utf8(&bytes).map_err(|_| {
                source_error(format!("PRD source '{}' must be valid UTF-8", source.id))
            })?;
        }
        verified.insert(source.id.clone(), bytes);
    }
    Ok(verified)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_response(
    context: &ContractContext,
    sources: &[ContractSource],
    verified_sources: &VerifiedSourceBytes,
    max_cost_microusd: u64,
    response: &ContractGenerationProviderResponse,
    expected_identity: &ContractGenerationProviderIdentity,
    options: &ContractGenerationOptions,
) -> Result<(), ContractGenerationError> {
    validate_identity(&response.identity)?;
    if response.identity != *expected_identity {
        return Err(response_mismatch("provider identity changed"));
    }
    if response.usage.cost_microusd > max_cost_microusd {
        return Err(ContractGenerationError::new(
            "test.agent.contract_generation.cost_budget_exceeded",
            "contract generation provider reported cost above the admitted budget",
            false,
        ));
    }
    if response
        .request_id
        .as_ref()
        .is_some_and(|value| value.trim().is_empty() || value.len() > MAX_REQUEST_ID_BYTES)
    {
        return Err(response_error("provider request ID is invalid"));
    }
    validate_provenance_binding(sources, &response.source_digests)?;
    if response.candidates.is_empty() || response.candidates.len() > options.max_candidates {
        return Err(response_error(format!(
            "candidate count must be between 1 and {}",
            options.max_candidates
        )));
    }

    let sources_by_id = sources
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<HashMap<_, _>>();
    let mut total_elements = 0usize;
    let mut candidate_ids = HashSet::new();
    for candidate in &response.candidates {
        let source = sources_by_id
            .get(candidate.source_id.as_str())
            .ok_or_else(|| response_error("candidate references an unknown source"))?;
        let source_bytes = verified_sources
            .get(candidate.source_id.as_str())
            .ok_or_else(|| response_mismatch("verified source bytes are missing"))?;
        if candidate.context != *context {
            return Err(response_mismatch(
                "candidate context does not match the requested contract context",
            ));
        }
        if candidate.variants.is_empty() {
            return Err(response_error("candidate requires at least one variant"));
        }
        validate_decisions(
            &candidate.unresolved_decisions,
            source,
            source_bytes,
            options,
        )?;
        let mut variant_ids = HashSet::new();
        for variant in &candidate.variants {
            validate_candidate_variant(variant, options)?;
            if !variant_ids.insert(variant.id.as_str()) {
                return Err(response_error(format!(
                    "candidate source '{}' contains duplicate variant '{}'",
                    candidate.source_id, variant.id
                )));
            }
            total_elements = total_elements
                .checked_add(variant.elements.len())
                .ok_or_else(|| response_error("candidate element count overflowed"))?;
            if total_elements > options.max_elements {
                return Err(response_error(
                    "provider returned too many candidate elements",
                ));
            }
            validate_variant_hierarchy(variant)?;
            for element in &variant.elements {
                let candidate_id = candidate_id(candidate, variant, element);
                if !candidate_ids.insert(candidate_id.clone()) {
                    return Err(response_error(format!(
                        "candidate ID '{candidate_id}' is duplicated"
                    )));
                }
                validate_candidate_element(
                    &candidate_id,
                    element,
                    variant,
                    source,
                    source_bytes,
                    &candidate.unresolved_decisions,
                    options,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_provenance_binding(
    sources: &[ContractSource],
    returned: &[GeneratedContractProvenance],
) -> Result<(), ContractGenerationError> {
    if sources.len() != returned.len() {
        return Err(response_mismatch(
            "provider did not return one provenance binding per source",
        ));
    }
    let expected = sources
        .iter()
        .map(|source| {
            (
                source.id.as_str(),
                source.kind,
                source.uri.as_str(),
                source.sha256.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual = returned
        .iter()
        .map(|source| {
            (
                source.source_id.as_str(),
                source.kind,
                source.uri.as_str(),
                source.sha256.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    if expected != actual || actual.len() != returned.len() {
        return Err(response_mismatch(
            "provider provenance does not exactly match the admitted sources",
        ));
    }
    Ok(())
}

fn validate_candidate_variant(
    variant: &ContractCandidateVariant,
    options: &ContractGenerationOptions,
) -> Result<(), ContractGenerationError> {
    validate_identifier(&variant.id, "variant identifier")?;
    if variant.state.trim().is_empty()
        || variant.state.len() > options.max_string_bytes
        || variant.elements.is_empty()
        || variant
            .min_width
            .zip(variant.max_width)
            .is_some_and(|(min, max)| min > max)
        || variant
            .language
            .as_ref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > options.max_string_bytes)
    {
        return Err(response_error("candidate variant is empty or invalid"));
    }
    Ok(())
}

fn validate_variant_hierarchy(
    variant: &ContractCandidateVariant,
) -> Result<(), ContractGenerationError> {
    let by_id = variant
        .elements
        .iter()
        .map(|element| (element.element.id.as_str(), element))
        .collect::<HashMap<_, _>>();
    if by_id.len() != variant.elements.len() {
        return Err(response_error(format!(
            "variant '{}' contains duplicate element identifiers",
            variant.id
        )));
    }
    for element in &variant.elements {
        let mut seen = HashSet::new();
        let mut current = Some(element.element.id.as_str());
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(response_error(format!(
                    "variant '{}' contains a cyclic element hierarchy",
                    variant.id
                )));
            }
            current = by_id
                .get(id)
                .and_then(|value| value.element.parent.as_deref());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_candidate_element(
    candidate_id: &str,
    candidate: &ContractCandidateElement,
    variant: &ContractCandidateVariant,
    source: &ContractSource,
    source_bytes: &[u8],
    decisions: &[ProductDecision],
    options: &ContractGenerationOptions,
) -> Result<(), ContractGenerationError> {
    validate_identifier(&candidate.element.id, "element identifier")?;
    validate_element_strings(&candidate.element, options.max_string_bytes)?;
    if candidate.confidence > 100
        || candidate.element.test_id.is_none()
            && candidate.element.component_id.is_none()
            && candidate.element.role.is_none()
        || !candidate.element.citations.is_empty()
        || candidate.source_spans.len() > MAX_SPANS_PER_ITEM
    {
        return Err(response_error(format!(
            "candidate '{candidate_id}' has invalid confidence, identity, citations, or source spans"
        )));
    }
    let variant_ids = variant
        .elements
        .iter()
        .map(|element| element.element.id.as_str())
        .collect::<HashSet<_>>();
    if candidate
        .element
        .parent
        .as_ref()
        .is_some_and(|parent| !variant_ids.contains(parent.as_str()))
    {
        return Err(response_error(format!(
            "candidate '{candidate_id}' references an unknown parent"
        )));
    }
    for span in &candidate.source_spans {
        validate_source_span(span, source, source_bytes, options.max_string_bytes)?;
    }
    match (source.kind, &candidate.design_region) {
        (ContractSourceKind::Prd, None) if !candidate.source_spans.is_empty() => {}
        (ContractSourceKind::Prd, None) => {
            return Err(response_error(format!(
                "PRD candidate '{candidate_id}' requires at least one exact source span"
            )));
        }
        (ContractSourceKind::Prd, Some(_)) => {
            return Err(response_error(
                "PRD candidates cannot include design regions",
            ));
        }
        (ContractSourceKind::Design, Some(region)) => {
            validate_design_region(region, source, &variant_ids)?;
            if region.parent_candidate_id != candidate.element.parent {
                return Err(response_error(format!(
                    "design candidate '{candidate_id}' has inconsistent semantic and geometric hierarchy"
                )));
            }
        }
        (ContractSourceKind::Design, None) => {
            return Err(response_error(
                "design candidates require a digest-bound coordinate region",
            ));
        }
    }
    let decision_ids = decisions
        .iter()
        .map(|decision| decision.id.as_str())
        .collect::<HashSet<_>>();
    let mut referenced_decisions = HashSet::new();
    for id in &candidate.unresolved_decision_ids {
        if !valid_identifier(id)
            || !referenced_decisions.insert(id.as_str())
            || !decision_ids.contains(id.as_str())
        {
            return Err(response_error(format!(
                "candidate '{candidate_id}' references an invalid, duplicate, or unknown product decision"
            )));
        }
    }
    Ok(())
}

fn validate_element_strings(
    element: &ContractElement,
    max_string_bytes: usize,
) -> Result<(), ContractGenerationError> {
    for (name, value) in [
        ("test_id", element.test_id.as_ref()),
        ("component_id", element.component_id.as_ref()),
        ("role", element.role.as_ref()),
        ("name", element.name.as_ref()),
        ("description", element.description.as_ref()),
        ("parent", element.parent.as_ref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty() || value.len() > max_string_bytes) {
            return Err(response_error(format!(
                "candidate element {name} is empty or unbounded"
            )));
        }
    }
    Ok(())
}

fn validate_source_span(
    span: &ContractSourceSpan,
    source: &ContractSource,
    source_bytes: &[u8],
    max_string_bytes: usize,
) -> Result<(), ContractGenerationError> {
    if span.source_id != source.id
        || span.quote.trim().is_empty()
        || span.quote.len() > max_string_bytes
        || span.start >= span.end
    {
        return Err(response_error("source span is invalid"));
    }
    let start = usize::try_from(span.start).map_err(|_| response_error("source span overflow"))?;
    let end = usize::try_from(span.end).map_err(|_| response_error("source span overflow"))?;
    if end > source_bytes.len() || source_bytes.get(start..end) != Some(span.quote.as_bytes()) {
        return Err(response_error(
            "quoted source span does not match the admitted source bytes",
        ));
    }
    Ok(())
}

fn validate_design_region(
    region: &super::DesignElementRegion,
    source: &ContractSource,
    variant_ids: &HashSet<&str>,
) -> Result<(), ContractGenerationError> {
    if region.source_id != source.id
        || ![region.x, region.y, region.width, region.height]
            .into_iter()
            .all(f64::is_finite)
        || region.x < 0.0
        || region.y < 0.0
        || region.width <= 0.0
        || region.height <= 0.0
        || region
            .parent_candidate_id
            .as_ref()
            .is_some_and(|id| !variant_ids.contains(id.as_str()))
    {
        return Err(response_error("design candidate region is invalid"));
    }
    let (max_x, max_y) = match region.coordinate_space {
        DesignCoordinateSpace::ImagePixels => source
            .width
            .zip(source.height)
            .map(|(width, height)| (f64::from(width), f64::from(height)))
            .ok_or_else(|| response_error("design source dimensions are missing"))?,
        DesignCoordinateSpace::Normalized => (1.0, 1.0),
    };
    if region.x + region.width > max_x || region.y + region.height > max_y {
        return Err(response_error(
            "design candidate region is outside its declared coordinate space",
        ));
    }
    Ok(())
}

fn validate_decisions(
    decisions: &[ProductDecision],
    source: &ContractSource,
    source_bytes: &[u8],
    options: &ContractGenerationOptions,
) -> Result<(), ContractGenerationError> {
    if decisions.len() > options.max_elements {
        return Err(response_error(
            "provider returned too many product decisions",
        ));
    }
    let mut ids = HashSet::new();
    for decision in decisions {
        validate_identifier(&decision.id, "product decision identifier")?;
        if !ids.insert(decision.id.as_str())
            || decision.question.trim().is_empty()
            || decision.question.len() > options.max_string_bytes
            || decision.source_spans.len() > MAX_SPANS_PER_ITEM
        {
            return Err(response_error("product decision is duplicate or invalid"));
        }
        for span in &decision.source_spans {
            validate_source_span(span, source, source_bytes, options.max_string_bytes)?;
        }
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn valid_relative_uri(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn valid_dimensions(width: Option<u32>, height: Option<u32>) -> bool {
    width.zip(height).is_some_and(|(width, height)| {
        width > 0 && height > 0 && width <= MAX_DIMENSION && height <= MAX_DIMENSION
    })
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn config_error(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.config_invalid",
        message,
        false,
    )
}

fn source_error(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.source_invalid",
        message,
        false,
    )
}

fn response_error(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.response_invalid",
        message,
        false,
    )
}

fn response_mismatch(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.response_mismatch",
        message,
        false,
    )
}
