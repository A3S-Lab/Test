use std::time::Duration;

use a3s_test_core::{ContractContext, ContractElement, ContractProvenanceKind, PageContextTheme};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationProviderIdentity {
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationProviderRequest {
    pub contract_name: String,
    pub context: ContractContext,
    pub sources: Vec<ContractSource>,
    pub issued_at_unix_ms: u64,
    pub deadline_unix_ms: u64,
    pub max_cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractSourceSpan {
    pub source_id: String,
    pub quote: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignCoordinateSpace {
    ImagePixels,
    Normalized,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductDecisionStatus {
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProductDecision {
    pub id: String,
    pub question: String,
    pub status: ProductDecisionStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<ContractSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCandidateElement {
    pub element: ContractElement,
    pub confidence: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<ContractSourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_region: Option<DesignElementRegion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_decision_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCandidate {
    pub source_id: String,
    pub context: ContractContext,
    pub variants: Vec<ContractCandidateVariant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_decisions: Vec<ProductDecision>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationProviderResponse {
    pub identity: ContractGenerationProviderIdentity,
    pub source_digests: Vec<GeneratedContractProvenance>,
    pub candidates: Vec<ContractCandidate>,
    pub usage: ContractGenerationUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
