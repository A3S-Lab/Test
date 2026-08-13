use std::time::Duration;

use a3s_test_core::{
    ContractContext, ContractElement, ContractMode, ContractProvenanceKind, ContractSeverity,
    PageContextTheme,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContractSourceKind {
    Prd,
    Design,
}

impl ContractSourceKind {
    #[must_use]
    pub fn provenance_kind(self) -> ContractProvenanceKind {
        match self {
            Self::Prd => ContractProvenanceKind::Prd,
            Self::Design => ContractProvenanceKind::Design,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractSource {
    pub id: String,
    pub kind: ContractSourceKind,
    pub uri: String,
    pub path: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationProviderIdentity {
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationProviderRequest {
    pub contract_name: String,
    #[serde(with = "contract_context_wire")]
    #[schemars(with = "ContractContextWire")]
    pub context: ContractContext,
    pub sources: Vec<ContractSource>,
    pub issued_at_unix_ms: u64,
    pub deadline_unix_ms: u64,
    pub max_cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractSourceSpan {
    pub source_id: String,
    pub quote: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignCoordinateSpace {
    ImagePixels,
    Normalized,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignElementRegion {
    pub source_id: String,
    pub coordinate_space: DesignCoordinateSpace,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_candidate_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductDecisionStatus {
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProductDecision {
    pub id: String,
    pub question: String,
    pub status: ProductDecisionStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<ContractSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCandidateElement {
    #[serde(with = "contract_element_wire")]
    #[schemars(with = "ContractCandidateWireElement")]
    pub element: ContractElement,
    pub confidence: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<ContractSourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_region: Option<DesignElementRegion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_decision_ids: Vec<String>,
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct ContractCandidateWireElement {
    id: String,
    test_id: Option<String>,
    component_id: Option<String>,
    role: Option<String>,
    name: Option<String>,
    description: Option<String>,
    required: bool,
    visible: Option<bool>,
    enabled: Option<bool>,
    checked: Option<bool>,
    selected: Option<bool>,
    expanded: Option<bool>,
    readonly: Option<bool>,
    form_required: Option<bool>,
    invalid: Option<bool>,
    parent: Option<String>,
    severity: ContractSeverity,
}

impl From<&ContractElement> for ContractCandidateWireElement {
    fn from(element: &ContractElement) -> Self {
        Self {
            id: element.id.clone(),
            test_id: element.test_id.clone(),
            component_id: element.component_id.clone(),
            role: element.role.clone(),
            name: element.name.clone(),
            description: element.description.clone(),
            required: element.required,
            visible: element.visible,
            enabled: element.enabled,
            checked: element.checked,
            selected: element.selected,
            expanded: element.expanded,
            readonly: element.readonly,
            form_required: element.form_required,
            invalid: element.invalid,
            parent: element.parent.clone(),
            severity: element.severity,
        }
    }
}

impl From<ContractCandidateWireElement> for ContractElement {
    fn from(element: ContractCandidateWireElement) -> Self {
        Self {
            id: element.id,
            test_id: element.test_id,
            component_id: element.component_id,
            role: element.role,
            name: element.name,
            description: element.description,
            required: element.required,
            visible: element.visible,
            enabled: element.enabled,
            checked: element.checked,
            selected: element.selected,
            expanded: element.expanded,
            readonly: element.readonly,
            form_required: element.form_required,
            invalid: element.invalid,
            parent: element.parent,
            severity: element.severity,
            citations: Vec::new(),
        }
    }
}

mod contract_element_wire {
    use serde::{ser::Error as _, Deserialize, Deserializer, Serializer};

    use super::{ContractCandidateWireElement, ContractElement};

    pub(super) fn serialize<S>(element: &ContractElement, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if !element.citations.is_empty() {
            return Err(S::Error::custom(
                "contract-generation provider elements cannot contain approved citations",
            ));
        }
        serde::Serialize::serialize(&ContractCandidateWireElement::from(element), serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<ContractElement, D::Error>
    where
        D: Deserializer<'de>,
    {
        ContractCandidateWireElement::deserialize(deserializer).map(ContractElement::from)
    }
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct ContractContextWire {
    mode: ContractMode,
    audience: Vec<String>,
    primary_outcome: String,
}

impl From<&ContractContext> for ContractContextWire {
    fn from(context: &ContractContext) -> Self {
        Self {
            mode: context.mode,
            audience: context.audience.clone(),
            primary_outcome: context.primary_outcome.clone(),
        }
    }
}

impl From<ContractContextWire> for ContractContext {
    fn from(context: ContractContextWire) -> Self {
        Self {
            mode: context.mode,
            audience: context.audience,
            primary_outcome: context.primary_outcome,
        }
    }
}

mod contract_context_wire {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::{ContractContext, ContractContextWire};

    pub(super) fn serialize<S>(context: &ContractContext, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde::Serialize::serialize(&ContractContextWire::from(context), serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<ContractContext, D::Error>
    where
        D: Deserializer<'de>,
    {
        ContractContextWire::deserialize(deserializer).map(ContractContext::from)
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCandidateVariant {
    pub id: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<PageContextTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub elements: Vec<ContractCandidateElement>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCandidate {
    pub source_id: String,
    #[serde(with = "contract_context_wire")]
    #[schemars(with = "ContractContextWire")]
    pub context: ContractContext,
    pub variants: Vec<ContractCandidateVariant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_decisions: Vec<ProductDecision>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationProviderResponse {
    pub identity: ContractGenerationProviderIdentity,
    pub source_digests: Vec<GeneratedContractProvenance>,
    pub candidates: Vec<ContractCandidate>,
    pub usage: ContractGenerationUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedContractProvenance {
    pub source_id: String,
    pub kind: ContractSourceKind,
    pub uri: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractConflictStatus {
    Unresolved,
    Resolved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractConflictResolution {
    pub conflict_id: String,
    pub selected_candidate_id: String,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractConflict {
    pub id: String,
    pub variant_id: String,
    pub element_id: String,
    pub field: String,
    pub candidate_ids: Vec<String>,
    pub values: Vec<Value>,
    pub status: ContractConflictStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ContractConflictResolution>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedContractDraft {
    pub name: String,
    pub version: u32,
    pub context: ContractContext,
    pub provenance: Vec<GeneratedContractProvenance>,
    pub candidates: Vec<ContractCandidate>,
    pub conflicts: Vec<ContractConflict>,
    pub unresolved_decisions: Vec<ProductDecision>,
    pub usage: ContractGenerationUsage,
    pub provider: ContractGenerationProviderIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractReviewAction {
    Approve,
    Reject,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractReviewDecision {
    pub candidate_id: String,
    pub action: ContractReviewAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationReview {
    pub reviewer: String,
    pub decisions: Vec<ContractReviewDecision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflict_resolutions: Vec<ContractConflictResolution>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewedContractDraft {
    pub contract: a3s_test_core::SurfaceContractDraft,
    pub generated: GeneratedContractDraft,
    pub review: ContractGenerationReview,
}

impl ReviewedContractDraft {
    #[must_use]
    pub fn contract(&self) -> &a3s_test_core::SurfaceContractDraft {
        &self.contract
    }

    #[must_use]
    pub fn generated(&self) -> &GeneratedContractDraft {
        &self.generated
    }

    #[must_use]
    pub fn into_contract(self) -> a3s_test_core::SurfaceContractDraft {
        self.contract
    }
}

#[derive(Clone, Debug)]
pub struct ContractGenerationOptions {
    pub timeout: Duration,
    pub max_sources: usize,
    pub max_source_bytes: usize,
    pub max_candidates: usize,
    pub max_elements: usize,
    pub max_string_bytes: usize,
}

impl Default for ContractGenerationOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_sources: 8,
            max_source_bytes: 8 * 1_024 * 1_024,
            max_candidates: 64,
            max_elements: 1_024,
            max_string_bytes: 16 * 1_024,
        }
    }
}
