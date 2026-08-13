use async_trait::async_trait;

use crate::{
    GroundingError, GroundingProviderIdentity, GroundingProviderRequest, GroundingProviderResponse,
};

/// Locates query-relevant points or boxes in one already verified screenshot.
///
/// Implementations own model transport, credentials, runtime, and licensing.
/// They must honor the supplied deadline and cost ceiling. A3S Test validates
/// the returned identity, image binding, bounds, and usage independently.
#[async_trait]
pub trait VisualGroundingProvider: Send + Sync {
    fn identity(&self) -> GroundingProviderIdentity;

    async fn locate(
        &self,
        request: GroundingProviderRequest,
    ) -> Result<GroundingProviderResponse, GroundingError>;
}
