use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_test_core::ContractContext;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::merge::{detect_conflicts, merge_decisions};
use super::review::review;
use super::validation::{
    read_verified_sources, validate_context, validate_identifier, validate_identity,
    validate_options, validate_response, validate_sources,
};
use crate::{
    ContractGenerationError, ContractGenerationOptions, ContractGenerationProvider,
    ContractGenerationProviderIdentity, ContractGenerationProviderRequest,
    ContractGenerationProviderResponse, ContractGenerationReview, ContractSource,
    GeneratedContractDraft, ReviewedContractDraft,
};

pub struct ContractGenerationService {
    provider: Arc<dyn ContractGenerationProvider>,
    options: ContractGenerationOptions,
    identity: ContractGenerationProviderIdentity,
}

impl fmt::Debug for ContractGenerationService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContractGenerationService")
            .field("options", &self.options)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl ContractGenerationService {
    pub fn new(
        provider: Arc<dyn ContractGenerationProvider>,
        options: ContractGenerationOptions,
    ) -> Result<Self, ContractGenerationError> {
        validate_options(&options)?;
        let identity = provider.identity();
        validate_identity(&identity)?;
        Ok(Self {
            provider,
            options,
            identity,
        })
    }

    pub async fn generate(
        &self,
        contract_name: impl Into<String>,
        context: ContractContext,
        sources: Vec<ContractSource>,
        max_cost_microusd: u64,
        cancellation: CancellationToken,
    ) -> Result<GeneratedContractDraft, ContractGenerationError> {
        let contract_name = contract_name.into();
        validate_identifier(&contract_name, "contract name")?;
        validate_context(&context, self.options.max_string_bytes)?;
        validate_sources(&sources, &self.options)?;
        read_verified_sources(&sources, self.options.max_source_bytes).await?;

        let issued_at_unix_ms = unix_ms()?;
        let timeout_ms = u64::try_from(self.options.timeout.as_millis()).map_err(|_| {
            config_error("contract generation timeout cannot be represented in milliseconds")
        })?;
        let deadline_unix_ms = issued_at_unix_ms.checked_add(timeout_ms).ok_or_else(|| {
            ContractGenerationError::new(
                "test.agent.contract_generation.clock_invalid",
                "contract generation deadline overflowed the Unix millisecond clock",
                false,
            )
        })?;
        let deadline = Instant::now() + self.options.timeout;
        let request = ContractGenerationProviderRequest {
            contract_name: contract_name.clone(),
            context: context.clone(),
            sources: sources.clone(),
            issued_at_unix_ms,
            deadline_unix_ms,
            max_cost_microusd,
        };
        let response = self.call_provider(request, deadline, cancellation).await?;

        // The model call can be slow. Re-read and re-hash every source before
        // admitting any returned span so a concurrent edit cannot create stale
        // provenance or make synchronous file I/O leak into the async path.
        let verified_sources =
            read_verified_sources(&sources, self.options.max_source_bytes).await?;
        validate_response(
            &context,
            &sources,
            &verified_sources,
            max_cost_microusd,
            &response,
            &self.identity,
            &self.options,
        )?;

        let conflicts = detect_conflicts(&response.candidates);
        let unresolved_decisions = merge_decisions(&response.candidates)?;
        Ok(GeneratedContractDraft {
            name: contract_name,
            version: 1,
            context,
            provenance: response.source_digests,
            candidates: response.candidates,
            conflicts,
            unresolved_decisions,
            usage: response.usage,
            provider: response.identity,
            request_id: response.request_id,
        })
    }

    pub fn review(
        &self,
        draft: GeneratedContractDraft,
        review_request: ContractGenerationReview,
    ) -> Result<ReviewedContractDraft, ContractGenerationError> {
        review(draft, review_request, self.options.max_string_bytes)
    }

    async fn call_provider(
        &self,
        request: ContractGenerationProviderRequest,
        deadline: Instant,
        cancellation: CancellationToken,
    ) -> Result<ContractGenerationProviderResponse, ContractGenerationError> {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(ContractGenerationError::new(
                "test.agent.contract_generation.cancelled",
                "contract generation was cancelled before the provider completed",
                false,
            )),
            result = tokio::time::timeout_at(deadline, self.provider.generate(request)) => {
                match result {
                    Ok(result) => result,
                    Err(_) => Err(ContractGenerationError::new(
                        "test.agent.contract_generation.timeout",
                        "contract generation provider exceeded the configured deadline",
                        true,
                    )),
                }
            }
        }
    }
}

fn unix_ms() -> Result<u64, ContractGenerationError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        ContractGenerationError::new(
            "test.agent.contract_generation.clock_invalid",
            "system clock is earlier than the Unix epoch",
            false,
        )
    })?;
    u64::try_from(duration.as_millis()).map_err(|_| {
        ContractGenerationError::new(
            "test.agent.contract_generation.clock_invalid",
            "system clock cannot be represented in Unix milliseconds",
            false,
        )
    })
}

fn config_error(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.config_invalid",
        message,
        false,
    )
}
