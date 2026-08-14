//! Schema-constrained model execution, contract generation, visual grounding, and design audit for A3S Test.

mod contract_generation;
mod design_audit;
mod error;
mod grounding;
mod http_provider;
mod model;
mod policy;
mod provider;
mod provider_protocol;
mod redaction;
mod runtime;

pub use a3s_test_core::{
    DesignAuditAuthority, DesignAuditDimension, DesignAuditFinding, DesignAuditNormalizedRegion,
    DesignAuditPriority, DesignAuditProvenance, DesignAuditProviderIdentity, DesignAuditReport,
    DesignAuditTarget, DesignAuditUsage, DESIGN_AUDIT_REPORT_PROTOCOL,
};
pub use contract_generation::{
    ContractCandidate, ContractCandidateElement, ContractCandidateVariant, ContractConflict,
    ContractConflictResolution, ContractConflictStatus, ContractGenerationOptions,
    ContractGenerationProvider, ContractGenerationProviderIdentity,
    ContractGenerationProviderRequest, ContractGenerationProviderResponse,
    ContractGenerationReview, ContractGenerationService, ContractGenerationUsage,
    ContractReviewAction, ContractReviewDecision, ContractSource, ContractSourceKind,
    ContractSourceSpan, ContractWorkflowAdmission, ContractWorkflowArtifact, ContractWorkflowStage,
    DesignCoordinateSpace, DesignElementRegion, GeneratedContractDraft,
    GeneratedContractProvenance, ProductDecision, ProductDecisionStatus, ReviewedContractDraft,
    CONTRACT_WORKFLOW_PROTOCOL,
};
pub use design_audit::{
    DesignAuditImageAttachment, DesignAuditOptions, DesignAuditProvider,
    DesignAuditProviderRequest, DesignAuditProviderResponse, DesignAuditRequest,
    DesignAuditService,
};
pub use error::{AgentError, ContractGenerationError, DesignAuditError, GroundingError, LlmError};
pub use grounding::{
    GroundingAuthority, GroundingCandidate, GroundingCandidateGeometry, GroundingCoordinateSpace,
    GroundingImageAttachment, GroundingImageCandidate, GroundingOptions, GroundingPageContext,
    GroundingPoint, GroundingProvenance, GroundingProviderIdentity, GroundingProviderRequest,
    GroundingProviderResponse, GroundingRequest, GroundingResult, GroundingSemanticMatch,
    GroundingTrigger, GroundingUsage, SemanticFallbackReason, VisualGroundingProvider,
    VisualGroundingService,
};
pub use http_provider::{
    HttpContractGenerationProvider, HttpContractGenerationRequest, HttpContractGenerationResponse,
    HttpDesignAuditProvider, HttpDesignAuditRequest, HttpDesignAuditResponse,
    HttpLlmCompletionRequest, HttpLlmCompletionResponse, HttpLlmProvider, HttpProviderConfig,
    HttpProviderConfigError, HttpProviderEndpoint, HttpProviderErrorResponse,
    HttpVisualGroundingProvider, HttpVisualGroundingRequest, HttpVisualGroundingResponse,
};
pub use model::{
    ActionHistory, AgentDecision, AgentGoal, AgentOptions, AgentRunResult, AgentStatus, AgentTurn,
    LlmIdentity, LlmUsage, PlannerContext, RemainingBudget,
};
pub use policy::{ActionKind, ActionPolicy, CapabilityPolicy, NavigationScope, PolicyContext};
pub use provider::{LlmImageAttachment, LlmProvider, StructuredLlmRequest, StructuredLlmResponse};
pub use provider_protocol::{
    contract_generation_provider_schema, design_audit_provider_schema, llm_provider_schema,
    visual_grounding_provider_schema, HttpProviderProtocolSchema, ProviderOutputAuthority,
    ProviderProtocolSchema, ProviderSafetyInvariants, CONTRACT_GENERATION_PROVIDER_PROTOCOL,
    DESIGN_AUDIT_PROVIDER_PROTOCOL, LLM_PROVIDER_PROTOCOL, VISUAL_GROUNDING_PROVIDER_PROTOCOL,
};
pub use redaction::{ProvenanceRedactor, REDACTED_VALUE};
pub use runtime::{AgentLoop, AGENT_PROMPT_VERSION};
