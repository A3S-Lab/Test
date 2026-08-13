use schemars::Schema;
use serde::Serialize;

use crate::{
    ContractGenerationProviderRequest, ContractGenerationProviderResponse,
    GroundingProviderRequest, GroundingProviderResponse,
};

pub const CONTRACT_GENERATION_PROVIDER_PROTOCOL: &str = "a3s.test.contract-generation-provider/1";
pub const VISUAL_GROUNDING_PROVIDER_PROTOCOL: &str = "a3s.test.visual-grounding-provider/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutputAuthority {
    CandidateOnly,
    Advisory,
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
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProviderProtocolSchema {
    pub protocol: &'static str,
    pub authority: ProviderOutputAuthority,
    pub invariants: ProviderSafetyInvariants,
    pub request_schema: Schema,
    pub response_schema: Schema,
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
        },
        request_schema: schemars::schema_for!(ContractGenerationProviderRequest),
        response_schema: schemars::schema_for!(ContractGenerationProviderResponse),
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
        },
        request_schema: schemars::schema_for!(GroundingProviderRequest),
        response_schema: schemars::schema_for!(GroundingProviderResponse),
    }
}
