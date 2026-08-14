use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use a3s_test_core::{
    DesignAuditAuthority, DesignAuditDimension, DesignAuditFinding, DesignAuditNormalizedRegion,
    DesignAuditProvenance, DesignAuditProviderIdentity, DesignAuditReport, DesignAuditTarget,
    PageContextGeometry, PageContextSnapshot, DESIGN_AUDIT_REPORT_PROTOCOL, PAGE_CONTEXT_PROTOCOL,
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::MAX_DESIGN_AUDIT_IMAGE_BYTES;
use crate::{
    DesignAuditError, DesignAuditOptions, DesignAuditProvider, DesignAuditProviderRequest,
    DesignAuditProviderResponse, DesignAuditRequest,
};

const MAX_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MIN_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_FINDINGS: usize = 500;
const MAX_TEXT_BYTES: usize = 64 * 1_024;
const MAX_PAGE_CONTEXT_BYTES: usize = 32 * 1_024 * 1_024;
const MAX_PATH_BYTES: usize = 16 * 1_024;
const MAX_IDENTITY_BYTES: usize = 1_024;
const MAX_REQUEST_ID_BYTES: usize = 4 * 1_024;
const MAX_SCREENSHOT_DIMENSION: u32 = 32_768;
const MAX_PAGE_CONTEXT_NODES: usize = 5_000;
const MAX_PAGE_CONTEXT_COMPONENTS: usize = 1_000;
const MAX_IDENTIFIER_BYTES: usize = 1_024;

pub struct DesignAuditService {
    provider: Arc<dyn DesignAuditProvider>,
    options: DesignAuditOptions,
    identity: DesignAuditProviderIdentity,
}

impl fmt::Debug for DesignAuditService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesignAuditService")
            .field("options", &self.options)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl DesignAuditService {
    pub fn new(
        provider: Arc<dyn DesignAuditProvider>,
        options: DesignAuditOptions,
    ) -> Result<Self, DesignAuditError> {
        validate_options(&options)?;
        let identity = provider.identity();
        validate_identity(&identity, "configured provider identity")?;
        Ok(Self {
            provider,
            options,
            identity,
        })
    }

    pub fn validate_request(&self, request: &DesignAuditRequest) -> Result<(), DesignAuditError> {
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
        if request.observation_id == 0 || request.surface_revision == 0 {
            return Err(request_error(
                "design audit requires positive observation and surface revision identifiers",
            ));
        }
        validate_dimensions(&request.dimensions)?;
        validate_page_context(
            &request.page_context,
            request.surface_revision,
            self.options.max_page_context_bytes,
        )?;
        Ok(())
    }

    pub async fn audit(
        &self,
        request: DesignAuditRequest,
        cancellation: CancellationToken,
    ) -> Result<DesignAuditReport, DesignAuditError> {
        self.validate_request(&request)?;
        let actual_screenshot_sha256 = hash_screenshot(&request.screenshot_path).await?;
        if actual_screenshot_sha256 != request.screenshot_sha256 {
            return Err(DesignAuditError::new(
                "test.agent.design_audit.screenshot_mismatch",
                "screenshot bytes do not match the admitted SHA-256 digest",
                false,
            ));
        }
        let page_context_bytes = serde_json::to_vec(&request.page_context).map_err(|error| {
            request_error(format!("failed to encode admitted page context: {error}"))
        })?;
        let page_context_sha256 = format!("sha256:{:x}", Sha256::digest(page_context_bytes));
        let issued_at_unix_ms = unix_ms()?;
        let timeout_ms = u64::try_from(self.options.timeout.as_millis()).map_err(|_| {
            config_error("design-audit timeout cannot be represented in milliseconds")
        })?;
        let deadline_unix_ms = issued_at_unix_ms.checked_add(timeout_ms).ok_or_else(|| {
            DesignAuditError::new(
                "test.agent.design_audit.clock_invalid",
                "design-audit deadline overflowed the Unix millisecond clock",
                false,
            )
        })?;
        let deadline = Instant::now() + self.options.timeout;
        let provider_request = DesignAuditProviderRequest {
            screenshot_path: request.screenshot_path.clone(),
            screenshot_sha256: request.screenshot_sha256.clone(),
            page_context_sha256: page_context_sha256.clone(),
            width: request.width,
            height: request.height,
            observation_id: request.observation_id,
            surface_revision: request.surface_revision,
            page_context: request.page_context.clone(),
            dimensions: request.dimensions.clone(),
            issued_at_unix_ms,
            deadline_unix_ms,
            max_cost_microusd: request.max_cost_microusd,
        };
        let response = tokio::select! {
            biased;
            () = cancellation.cancelled() => {
                return Err(DesignAuditError::new(
                    "test.agent.design_audit.cancelled",
                    "design audit was cancelled before the provider completed",
                    false,
                ));
            }
            result = tokio::time::timeout_at(deadline, self.provider.audit(provider_request)) => {
                match result {
                    Ok(result) => result?,
                    Err(_) => {
                        return Err(DesignAuditError::new(
                            "test.agent.design_audit.timeout",
                            "design-audit provider exceeded the configured deadline",
                            true,
                        ));
                    }
                }
            }
        };
        self.validate_response(&request, &page_context_sha256, &response)?;
        Ok(DesignAuditReport {
            protocol: DESIGN_AUDIT_REPORT_PROTOCOL.to_string(),
            provenance: DesignAuditProvenance {
                identity: response.identity,
                observation_id: response.observation_id,
                surface_revision: response.surface_revision,
                screenshot_sha256: response.screenshot_sha256,
                page_context_sha256: response.page_context_sha256,
                width: response.width,
                height: response.height,
                usage: response.usage,
                request_id: response.request_id,
                authority: DesignAuditAuthority::Advisory,
            },
            dimensions: response.dimensions,
            findings: response.findings,
        })
    }

    fn validate_response(
        &self,
        request: &DesignAuditRequest,
        page_context_sha256: &str,
        response: &DesignAuditProviderResponse,
    ) -> Result<(), DesignAuditError> {
        validate_identity(&response.identity, "returned provider identity")?;
        if response.identity != self.identity
            || response.observation_id != request.observation_id
            || response.surface_revision != request.surface_revision
            || response.screenshot_sha256 != request.screenshot_sha256
            || response.page_context_sha256 != page_context_sha256
            || response.width != request.width
            || response.height != request.height
            || response.dimensions != request.dimensions
        {
            return Err(DesignAuditError::new(
                "test.agent.design_audit.response_mismatch",
                "provider response does not match its admitted identity, observation, revision, digests, dimensions, or audit scope",
                false,
            ));
        }
        if response.findings.len() > self.options.max_findings {
            return Err(response_error(format!(
                "provider returned {} findings, exceeding the {} finding limit",
                response.findings.len(),
                self.options.max_findings
            )));
        }
        if response.usage.cost_microusd > request.max_cost_microusd {
            return Err(DesignAuditError::new(
                "test.agent.design_audit.cost_budget_exceeded",
                "design-audit provider reported cost above the admitted request budget",
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
        let mut finding_ids = HashSet::with_capacity(response.findings.len());
        let requested_dimensions = request.dimensions.iter().copied().collect::<HashSet<_>>();
        for finding in &response.findings {
            validate_finding(
                finding,
                &request.page_context,
                &requested_dimensions,
                &self.options,
            )?;
            if !finding_ids.insert(finding.id.as_str()) {
                return Err(response_error(
                    "finding IDs must be unique within one response",
                ));
            }
        }
        Ok(())
    }
}

fn validate_options(options: &DesignAuditOptions) -> Result<(), DesignAuditError> {
    if options.timeout < MIN_TIMEOUT || options.timeout > MAX_TIMEOUT {
        return Err(config_error(format!(
            "design-audit timeout must be between {} millisecond and {} seconds",
            MIN_TIMEOUT.as_millis(),
            MAX_TIMEOUT.as_secs()
        )));
    }
    if options.max_findings == 0 || options.max_findings > MAX_FINDINGS {
        return Err(config_error(format!(
            "finding limit must be between 1 and {MAX_FINDINGS}"
        )));
    }
    for (name, value) in [
        ("summary", options.max_summary_bytes),
        ("rationale", options.max_rationale_bytes),
        ("recommendation", options.max_recommendation_bytes),
    ] {
        if value == 0 || value > MAX_TEXT_BYTES {
            return Err(config_error(format!(
                "{name} byte limit must be between 1 and {MAX_TEXT_BYTES}"
            )));
        }
    }
    if options.max_page_context_bytes == 0
        || options.max_page_context_bytes > MAX_PAGE_CONTEXT_BYTES
    {
        return Err(config_error(format!(
            "page-context byte limit must be between 1 and {MAX_PAGE_CONTEXT_BYTES}"
        )));
    }
    Ok(())
}

fn validate_dimensions(dimensions: &[DesignAuditDimension]) -> Result<(), DesignAuditError> {
    if dimensions.is_empty() || dimensions.len() > DesignAuditDimension::ALL.len() {
        return Err(request_error(format!(
            "design audit requires between 1 and {} dimensions",
            DesignAuditDimension::ALL.len()
        )));
    }
    let unique = dimensions.iter().collect::<HashSet<_>>();
    if unique.len() != dimensions.len() {
        return Err(request_error("design-audit dimensions must be unique"));
    }
    Ok(())
}

fn validate_page_context(
    context: &PageContextSnapshot,
    surface_revision: u64,
    max_bytes: usize,
) -> Result<(), DesignAuditError> {
    if context.protocol.as_deref() != Some(PAGE_CONTEXT_PROTOCOL)
        || context
            .sdk_version
            .as_ref()
            .is_none_or(|value| value.trim().is_empty() || value.len() > MAX_IDENTIFIER_BYTES)
        || context.revision != Some(surface_revision)
        || context.page.as_ref().is_none_or(|page| !page.ready)
        || context.truncated
        || context.next_cursor.is_some()
        || !context.removed_node_ids.is_empty()
    {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.context_incomplete",
            "page context must be complete, ready, protocol-bound, and revision-bound to the design audit",
            false,
        ));
    }
    if context.nodes.len() > MAX_PAGE_CONTEXT_NODES
        || context.components.len() > MAX_PAGE_CONTEXT_COMPONENTS
    {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.context_unbounded",
            "page context exceeds the admitted node or component limit",
            false,
        ));
    }
    let mut node_ids = HashSet::with_capacity(context.nodes.len());
    for node in &context.nodes {
        if node.id.trim().is_empty()
            || node.id.len() > MAX_IDENTIFIER_BYTES
            || !node_ids.insert(node.id.as_str())
        {
            return Err(DesignAuditError::new(
                "test.agent.design_audit.context_invalid",
                "page-context node IDs must be non-empty, bounded, and unique",
                false,
            ));
        }
    }
    let encoded = serde_json::to_vec(context)
        .map_err(|error| request_error(format!("failed to encode page context: {error}")))?;
    if encoded.len() > max_bytes {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.context_unbounded",
            format!("page context exceeds the admitted {max_bytes} byte limit"),
            false,
        ));
    }
    Ok(())
}

