use async_trait::async_trait;

use crate::{
    DesignAuditError, DesignAuditProviderIdentity, DesignAuditProviderRequest,
    DesignAuditProviderResponse,
};

/// Reviews one already verified screenshot and its exact Test Kit context.
///
/// Implementations own model transport, credentials, runtime, and licensing.
/// They must honor the supplied deadline and cost ceiling. A3S Test admits all
/// returned provenance and targets locally; provider findings remain advice
/// until a human explicitly promotes them into the repair flow.
#[async_trait]
pub trait DesignAuditProvider: Send + Sync {
    fn identity(&self) -> DesignAuditProviderIdentity;

    async fn audit(
        &self,
        request: DesignAuditProviderRequest,
    ) -> Result<DesignAuditProviderResponse, DesignAuditError>;
}
