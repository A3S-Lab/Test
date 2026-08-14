use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::{
    ContractGenerationProviderRequest, ContractGenerationProviderResponse,
    DesignAuditImageAttachment, DesignAuditProviderRequest, DesignAuditProviderResponse,
    GroundingImageAttachment, GroundingProviderRequest, GroundingProviderResponse,
    StructuredLlmRequest, StructuredLlmResponse,
};

fn contract_generation_protocol_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": "a3s.test.contract-generation-provider/1"
    })
}

fn visual_grounding_protocol_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": "a3s.test.visual-grounding-provider/2"
    })
}

fn design_audit_protocol_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": "a3s.test.design-audit-provider/1"
    })
}

fn llm_protocol_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": "a3s.test.llm-provider/1"
    })
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpProviderErrorResponse {
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = r"^[a-z0-9_]+(?:\.[a-z0-9_]+)*$")
    )]
    pub code: String,
    #[schemars(length(min = 1, max = 65536))]
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpContractGenerationRequest {
    #[schemars(schema_with = "contract_generation_protocol_schema")]
    pub protocol: String,
    pub request: ContractGenerationProviderRequest,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum HttpContractGenerationResponse {
    Success {
        #[schemars(schema_with = "contract_generation_protocol_schema")]
        protocol: String,
        response: ContractGenerationProviderResponse,
    },
    Failure {
        #[schemars(schema_with = "contract_generation_protocol_schema")]
        protocol: String,
        error: HttpProviderErrorResponse,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpVisualGroundingRequest {
    #[schemars(schema_with = "visual_grounding_protocol_schema")]
    pub protocol: String,
    pub request: GroundingProviderRequest,
    pub image: GroundingImageAttachment,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum HttpVisualGroundingResponse {
    Success {
        #[schemars(schema_with = "visual_grounding_protocol_schema")]
        protocol: String,
        response: GroundingProviderResponse,
    },
    Failure {
        #[schemars(schema_with = "visual_grounding_protocol_schema")]
        protocol: String,
        error: HttpProviderErrorResponse,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpDesignAuditRequest {
    #[schemars(schema_with = "design_audit_protocol_schema")]
    pub protocol: String,
    pub request: DesignAuditProviderRequest,
    pub image: DesignAuditImageAttachment,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum HttpDesignAuditResponse {
    Success {
        #[schemars(schema_with = "design_audit_protocol_schema")]
        protocol: String,
        response: DesignAuditProviderResponse,
    },
    Failure {
        #[schemars(schema_with = "design_audit_protocol_schema")]
        protocol: String,
        error: HttpProviderErrorResponse,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpLlmCompletionRequest {
    #[schemars(schema_with = "llm_protocol_schema")]
    pub protocol: String,
    pub request: StructuredLlmRequest,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum HttpLlmCompletionResponse {
    Success {
        #[schemars(schema_with = "llm_protocol_schema")]
        protocol: String,
        response: StructuredLlmResponse,
    },
    Failure {
        #[schemars(schema_with = "llm_protocol_schema")]
        protocol: String,
        error: HttpProviderErrorResponse,
    },
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HttpProviderRequestEnvelope<'a, Request> {
    pub(super) protocol: &'static str,
    pub(super) request: &'a Request,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum HttpProviderResponseEnvelope<Response> {
    Success {
        protocol: String,
        response: Response,
    },
    Failure {
        protocol: String,
        error: HttpProviderRemoteError,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HttpProviderRemoteError {
    pub(super) code: String,
    pub(super) message: String,
    pub(super) retryable: bool,
}
