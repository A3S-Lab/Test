use std::time::Duration;

use a3s_test_core::{PageContextSnapshot, Target};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingProviderIdentity {
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundingCoordinateSpace {
    ScreenshotPixels,
    Normalized,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticFallbackReason {
    Canvas,
    ImageOnly,
    RemoteDesktop,
    DesignReference,
    NoSemanticMatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroundingTrigger {
    ExplicitRequest,
    SemanticFallback { reason: SemanticFallbackReason },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingRequest {
    pub screenshot_path: String,
    pub screenshot_sha256: String,
    pub width: u32,
    pub height: u32,
    pub query: String,
    pub observation_id: u64,
    pub trigger: GroundingTrigger,
    pub max_cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingProviderRequest {
    pub screenshot_path: String,
    pub screenshot_sha256: String,
    pub width: u32,
    pub height: u32,
    pub query: String,
    pub observation_id: u64,
    pub trigger: GroundingTrigger,
    pub issued_at_unix_ms: u64,
    pub deadline_unix_ms: u64,
    pub max_cost_microusd: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct GroundingPageContext<'a> {
    pub observation_id: u64,
    pub snapshot: &'a PageContextSnapshot,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroundingCandidateGeometry {
    Point {
        x: f64,
        y: f64,
    },
    Box {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingCandidate {
    pub geometry: GroundingCandidateGeometry,
    pub confidence: f64,
    pub label: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingUsage {
    pub input_units: u64,
    pub output_units: u64,
    pub cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingProviderResponse {
    pub identity: GroundingProviderIdentity,
    pub observation_id: u64,
    pub screenshot_sha256: String,
    pub width: u32,
    pub height: u32,
    pub coordinate_space: GroundingCoordinateSpace,
    pub candidates: Vec<GroundingCandidate>,
    pub usage: GroundingUsage,
    pub request_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundingAuthority {
    Advisory,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingProvenance {
    pub identity: GroundingProviderIdentity,
    pub observation_id: u64,
    pub screenshot_sha256: String,
    pub width: u32,
    pub height: u32,
    pub provider_coordinate_space: GroundingCoordinateSpace,
    pub usage: GroundingUsage,
    pub request_id: Option<String>,
    pub authority: GroundingAuthority,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingSemanticMatch {
    pub candidate_index: u32,
    pub node_id: String,
    pub reference: Option<String>,
    pub target: Target,
    pub screenshot_point: GroundingPoint,
    pub viewport_point: GroundingPoint,
    pub confidence: f64,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingImageCandidate {
    pub candidate_index: u32,
    pub geometry: GroundingCandidateGeometry,
    pub screenshot_point: GroundingPoint,
    pub viewport_point: Option<GroundingPoint>,
    pub confidence: f64,
    pub label: Option<String>,
    pub semantic_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "resolution", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroundingResult {
    Semantic {
        provenance: GroundingProvenance,
        matches: Vec<GroundingSemanticMatch>,
        image_bound_candidates: Vec<GroundingImageCandidate>,
    },
    ImageBound {
        provenance: GroundingProvenance,
        candidates: Vec<GroundingImageCandidate>,
    },
}

impl GroundingResult {
    #[must_use]
    pub fn provenance(&self) -> &GroundingProvenance {
        match self {
            Self::Semantic { provenance, .. } | Self::ImageBound { provenance, .. } => provenance,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GroundingOptions {
    pub timeout: Duration,
    pub max_candidates: usize,
    pub max_query_bytes: usize,
    pub max_label_bytes: usize,
}

impl Default for GroundingOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            max_candidates: 32,
            max_query_bytes: 4 * 1_024,
            max_label_bytes: 1_024,
        }
    }
}
