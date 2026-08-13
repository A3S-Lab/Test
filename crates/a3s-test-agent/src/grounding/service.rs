use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::reconcile::{reconcile_candidate, CandidateResolution};
use super::MAX_GROUNDING_IMAGE_BYTES;
use crate::{
    GroundingAuthority, GroundingCandidateGeometry, GroundingCoordinateSpace, GroundingError,
    GroundingOptions, GroundingPageContext, GroundingProvenance, GroundingProviderIdentity,
    GroundingProviderRequest, GroundingProviderResponse, GroundingRequest, GroundingResult,
    VisualGroundingProvider,
};

const MAX_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MIN_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_CANDIDATES: usize = 256;
const MAX_QUERY_BYTES: usize = 64 * 1_024;
const MAX_LABEL_BYTES: usize = 16 * 1_024;
const MAX_PATH_BYTES: usize = 16 * 1_024;
const MAX_IDENTITY_BYTES: usize = 1_024;
const MAX_REQUEST_ID_BYTES: usize = 4 * 1_024;
const MAX_SCREENSHOT_DIMENSION: u32 = 32_768;
const MAX_PAGE_CONTEXT_NODES: usize = 5_000;

pub struct VisualGroundingService {
    provider: Arc<dyn VisualGroundingProvider>,
    options: GroundingOptions,
    identity: GroundingProviderIdentity,
}

