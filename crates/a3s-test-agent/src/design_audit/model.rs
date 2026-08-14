use std::fmt;
use std::time::Duration;

use a3s_test_core::{
    DesignAuditDimension, DesignAuditFinding, DesignAuditProviderIdentity, DesignAuditUsage,
    PageContextSnapshot,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditRequest {
    pub screenshot_path: String,
    pub screenshot_sha256: String,
    pub width: u32,
    pub height: u32,
    pub observation_id: u64,
    pub surface_revision: u64,
    pub page_context: PageContextSnapshot,
    pub dimensions: Vec<DesignAuditDimension>,
    pub max_cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditProviderRequest {
    /// Adapter-owned evidence path for in-process or shared-filesystem providers.
    /// HTTP adapters replace this with a logical attachment name and send the
    /// admitted bytes in their versioned envelope.
    pub screenshot_path: String,
    pub screenshot_sha256: String,
    pub page_context_sha256: String,
    pub width: u32,
    pub height: u32,
    pub observation_id: u64,
    pub surface_revision: u64,
    pub page_context: PageContextSnapshot,
    pub dimensions: Vec<DesignAuditDimension>,
    pub issued_at_unix_ms: u64,
    pub deadline_unix_ms: u64,
    pub max_cost_microusd: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditProviderResponse {
    pub identity: DesignAuditProviderIdentity,
    pub observation_id: u64,
    pub surface_revision: u64,
    pub screenshot_sha256: String,
    pub page_context_sha256: String,
    pub width: u32,
    pub height: u32,
    pub dimensions: Vec<DesignAuditDimension>,
    pub findings: Vec<DesignAuditFinding>,
    pub usage: DesignAuditUsage,
    pub request_id: Option<String>,
}

#[derive(Clone, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignAuditImageAttachment {
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub screenshot_sha256: String,
    #[schemars(regex(pattern = r"^image/png$"))]
    pub media_type: String,
    #[schemars(length(min = 4, max = 44739244))]
    pub bytes_base64: String,
}

impl fmt::Debug for DesignAuditImageAttachment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesignAuditImageAttachment")
            .field("screenshot_sha256", &self.screenshot_sha256)
            .field("media_type", &self.media_type)
            .field("bytes_base64", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct DesignAuditOptions {
    pub timeout: Duration,
    pub max_findings: usize,
    pub max_summary_bytes: usize,
    pub max_rationale_bytes: usize,
    pub max_recommendation_bytes: usize,
    pub max_page_context_bytes: usize,
}

impl Default for DesignAuditOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_findings: 100,
            max_summary_bytes: 2 * 1_024,
            max_rationale_bytes: 8 * 1_024,
            max_recommendation_bytes: 8 * 1_024,
            max_page_context_bytes: 8 * 1_024 * 1_024,
        }
    }
}
