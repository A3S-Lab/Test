use async_trait::async_trait;

use crate::{
    ContractGenerationError, ContractGenerationProviderIdentity, ContractGenerationProviderRequest,
    ContractGenerationProviderResponse,
};

#[async_trait]
pub trait ContractGenerationProvider: Send + Sync {
    fn identity(&self) -> ContractGenerationProviderIdentity;

    async fn generate(
        &self,
        request: ContractGenerationProviderRequest,
    ) -> Result<ContractGenerationProviderResponse, ContractGenerationError>;
}
