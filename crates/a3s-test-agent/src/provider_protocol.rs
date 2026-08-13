use schemars::Schema;
use serde::Serialize;

use crate::{
    ContractGenerationProviderRequest, ContractGenerationProviderResponse,
    GroundingProviderRequest, GroundingProviderResponse, HttpContractGenerationRequest,
    HttpContractGenerationResponse, HttpLlmCompletionRequest, HttpLlmCompletionResponse,
    HttpVisualGroundingRequest, HttpVisualGroundingResponse, StructuredLlmRequest,
    StructuredLlmResponse,
};

pub const CONTRACT_GENERATION_PROVIDER_PROTOCOL: &str = "a3s.test.contract-generation-provider/1";
pub const LLM_PROVIDER_PROTOCOL: &str = "a3s.test.llm-provider/1";
pub const VISUAL_GROUNDING_PROVIDER_PROTOCOL: &str = "a3s.test.visual-grounding-provider/2";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutputAuthority {
    CandidateOnly,
    Advisory,
    ProposalOnly,
}

#[must_use]
pub fn llm_provider_schema() -> ProviderProtocolSchema {
    ProviderProtocolSchema {
        protocol: LLM_PROVIDER_PROTOCOL,
        authority: ProviderOutputAuthority::ProposalOnly,
        invariants: ProviderSafetyInvariants {
            input_digest_bound: false,
            request_deadline_required: true,
            request_cost_ceiling_required: true,
            response_identity_bound: true,
            local_admission_required: true,
            observation_scoped_output: true,
            semantic_evidence_preferred: true,
            human_review_required_for_expected_surface: false,
            may_determine_test_verdict: false,
            may_authorize_repair: false,
            may_claim_browser_observation: false,
            may_propose_surface_actions: true,
        },
        request_schema: schemars::schema_for!(StructuredLlmRequest),
        response_schema: schemars::schema_for!(StructuredLlmResponse),
        http: HttpProviderProtocolSchema {
            method: "POST",
            content_type: "application/json",
            redirects_allowed: false,
            endpoint_policy: "https_or_loopback_http",
            request_envelope_schema: schemars::schema_for!(HttpLlmCompletionRequest),
            response_envelope_schema: schemars::schema_for!(HttpLlmCompletionResponse),
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderSafetyInvariants {
    pub input_digest_bound: bool,
    pub request_deadline_required: bool,
    pub request_cost_ceiling_required: bool,
    pub response_identity_bound: bool,
    pub local_admission_required: bool,
    pub observation_scoped_output: bool,
    pub semantic_evidence_preferred: bool,
    pub human_review_required_for_expected_surface: bool,
    pub may_determine_test_verdict: bool,
    pub may_authorize_repair: bool,
    pub may_claim_browser_observation: bool,
    pub may_propose_surface_actions: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProviderProtocolSchema {
    pub protocol: &'static str,
    pub authority: ProviderOutputAuthority,
    pub invariants: ProviderSafetyInvariants,
    pub request_schema: Schema,
    pub response_schema: Schema,
    pub http: HttpProviderProtocolSchema,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HttpProviderProtocolSchema {
    pub method: &'static str,
    pub content_type: &'static str,
    pub redirects_allowed: bool,
    pub endpoint_policy: &'static str,
    pub request_envelope_schema: Schema,
    pub response_envelope_schema: Schema,
}

#[must_use]
pub fn contract_generation_provider_schema() -> ProviderProtocolSchema {
    ProviderProtocolSchema {
        protocol: CONTRACT_GENERATION_PROVIDER_PROTOCOL,
        authority: ProviderOutputAuthority::CandidateOnly,
        invariants: ProviderSafetyInvariants {
            input_digest_bound: true,
            request_deadline_required: true,
            request_cost_ceiling_required: true,
            response_identity_bound: true,
            local_admission_required: true,
            observation_scoped_output: false,
            semantic_evidence_preferred: false,
            human_review_required_for_expected_surface: true,
            may_determine_test_verdict: false,
            may_authorize_repair: false,
            may_claim_browser_observation: false,
            may_propose_surface_actions: false,
        },
        request_schema: schemars::schema_for!(ContractGenerationProviderRequest),
        response_schema: schemars::schema_for!(ContractGenerationProviderResponse),
        http: HttpProviderProtocolSchema {
            method: "POST",
            content_type: "application/json",
            redirects_allowed: false,
            endpoint_policy: "https_or_loopback_http",
            request_envelope_schema: schemars::schema_for!(HttpContractGenerationRequest),
            response_envelope_schema: schemars::schema_for!(HttpContractGenerationResponse),
        },
    }
}

#[must_use]
pub fn visual_grounding_provider_schema() -> ProviderProtocolSchema {
    ProviderProtocolSchema {
        protocol: VISUAL_GROUNDING_PROVIDER_PROTOCOL,
        authority: ProviderOutputAuthority::Advisory,
        invariants: ProviderSafetyInvariants {
            input_digest_bound: true,
            request_deadline_required: true,
            request_cost_ceiling_required: true,
            response_identity_bound: true,
            local_admission_required: true,
            observation_scoped_output: true,
            semantic_evidence_preferred: true,
            human_review_required_for_expected_surface: false,
            may_determine_test_verdict: false,
            may_authorize_repair: false,
            may_claim_browser_observation: false,
            may_propose_surface_actions: false,
        },
        request_schema: schemars::schema_for!(GroundingProviderRequest),
        response_schema: schemars::schema_for!(GroundingProviderResponse),
        http: HttpProviderProtocolSchema {
            method: "POST",
            content_type: "application/json",
            redirects_allowed: false,
            endpoint_policy: "https_or_loopback_http",
            request_envelope_schema: schemars::schema_for!(HttpVisualGroundingRequest),
            response_envelope_schema: schemars::schema_for!(HttpVisualGroundingResponse),
        },
    }
}