fn validate_finding(
    finding: &DesignAuditFinding,
    context: &PageContextSnapshot,
    requested_dimensions: &HashSet<DesignAuditDimension>,
    options: &DesignAuditOptions,
) -> Result<(), DesignAuditError> {
    if finding.id.trim().is_empty() || finding.id.len() > MAX_IDENTIFIER_BYTES {
        return Err(response_error(format!(
            "finding IDs must contain 1 to {MAX_IDENTIFIER_BYTES} bytes"
        )));
    }
    if !requested_dimensions.contains(&finding.dimension) {
        return Err(response_error(
            "finding dimension was not included in the admitted audit request",
        ));
    }
    for (name, value, limit) in [
        (
            "summary",
            finding.summary.as_str(),
            options.max_summary_bytes,
        ),
        (
            "rationale",
            finding.rationale.as_str(),
            options.max_rationale_bytes,
        ),
        (
            "recommendation",
            finding.recommendation.as_str(),
            options.max_recommendation_bytes,
        ),
    ] {
        if value.trim().is_empty() || value.len() > limit {
            return Err(response_error(format!(
                "finding {name} must contain 1 to {limit} bytes"
            )));
        }
    }
    if finding.confidence > 100 {
        return Err(response_error(
            "finding confidence must be between 0 and 100",
        ));
    }
    match &finding.target {
        DesignAuditTarget::Page => Ok(()),
        DesignAuditTarget::Node { node_id } => {
            let node = context
                .nodes
                .iter()
                .find(|candidate| candidate.id == *node_id)
                .ok_or_else(|| {
                    response_error(
                        "finding node target is not present in the admitted page context",
                    )
                })?;
            if !node.state.visible
                || node
                    .geometry
                    .as_ref()
                    .is_none_or(|geometry| !valid_node_geometry(geometry))
            {
                return Err(response_error(
                    "finding node target must have finite visible geometry in the admitted page context",
                ));
            }
            Ok(())
        }
        DesignAuditTarget::Region { region } => {
            if valid_normalized_region(*region) {
                Ok(())
            } else {
                Err(response_error(
                    "finding region must be finite, positive-sized, and inside normalized screenshot bounds",
                ))
            }
        }
    }
}

