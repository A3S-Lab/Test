use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{LlmError, LlmIdentity, LlmUsage, PlannerContext};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredLlmRequest {
    pub prompt_version: String,
    pub system_instruction: String,
    pub context: PlannerContext,
    pub image_attachments: Vec<LlmImageAttachment>,
    pub response_schema: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LlmImageAttachment {
    pub name: String,
    pub path: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredLlmResponse {
    pub decision: Value,
    pub usage: LlmUsage,
    pub request_id: Option<String>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn identity(&self) -> LlmIdentity;

    /// Execute one real LLM request constrained by `response_schema`.
    ///
    /// Implementations must not replace model inference with keyword routing.
    /// The agent loop independently validates the returned JSON before any
    /// proposed surface action is executed.
    async fn complete(
        &self,
        request: StructuredLlmRequest,
    ) -> Result<StructuredLlmResponse, LlmError>;
}
