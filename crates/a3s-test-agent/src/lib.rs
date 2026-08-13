//! Schema-constrained model execution and optional visual grounding for A3S Test.

mod error;
mod grounding;
mod model;
mod policy;
mod provider;
mod redaction;
mod runtime;

pub use error::{AgentError, GroundingError, LlmError};
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