fn valid_node_geometry(geometry: &PageContextGeometry) -> bool {
    let rect = &geometry.normalized;
    [
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        geometry.visible_ratio,
    ]
    .into_iter()
    .all(f64::is_finite)
        && rect.width > 0.0
        && rect.height > 0.0
        && (0.0..=1.0).contains(&geometry.visible_ratio)
        && geometry.visible_ratio > 0.0
}

fn valid_normalized_region(region: DesignAuditNormalizedRegion) -> bool {
    [region.x, region.y, region.width, region.height]
        .into_iter()
        .all(f64::is_finite)
        && region.x >= 0.0
        && region.y >= 0.0
        && region.width > 0.0
        && region.height > 0.0
        && region.x + region.width <= 1.0
        && region.y + region.height <= 1.0
}

fn validate_identity(
    identity: &DesignAuditProviderIdentity,
    description: &str,
) -> Result<(), DesignAuditError> {
    if identity.provider.trim().is_empty()
        || identity.model.trim().is_empty()
        || identity.provider.len() > MAX_IDENTITY_BYTES
        || identity.model.len() > MAX_IDENTITY_BYTES
    {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.identity_invalid",
            format!(
                "{description} requires non-empty provider and model names no longer than {MAX_IDENTITY_BYTES} bytes"
            ),
            false,
        ));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn unix_ms() -> Result<u64, DesignAuditError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        DesignAuditError::new(
            "test.agent.design_audit.clock_invalid",
            "system clock is earlier than the Unix epoch",
            false,
        )
    })?;
    u64::try_from(duration.as_millis()).map_err(|_| {
        DesignAuditError::new(
            "test.agent.design_audit.clock_invalid",
            "system clock cannot be represented in Unix milliseconds",
            false,
        )
    })
}

