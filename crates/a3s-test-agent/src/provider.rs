use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{LlmError, LlmIdentity, LlmUsage, PlannerContext};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StructuredLlmRequest {
    pub prompt_version: String,
    pub system_instruction: String,
    pub context: PlannerContext,
    pub response_schema: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
