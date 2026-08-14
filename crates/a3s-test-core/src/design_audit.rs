use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DESIGN_AUDIT_REPORT_PROTOCOL: &str = "a3s.test.design-audit-report/1";

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DesignAuditDimension {
    VisualHierarchy,
    LayoutComposition,
    SpacingRhythm,
    Typography,
    ColorUse,
    Consistency,
    InteractionClarity,
    ContentClarity,
    ResponsiveComposition,
}

impl DesignAuditDimension {
    pub const ALL: [Self; 9] = [
        Self::VisualHierarchy,
        Self::LayoutComposition,
        Self::SpacingRhythm,
        Self::Typography,
        Self::ColorUse,
        Self::Consistency,
        Self::InteractionClarity,
        Self::ContentClarity,
        Self::ResponsiveComposition,
    ];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignAuditPriority {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditProviderIdentity {
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditNormalizedRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesignAuditTarget {
    Page,
    Node { node_id: String },
    Region { region: DesignAuditNormalizedRegion },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditFinding {
    pub id: String,
    pub dimension: DesignAuditDimension,
    pub priority: DesignAuditPriority,
    pub summary: String,
    pub rationale: String,
    pub recommendation: String,
    pub confidence: u8,
    pub target: DesignAuditTarget,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditUsage {
    pub input_units: u64,
    pub output_units: u64,
    pub cost_microusd: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignAuditAuthority {
    Advisory,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditProvenance {
    pub identity: DesignAuditProviderIdentity,
    pub observation_id: u64,
    pub surface_revision: u64,
    pub screenshot_sha256: String,
    pub page_context_sha256: String,
    pub width: u32,
    pub height: u32,
    pub usage: DesignAuditUsage,
    pub request_id: Option<String>,
    pub authority: DesignAuditAuthority,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditReport {
    pub protocol: String,
    pub provenance: DesignAuditProvenance,
    pub dimensions: Vec<DesignAuditDimension>,
    pub findings: Vec<DesignAuditFinding>,
}