async fn hash_screenshot(path: &str) -> Result<String, DesignAuditError> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        DesignAuditError::new(
            "test.agent.design_audit.screenshot_invalid",
            format!("failed to inspect design-audit screenshot: {error}"),
            false,
        )
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_DESIGN_AUDIT_IMAGE_BYTES
    {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.screenshot_invalid",
            format!(
                "design-audit screenshot must be a regular non-symbolic-link file containing 1 to {MAX_DESIGN_AUDIT_IMAGE_BYTES} bytes"
            ),
            false,
        ));
    }
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        DesignAuditError::new(
            "test.agent.design_audit.screenshot_invalid",
            format!("failed to open design-audit screenshot: {error}"),
            false,
        )
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_DESIGN_AUDIT_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            DesignAuditError::new(
                "test.agent.design_audit.screenshot_invalid",
                format!("failed to read design-audit screenshot: {error}"),
                false,
            )
        })?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_DESIGN_AUDIT_IMAGE_BYTES {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.screenshot_invalid",
            format!(
                "design-audit screenshot must contain 1 to {MAX_DESIGN_AUDIT_IMAGE_BYTES} bytes"
            ),
            false,
        ));
    }
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn config_error(message: impl Into<String>) -> DesignAuditError {
    DesignAuditError::new("test.agent.design_audit.config_invalid", message, false)
}

fn request_error(message: impl Into<String>) -> DesignAuditError {
    DesignAuditError::new("test.agent.design_audit.request_invalid", message, false)
}

fn response_error(message: impl Into<String>) -> DesignAuditError {
    DesignAuditError::new("test.agent.design_audit.response_invalid", message, false)
}