impl fmt::Debug for VisualGroundingService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisualGroundingService")
            .field("options", &self.options)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl VisualGroundingService {
    pub fn new(
        provider: Arc<dyn VisualGroundingProvider>,
        options: GroundingOptions,
    ) -> Result<Self, GroundingError> {
        validate_options(&options)?;
        let identity = provider.identity();
        validate_identity(&identity, "configured provider identity")?;
        Ok(Self {
            provider,
            options,
            identity,
        })
    }

    pub fn validate_request(&self, request: &GroundingRequest) -> Result<(), GroundingError> {
        if request.screenshot_path.trim().is_empty()
            || request.screenshot_path.len() > MAX_PATH_BYTES
        {
            return Err(request_error(format!(
                "screenshot path must contain 1 to {MAX_PATH_BYTES} bytes"
            )));
        }
        if !valid_sha256(&request.screenshot_sha256) {
            return Err(request_error(
                "screenshot digest must use sha256:<64 lowercase hexadecimal characters>",
            ));
        }
        if request.width == 0
            || request.height == 0
            || request.width > MAX_SCREENSHOT_DIMENSION
            || request.height > MAX_SCREENSHOT_DIMENSION
        {
            return Err(request_error(format!(
                "screenshot dimensions must be between 1 and {MAX_SCREENSHOT_DIMENSION} pixels"
            )));
        }
        if request.query.trim().is_empty() || request.query.len() > self.options.max_query_bytes {
            return Err(request_error(format!(
                "grounding query must contain 1 to {} bytes",
                self.options.max_query_bytes
            )));
        }
        if request.observation_id == 0 {
            return Err(request_error(
                "grounding requires a positive current observation ID",
            ));
        }
        Ok(())
    }

    pub async fn ground(
        &self,
        request: GroundingRequest,
        page_context: Option<GroundingPageContext<'_>>,
        cancellation: CancellationToken,
    ) -> Result<GroundingResult, GroundingError> {
        self.validate_request(&request)?;
        let actual_screenshot_sha256 = hash_screenshot(&request.screenshot_path).await?;
        if actual_screenshot_sha256 != request.screenshot_sha256 {
            return Err(GroundingError::new(
                "test.agent.grounding.screenshot_mismatch",
                "screenshot bytes do not match the admitted SHA-256 digest",
                false,
            ));
        }
        if page_context.is_some_and(|context| context.observation_id != request.observation_id) {
            return Err(GroundingError::new(
                "test.agent.grounding.context_mismatch",
                "page context does not belong to the grounding observation ID",
                false,
            ));
        }
        if page_context.is_some_and(|context| {
            context.snapshot.truncated
                || context.snapshot.next_cursor.is_some()
                || context.snapshot.nodes.len() > MAX_PAGE_CONTEXT_NODES
                || context
                    .snapshot
                    .revision
                    .is_none_or(|revision| revision != context.surface_revision)
        }) {
            return Err(GroundingError::new(
                "test.agent.grounding.context_incomplete",
                "page context must be complete, bounded, and revision-bound to the grounding observation",
                false,
            ));
        }
        let issued_at_unix_ms = unix_ms()?;
        let timeout_ms = u64::try_from(self.options.timeout.as_millis())
            .map_err(|_| config_error("grounding timeout cannot be represented in milliseconds"))?;
        let deadline_unix_ms = issued_at_unix_ms.checked_add(timeout_ms).ok_or_else(|| {
            GroundingError::new(
                "test.agent.grounding.clock_invalid",
                "grounding deadline overflowed the Unix millisecond clock",
                false,
            )
        })?;
        let deadline = Instant::now() + self.options.timeout;
        let provider_request = GroundingProviderRequest {
            screenshot_path: request.screenshot_path.clone(),
            screenshot_sha256: request.screenshot_sha256.clone(),
            width: request.width,
            height: request.height,
            query: request.query.clone(),
            observation_id: request.observation_id,
            trigger: request.trigger,
            issued_at_unix_ms,
            deadline_unix_ms,
            max_cost_microusd: request.max_cost_microusd,
        };
        let response = tokio::select! {
            biased;
            () = cancellation.cancelled() => {
                return Err(GroundingError::new(
                    "test.agent.grounding.cancelled",
                    "visual grounding was cancelled before the provider completed",
                    false,
                ));
            }
            result = tokio::time::timeout_at(deadline, self.provider.locate(provider_request)) => {
                match result {
                    Ok(result) => result?,
                    Err(_) => {
                        return Err(GroundingError::new(
                            "test.agent.grounding.timeout",
                            "visual grounding provider exceeded the configured deadline",
                            true,
                        ));
                    }
                }
            }
        };
        self.validate_response(&request, &response)?;
        Ok(self.reconcile(
            &request,
            page_context.map(|context| context.snapshot),
            response,
        ))
    }

    fn validate_response(
        &self,
        request: &GroundingRequest,
        response: &GroundingProviderResponse,
    ) -> Result<(), GroundingError> {
        validate_identity(&response.identity, "returned provider identity")?;
        if response.identity != self.identity
            || response.observation_id != request.observation_id
            || response.screenshot_sha256 != request.screenshot_sha256
            || response.width != request.width
            || response.height != request.height
        {
            return Err(GroundingError::new(
                "test.agent.grounding.response_mismatch",
                "provider response does not match its admitted identity, observation, digest, or dimensions",
                false,
            ));
        }
        if response.candidates.len() > self.options.max_candidates {
            return Err(response_error(format!(
                "provider returned {} candidates, exceeding the {} candidate limit",
                response.candidates.len(),
                self.options.max_candidates
            )));
        }
        if response.usage.cost_microusd > request.max_cost_microusd {
            return Err(GroundingError::new(
                "test.agent.grounding.cost_budget_exceeded",
                "visual grounding provider reported cost above the admitted request budget",
                false,
            ));
        }
        if response
            .request_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > MAX_REQUEST_ID_BYTES)
        {
            return Err(response_error(format!(
                "provider request ID must contain 1 to {MAX_REQUEST_ID_BYTES} bytes when present"
            )));
        }
        for candidate in &response.candidates {
            if !candidate.confidence.is_finite() || !(0.0..=1.0).contains(&candidate.confidence) {
                return Err(response_error(
                    "candidate confidence must be finite and between 0 and 1",
                ));
            }
            if candidate.label.as_ref().is_some_and(|value| {
                value.trim().is_empty() || value.len() > self.options.max_label_bytes
            }) {
                return Err(response_error(format!(
                    "candidate labels must contain 1 to {} bytes when present",
                    self.options.max_label_bytes
                )));
            }
            validate_geometry(
                candidate.geometry,
                response.coordinate_space,
                response.width,
                response.height,
            )?;
        }
        Ok(())
    }

    fn reconcile(
        &self,
        request: &GroundingRequest,
        page_context: Option<&a3s_test_core::PageContextSnapshot>,
        response: GroundingProviderResponse,
    ) -> GroundingResult {
        let provenance = GroundingProvenance {
            identity: response.identity,
            observation_id: response.observation_id,
            screenshot_sha256: response.screenshot_sha256,
            width: response.width,
            height: response.height,
            provider_coordinate_space: response.coordinate_space,
            usage: response.usage,
            request_id: response.request_id,
            authority: GroundingAuthority::Advisory,
        };
        let mut matches = Vec::new();
        let mut image_bound_candidates = Vec::new();
        for (index, candidate) in response.candidates.iter().enumerate() {
            match reconcile_candidate(
                index,
                candidate,
                provenance.provider_coordinate_space,
                request.width,
                request.height,
                page_context,
            ) {
                CandidateResolution::Semantic(value) => matches.push(value),
                CandidateResolution::ImageBound(value) => image_bound_candidates.push(value),
            }
        }
        if matches.is_empty() {
            GroundingResult::ImageBound {
                provenance,
                candidates: image_bound_candidates,
            }
        } else {
            GroundingResult::Semantic {
                provenance,
                matches,
                image_bound_candidates,
            }
        }
    }
}

