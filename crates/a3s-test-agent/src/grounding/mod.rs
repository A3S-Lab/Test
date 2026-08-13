mod model;
mod provider;
mod reconcile;
mod service;

pub(crate) const MAX_GROUNDING_IMAGE_BYTES: u64 = 32 * 1_024 * 1_024;

pub use model::{
    GroundingAuthority, GroundingCandidate, GroundingCandidateGeometry, GroundingCoordinateSpace,
    GroundingImageAttachment, GroundingImageCandidate, GroundingOptions, GroundingPageContext,
    GroundingPoint, GroundingProvenance, GroundingProviderIdentity, GroundingProviderRequest,
    GroundingProviderResponse, GroundingRequest, GroundingResult, GroundingSemanticMatch,
    GroundingTrigger, GroundingUsage, SemanticFallbackReason,
};
pub use provider::VisualGroundingProvider;
pub use service::VisualGroundingService;
