//! Schema-constrained model execution, contract generation, and visual grounding for A3S Test.

mod contract_generation;
mod error;
mod grounding;
mod model;
mod policy;
mod provider;
mod redaction;
mod runtime;

pub use contract_generation::{
    ContractCandidate, ContractCandidateElement, ContractCandidateVariant, ContractConflict,
    ContractConflictResolution, ContractConflictStatus, ContractGenerationOptions,
    ContractGenerationProvider, ContractGenerationProviderIdentity,
    ContractGenerationProviderRequest, ContractGenerationProviderResponse,
    ContractGenerationReview, ContractGenerationService, ContractGenerationUsage,
    ContractReviewAction, ContractReviewDecision, ContractSource, ContractSourceKind,
    ContractSourceSpan, DesignCoordinateSpace, DesignElementRegion, GeneratedContractDraft,
    GeneratedContractProvenance, ProductDecision, ProductDecisionStatus, ReviewedContractDraft,
};
pub use error::{AgentError, ContractGenerationError, GroundingError, LlmError};
pub use grounding::{
    GroundingAuthority, GroundingCandidate, GroundingCandidateGeometry, GroundingCoordinateSpace,
    GroundingImageCandidate, GroundingOptions, GroundingPageContext, GroundingPoint,
    GroundingProvenance, GroundingProviderIdentity, GroundingProviderRequest,
    GroundingProviderResponse, GroundingRequest, GroundingResult, GroundingSemanticMatch,
    GroundingTrigger, GroundingUsage, SemanticFallbackReason, VisualGroundingProvider,
    VisualGroundingService,
};
pub use model::{
    ActionHistory, AgentDecision, AgentGoal, AgentOptions, AgentRunResult, AgentStatus, AgentTurn,
    LlmIdentity, LlmUsage, PlannerContext, RemainingBudget,
};
pub use policy::{ActionKind, ActionPolicy, CapabilityPolicy, NavigationScope, PolicyContext};
pub use provider::{LlmImageAttachment, LlmProvider, StructuredLlmRequest, StructuredLlmResponse};
pub use redaction::{ProvenanceRedactor, REDACTED_VALUE};
pub use runtime::{AgentLoop, AGENT_PROMPT_VERSION};