fn validate_options(options: &GroundingOptions) -> Result<(), GroundingError> {
    if options.timeout < MIN_TIMEOUT || options.timeout > MAX_TIMEOUT {
        return Err(config_error(format!(
            "grounding timeout must be between {} millisecond and {} seconds",
            MIN_TIMEOUT.as_millis(),
            MAX_TIMEOUT.as_secs()
        )));
    }
    if options.max_candidates == 0 || options.max_candidates > MAX_CANDIDATES {
        return Err(config_error(format!(
            "candidate limit must be between 1 and {MAX_CANDIDATES}"
        )));
    }
    if options.max_query_bytes == 0 || options.max_query_bytes > MAX_QUERY_BYTES {
        return Err(config_error(format!(
            "query byte limit must be between 1 and {MAX_QUERY_BYTES}"
        )));
    }
    if options.max_label_bytes == 0 || options.max_label_bytes > MAX_LABEL_BYTES {
        return Err(config_error(format!(
            "label byte limit must be between 1 and {MAX_LABEL_BYTES}"
        )));
    }
    Ok(())
}

fn validate_identity(
    identity: &GroundingProviderIdentity,
    description: &str,
) -> Result<(), GroundingError> {
    if identity.provider.trim().is_empty()
        || identity.model.trim().is_empty()
        || identity.provider.len() > MAX_IDENTITY_BYTES
        || identity.model.len() > MAX_IDENTITY_BYTES
    {
        return Err(GroundingError::new(
            "test.agent.grounding.identity_invalid",
            format!(
                "{description} requires non-empty provider and model names no longer than {MAX_IDENTITY_BYTES} bytes"
            ),
            false,
        ));
    }
    Ok(())
}

fn validate_geometry(
    geometry: GroundingCandidateGeometry,
    coordinate_space: GroundingCoordinateSpace,
    screenshot_width: u32,
    screenshot_height: u32,
) -> Result<(), GroundingError> {
    let (limit_x, limit_y) = match coordinate_space {
        GroundingCoordinateSpace::ScreenshotPixels => {
            (f64::from(screenshot_width), f64::from(screenshot_height))
        }
        GroundingCoordinateSpace::Normalized => (1.0, 1.0),
    };
    let valid = match geometry {
        GroundingCandidateGeometry::Point { x, y } => {
            finite([x, y]) && x >= 0.0 && y >= 0.0 && x < limit_x && y < limit_y
        }
        GroundingCandidateGeometry::Box {
            x,
            y,
            width,
            height,
        } => {
            finite([x, y, width, height])
                && x >= 0.0
                && y >= 0.0
                && width > 0.0
                && height > 0.0
                && x + width <= limit_x
                && y + height <= limit_y
        }
    };
    if valid {
        Ok(())
    } else {
        Err(response_error(
            "candidate geometry must be finite, positive-sized, and inside the declared coordinate space",
        ))
    }
}

fn finite<const N: usize>(values: [f64; N]) -> bool {
    values.into_iter().all(f64::is_finite)
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn unix_ms() -> Result<u64, GroundingError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        GroundingError::new(
            "test.agent.grounding.clock_invalid",
            "system clock is earlier than the Unix epoch",
            false,
        )
    })?;
    u64::try_from(duration.as_millis()).map_err(|_| {
        GroundingError::new(
            "test.agent.grounding.clock_invalid",
            "system clock cannot be represented in Unix milliseconds",
            false,
        )
    })
}

async fn hash_screenshot(path: &str) -> Result<String, GroundingError> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        GroundingError::new(
            "test.agent.grounding.screenshot_invalid",
            format!("failed to inspect grounding screenshot: {error}"),
            false,
        )
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_GROUNDING_IMAGE_BYTES
    {
        return Err(GroundingError::new(
            "test.agent.grounding.screenshot_invalid",
            format!(
                "grounding screenshot must be a regular non-symbolic-link file containing 1 to {MAX_GROUNDING_IMAGE_BYTES} bytes"
            ),
            false,
        ));
    }
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        GroundingError::new(
            "test.agent.grounding.screenshot_invalid",
            format!("failed to open grounding screenshot: {error}"),
            false,
        )
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_GROUNDING_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            GroundingError::new(
                "test.agent.grounding.screenshot_invalid",
                format!("failed to read grounding screenshot: {error}"),
                false,
            )
        })?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_GROUNDING_IMAGE_BYTES {
        return Err(GroundingError::new(
            "test.agent.grounding.screenshot_invalid",
            format!("grounding screenshot must contain 1 to {MAX_GROUNDING_IMAGE_BYTES} bytes"),
            false,
        ));
    }
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn config_error(message: impl Into<String>) -> GroundingError {
    GroundingError::new("test.agent.grounding.config_invalid", message, false)
}

fn request_error(message: impl Into<String>) -> GroundingError {
    GroundingError::new("test.agent.grounding.request_invalid", message, false)
}

fn response_error(message: impl Into<String>) -> GroundingError {
    GroundingError::new("test.agent.grounding.response_invalid", message, false)
}
