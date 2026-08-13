mod merge;
mod model;
mod provider;
mod review;
mod service;
mod validation;
mod workflow;

pub use model::{
    ContractCandidate, ContractCandidateElement, ContractCandidateVariant, ContractConflict,
    ContractConflictResolution, ContractConflictStatus, ContractGenerationOptions,
    ContractGenerationProviderIdentity, ContractGenerationProviderRequest,
    ContractGenerationProviderResponse, ContractGenerationReview, ContractGenerationUsage,
    ContractReviewAction, ContractReviewDecision, ContractSource, ContractSourceKind,
    ContractSourceSpan, DesignCoordinateSpace, DesignElementRegion, GeneratedContractDraft,
    GeneratedContractProvenance, ProductDecision, ProductDecisionStatus, ReviewedContractDraft,
};
pub use provider::ContractGenerationProvider;
pub use service::ContractGenerationService;
pub use workflow::{
    ContractWorkflowAdmission, ContractWorkflowArtifact, ContractWorkflowStage,
    CONTRACT_WORKFLOW_PROTOCOL,
};
