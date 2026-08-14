use std::fmt;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use reqwest::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{redirect, Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

mod config;
mod wire;

pub use config::{HttpProviderConfig, HttpProviderConfigError, HttpProviderEndpoint};
pub use wire::{
    HttpContractGenerationRequest, HttpContractGenerationResponse, HttpDesignAuditRequest,
    HttpDesignAuditResponse, HttpLlmCompletionRequest, HttpLlmCompletionResponse,
    HttpProviderErrorResponse, HttpVisualGroundingRequest, HttpVisualGroundingResponse,
};
use wire::{HttpProviderRemoteError, HttpProviderRequestEnvelope, HttpProviderResponseEnvelope};

use crate::design_audit::MAX_DESIGN_AUDIT_IMAGE_BYTES;
use crate::grounding::MAX_GROUNDING_IMAGE_BYTES;
use crate::{
    ContractGenerationError, ContractGenerationProvider, ContractGenerationProviderIdentity,
    ContractGenerationProviderRequest, ContractGenerationProviderResponse, DesignAuditError,
    DesignAuditImageAttachment, DesignAuditProvider, DesignAuditProviderIdentity,
    DesignAuditProviderRequest, DesignAuditProviderResponse, GroundingError,
    GroundingImageAttachment, GroundingProviderIdentity, GroundingProviderRequest,
    GroundingProviderResponse, LlmError, LlmIdentity, LlmProvider, StructuredLlmRequest,
    StructuredLlmResponse, VisualGroundingProvider, CONTRACT_GENERATION_PROVIDER_PROTOCOL,
    DESIGN_AUDIT_PROVIDER_PROTOCOL, LLM_PROVIDER_PROTOCOL, VISUAL_GROUNDING_PROVIDER_PROTOCOL,
};

const MAX_ERROR_MESSAGE_CHARACTERS: usize = 64 * 1_024;
const JSON_MEDIA_TYPE: &str = "application/json";

#[derive(Clone, Debug)]
struct HttpProviderTransport {
    client: Client,
    config: HttpProviderConfig,
}

impl HttpProviderTransport {
    fn new(config: HttpProviderConfig) -> Result<Self, HttpProviderConfigError> {
        config.validate()?;
        let client = Client::builder()
            .no_proxy()
            .redirect(redirect::Policy::none())
            .connect_timeout(config.timeout)
            .timeout(config.timeout)
            .user_agent(concat!("a3s-test-agent/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| {
                HttpProviderConfigError::new(format!(
                    "failed to construct the provider HTTP client: {error}"
                ))
            })?;
        Ok(Self { client, config })
    }

    async fn exchange<Request, Response>(
        &self,
        protocol: &'static str,
        request: &Request,
        issued_at_unix_ms: u64,
        deadline_unix_ms: u64,
    ) -> Result<Response, HttpExchangeError>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let timeout =
            admitted_request_timeout(issued_at_unix_ms, deadline_unix_ms, self.config.timeout)?;
        self.exchange_with_timeout(protocol, request, timeout).await
    }

    async fn exchange_without_wire_deadline<Request, Response>(
        &self,
        protocol: &'static str,
        request: &Request,
    ) -> Result<Response, HttpExchangeError>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        self.exchange_with_timeout(protocol, request, self.config.timeout)
            .await
    }

    async fn exchange_visual_grounding<Response>(
        &self,
        protocol: &'static str,
        request: &GroundingProviderRequest,
        image: &GroundingImageAttachment,
        issued_at_unix_ms: u64,
        deadline_unix_ms: u64,
    ) -> Result<Response, HttpExchangeError>
    where
        Response: DeserializeOwned,
    {
        let timeout =
            admitted_request_timeout(issued_at_unix_ms, deadline_unix_ms, self.config.timeout)?;
        let envelope = HttpVisualGroundingRequest {
            protocol: protocol.to_string(),
            request: request.clone(),
            image: image.clone(),
        };
        self.exchange_envelope(protocol, &envelope, timeout).await
    }

    async fn exchange_design_audit<Response>(
        &self,
        protocol: &'static str,
        request: &DesignAuditProviderRequest,
        image: &DesignAuditImageAttachment,
        issued_at_unix_ms: u64,
        deadline_unix_ms: u64,
    ) -> Result<Response, HttpExchangeError>
    where
        Response: DeserializeOwned,
    {
        let timeout =
            admitted_request_timeout(issued_at_unix_ms, deadline_unix_ms, self.config.timeout)?;
        let envelope = HttpDesignAuditRequest {
            protocol: protocol.to_string(),
            request: request.clone(),
            image: image.clone(),
        };
        self.exchange_envelope(protocol, &envelope, timeout).await
    }

    async fn exchange_with_timeout<Request, Response>(
        &self,
        protocol: &'static str,
        request: &Request,
        request_timeout: Duration,
    ) -> Result<Response, HttpExchangeError>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let envelope = HttpProviderRequestEnvelope { protocol, request };
        self.exchange_envelope(protocol, &envelope, request_timeout)
            .await
    }

    async fn exchange_envelope<Envelope, Response>(
        &self,
        protocol: &'static str,
        envelope: &Envelope,
        request_timeout: Duration,
    ) -> Result<Response, HttpExchangeError>
    where
        Envelope: Serialize,
        Response: DeserializeOwned,
    {
        let body = serialize_bounded_request(envelope, self.config.max_request_bytes)?;

        let mut request_builder = self
            .client
            .post(self.config.endpoint.as_url().clone())
            .header(CONTENT_TYPE, JSON_MEDIA_TYPE)
            .header(ACCEPT, JSON_MEDIA_TYPE)
            .timeout(request_timeout)
            .body(body);
        if let Some(authorization) = &self.config.authorization {
            request_builder = request_builder.header("authorization", authorization);
        }
        let response = request_builder.send().await.map_err(|error| {
            let retryable = error.is_timeout() || error.is_connect();
            HttpExchangeError::transport(
                format!("provider HTTP request failed: {error}"),
                retryable,
            )
        })?;

        if response.status().is_redirection() {
            return Err(HttpExchangeError::protocol(
                "provider HTTP redirects are not allowed",
            ));
        }
        if response.status() != StatusCode::OK {
            let retryable = response.status().is_server_error()
                || response.status() == StatusCode::TOO_MANY_REQUESTS;
            return Err(HttpExchangeError::transport(
                format!("provider HTTP endpoint returned {}", response.status()),
                retryable,
            ));
        }
        if !is_json_content_type(response.headers().get(CONTENT_TYPE)) {
            return Err(HttpExchangeError::protocol(
                "provider HTTP response must use application/json",
            ));
        }
        if response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > self.config.max_response_bytes as u64)
        {
            return Err(HttpExchangeError::protocol(format!(
                "provider response exceeds {} bytes",
                self.config.max_response_bytes
            )));
        }

        let body = read_bounded_body(response, self.config.max_response_bytes).await?;
        let envelope: HttpProviderResponseEnvelope<Response> = serde_json::from_slice(&body)
            .map_err(|error| {
                HttpExchangeError::protocol(format!(
                    "provider HTTP response is not a valid protocol envelope: {error}"
                ))
            })?;
        match envelope {
            HttpProviderResponseEnvelope::Success {
                protocol: response_protocol,
                response,
            } => {
                admit_response_protocol(&response_protocol, protocol)?;
                Ok(response)
            }
            HttpProviderResponseEnvelope::Failure {
                protocol: response_protocol,
                error,
            } => {
                admit_response_protocol(&response_protocol, protocol)?;
                Err(self.sanitize_remote_error(HttpExchangeError::remote(error)?))
            }
        }
    }

    fn sanitize_remote_error(&self, mut error: HttpExchangeError) -> HttpExchangeError {
        if let Some(authorization) = &self.config.authorization {
            error.message = error.message.replace(authorization, "[REDACTED]");
            if let Some((_, credential)) = authorization.split_once(' ') {
                if credential.len() >= 8 {
                    error.message = error.message.replace(credential, "[REDACTED]");
                }
            }
        }
        error
    }
}

struct BoundedBodyWriter {
    body: Vec<u8>,
    max_bytes: usize,
    exceeded: bool,
}

impl BoundedBodyWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            body: Vec::with_capacity(max_bytes.min(16 * 1_024)),
            max_bytes,
            exceeded: false,
        }
    }
}

impl Write for BoundedBodyWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.body.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("provider request body limit exceeded"));
        };
        if next_len > self.max_bytes {
            self.exceeded = true;
            return Err(io::Error::other("provider request body limit exceeded"));
        }
        self.body.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct HttpExchangeError {
    code: String,
    message: String,
    retryable: bool,
}

impl HttpExchangeError {
    fn protocol(message: impl Into<String>) -> Self {
        Self {
            code: "protocol_invalid".to_string(),
            message: message.into(),
            retryable: false,
        }
    }

    fn transport(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: "transport_failed".to_string(),
            message: message.into(),
            retryable,
        }
    }

    fn deadline(message: impl Into<String>) -> Self {
        Self {
            code: "deadline_exceeded".to_string(),
            message: message.into(),
            retryable: false,
        }
    }

    fn remote(error: HttpProviderRemoteError) -> Result<Self, Self> {
        if !valid_remote_code(&error.code) {
            return Err(Self::protocol(
                "provider HTTP error code is not a bounded dot-separated lowercase identifier",
            ));
        }
        if error.message.is_empty() || error.message.chars().count() > MAX_ERROR_MESSAGE_CHARACTERS
        {
            return Err(Self::protocol(format!(
                "provider HTTP error message must contain 1 to {MAX_ERROR_MESSAGE_CHARACTERS} characters"
            )));
        }
        Ok(Self {
            code: format!("remote.{}", error.code),
            message: error.message,
            retryable: error.retryable,
        })
    }
}

pub struct HttpLlmProvider {
    identity: LlmIdentity,
    transport: HttpProviderTransport,
}

impl fmt::Debug for HttpLlmProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpLlmProvider")
            .field("identity", &self.identity)
            .field("transport", &self.transport)
            .finish()
    }
}

impl HttpLlmProvider {
    pub fn new(
        identity: LlmIdentity,
        config: HttpProviderConfig,
    ) -> Result<Self, HttpProviderConfigError> {
        validate_identity(&identity.provider, &identity.model)?;
        Ok(Self {
            identity,
            transport: HttpProviderTransport::new(config)?,
        })
    }
}

#[async_trait]
impl LlmProvider for HttpLlmProvider {
    fn identity(&self) -> LlmIdentity {
        self.identity.clone()
    }

    async fn complete(
        &self,
        request: StructuredLlmRequest,
    ) -> Result<StructuredLlmResponse, LlmError> {
        self.transport
            .exchange_without_wire_deadline(LLM_PROVIDER_PROTOCOL, &request)
            .await
            .map_err(llm_http_error)
    }
}

pub struct HttpContractGenerationProvider {
    identity: ContractGenerationProviderIdentity,
    transport: HttpProviderTransport,
}

impl fmt::Debug for HttpContractGenerationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpContractGenerationProvider")
            .field("identity", &self.identity)
            .field("transport", &self.transport)
            .finish()
    }
}

impl HttpContractGenerationProvider {
    pub fn new(
        identity: ContractGenerationProviderIdentity,
        config: HttpProviderConfig,
    ) -> Result<Self, HttpProviderConfigError> {
        validate_identity(&identity.provider, &identity.model)?;
        Ok(Self {
            identity,
            transport: HttpProviderTransport::new(config)?,
        })
    }
}

#[async_trait]
impl ContractGenerationProvider for HttpContractGenerationProvider {
    fn identity(&self) -> ContractGenerationProviderIdentity {
        self.identity.clone()
    }

    async fn generate(
        &self,
        request: ContractGenerationProviderRequest,
    ) -> Result<ContractGenerationProviderResponse, ContractGenerationError> {
        self.transport
            .exchange(
                CONTRACT_GENERATION_PROVIDER_PROTOCOL,
                &request,
                request.issued_at_unix_ms,
                request.deadline_unix_ms,
            )
            .await
            .and_then(|response: ContractGenerationProviderResponse| {
                if response.identity == self.identity {
                    Ok(response)
                } else {
                    Err(HttpExchangeError::protocol(
                        "provider HTTP response identity does not match the configured provider",
                    ))
                }
            })
            .map_err(contract_http_error)
    }
}

pub struct HttpVisualGroundingProvider {
    identity: GroundingProviderIdentity,
    transport: HttpProviderTransport,
}

impl fmt::Debug for HttpVisualGroundingProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpVisualGroundingProvider")
            .field("identity", &self.identity)
            .field("transport", &self.transport)
            .finish()
    }
}

impl HttpVisualGroundingProvider {
    pub fn new(
        identity: GroundingProviderIdentity,
        config: HttpProviderConfig,
    ) -> Result<Self, HttpProviderConfigError> {
        validate_identity(&identity.provider, &identity.model)?;
        Ok(Self {
            identity,
            transport: HttpProviderTransport::new(config)?,
        })
    }
}

#[async_trait]
impl VisualGroundingProvider for HttpVisualGroundingProvider {
    fn identity(&self) -> GroundingProviderIdentity {
        self.identity.clone()
    }

    async fn locate(
        &self,
        request: GroundingProviderRequest,
    ) -> Result<GroundingProviderResponse, GroundingError> {
        admitted_request_timeout(
            request.issued_at_unix_ms,
            request.deadline_unix_ms,
            self.transport.config.timeout,
        )
        .map_err(grounding_http_error)?;
        let screenshot_path = request.screenshot_path.clone();
        let screenshot_sha256 = request.screenshot_sha256.clone();
        let issued_at_unix_ms = request.issued_at_unix_ms;
        let deadline_unix_ms = request.deadline_unix_ms;
        let screenshot_bytes = read_grounding_image(&screenshot_path).await?;
        let actual_sha256 = format!("sha256:{:x}", Sha256::digest(&screenshot_bytes));
        if actual_sha256 != screenshot_sha256 {
            return Err(GroundingError::new(
                "test.agent.grounding.http.image_mismatch",
                "grounding image bytes changed after request admission",
                false,
            ));
        }
        let wire_request = GroundingProviderRequest {
            screenshot_path: "observation.png".to_string(),
            ..request
        };
        let image = GroundingImageAttachment {
            screenshot_sha256,
            media_type: "image/png".to_string(),
            bytes_base64: BASE64_STANDARD.encode(screenshot_bytes),
        };
        self.transport
            .exchange_visual_grounding(
                VISUAL_GROUNDING_PROVIDER_PROTOCOL,
                &wire_request,
                &image,
                issued_at_unix_ms,
                deadline_unix_ms,
            )
            .await
            .and_then(|response: GroundingProviderResponse| {
                if response.identity == self.identity {
                    Ok(response)
                } else {
                    Err(HttpExchangeError::protocol(
                        "provider HTTP response identity does not match the configured provider",
                    ))
                }
            })
            .map_err(grounding_http_error)
    }
}

pub struct HttpDesignAuditProvider {
    identity: DesignAuditProviderIdentity,
    transport: HttpProviderTransport,
}

impl fmt::Debug for HttpDesignAuditProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpDesignAuditProvider")
            .field("identity", &self.identity)
            .field("transport", &self.transport)
            .finish()
    }
}

impl HttpDesignAuditProvider {
    pub fn new(
        identity: DesignAuditProviderIdentity,
        config: HttpProviderConfig,
    ) -> Result<Self, HttpProviderConfigError> {
        validate_identity(&identity.provider, &identity.model)?;
        Ok(Self {
            identity,
            transport: HttpProviderTransport::new(config)?,
        })
    }
}

#[async_trait]
impl DesignAuditProvider for HttpDesignAuditProvider {
    fn identity(&self) -> DesignAuditProviderIdentity {
        self.identity.clone()
    }

    async fn audit(
        &self,
        request: DesignAuditProviderRequest,
    ) -> Result<DesignAuditProviderResponse, DesignAuditError> {
        admitted_request_timeout(
            request.issued_at_unix_ms,
            request.deadline_unix_ms,
            self.transport.config.timeout,
        )
        .map_err(design_audit_http_error)?;
        let screenshot_path = request.screenshot_path.clone();
        let screenshot_sha256 = request.screenshot_sha256.clone();
        let actual_page_context_sha256 = serde_json::to_vec(&request.page_context)
            .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
            .map_err(|error| {
                DesignAuditError::new(
                    "test.agent.design_audit.http.context_invalid",
                    format!("failed to encode design-audit page context: {error}"),
                    false,
                )
            })?;
        if actual_page_context_sha256 != request.page_context_sha256 {
            return Err(DesignAuditError::new(
                "test.agent.design_audit.http.context_mismatch",
                "design-audit page context does not match its admitted SHA-256 digest",
                false,
            ));
        }
        let issued_at_unix_ms = request.issued_at_unix_ms;
        let deadline_unix_ms = request.deadline_unix_ms;
        let screenshot_bytes = read_design_audit_image(&screenshot_path).await?;
        let actual_sha256 = format!("sha256:{:x}", Sha256::digest(&screenshot_bytes));
        if actual_sha256 != screenshot_sha256 {
            return Err(DesignAuditError::new(
                "test.agent.design_audit.http.image_mismatch",
                "design-audit image bytes changed after request admission",
                false,
            ));
        }
        let wire_request = DesignAuditProviderRequest {
            screenshot_path: "observation.png".to_string(),
            ..request
        };
        let image = DesignAuditImageAttachment {
            screenshot_sha256,
            media_type: "image/png".to_string(),
            bytes_base64: BASE64_STANDARD.encode(screenshot_bytes),
        };
        self.transport
            .exchange_design_audit(
                DESIGN_AUDIT_PROVIDER_PROTOCOL,
                &wire_request,
                &image,
                issued_at_unix_ms,
                deadline_unix_ms,
            )
            .await
            .and_then(|response: DesignAuditProviderResponse| {
                if response.identity == self.identity {
                    Ok(response)
                } else {
                    Err(HttpExchangeError::protocol(
                        "provider HTTP response identity does not match the configured provider",
                    ))
                }
            })
            .map_err(design_audit_http_error)
    }
}

async fn read_design_audit_image(path: &str) -> Result<Vec<u8>, DesignAuditError> {
    const MAX_PATH_BYTES: usize = 16 * 1_024;
    if path.trim().is_empty() || path.len() > MAX_PATH_BYTES {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.http.image_invalid",
            format!("design-audit image path must contain 1 to {MAX_PATH_BYTES} bytes"),
            false,
        ));
    }
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        DesignAuditError::new(
            "test.agent.design_audit.http.image_invalid",
            format!("failed to inspect design-audit image: {error}"),
            false,
        )
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_DESIGN_AUDIT_IMAGE_BYTES
    {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.http.image_invalid",
            "design-audit image must be a bounded non-empty regular file",
            false,
        ));
    }
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        DesignAuditError::new(
            "test.agent.design_audit.http.image_invalid",
            format!("failed to open design-audit image: {error}"),
            false,
        )
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_DESIGN_AUDIT_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            DesignAuditError::new(
                "test.agent.design_audit.http.image_invalid",
                format!("failed to read design-audit image: {error}"),
                false,
            )
        })?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_DESIGN_AUDIT_IMAGE_BYTES {
        return Err(DesignAuditError::new(
            "test.agent.design_audit.http.image_invalid",
            "design-audit image changed outside the admitted size bound",
            false,
        ));
    }
    Ok(bytes)
}

async fn read_grounding_image(path: &str) -> Result<Vec<u8>, GroundingError> {
    validate_grounding_image_path(path)?;
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        GroundingError::new(
            "test.agent.grounding.http.image_invalid",
            format!("failed to inspect grounding image: {error}"),
            false,
        )
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_GROUNDING_IMAGE_BYTES
    {
        return Err(GroundingError::new(
            "test.agent.grounding.http.image_invalid",
            "grounding image must be a bounded non-empty regular file",
            false,
        ));
    }
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        GroundingError::new(
            "test.agent.grounding.http.image_invalid",
            format!("failed to open grounding image: {error}"),
            false,
        )
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_GROUNDING_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            GroundingError::new(
                "test.agent.grounding.http.image_invalid",
                format!("failed to read grounding image: {error}"),
                false,
            )
        })?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_GROUNDING_IMAGE_BYTES {
        return Err(GroundingError::new(
            "test.agent.grounding.http.image_invalid",
            "grounding image changed outside the admitted size bound",
            false,
        ));
    }
    Ok(bytes)
}

fn validate_grounding_image_path(path: &str) -> Result<(), GroundingError> {
    const MAX_PATH_BYTES: usize = 16 * 1_024;
    if path.trim().is_empty() || path.len() > MAX_PATH_BYTES {
        return Err(GroundingError::new(
            "test.agent.grounding.http.image_invalid",
            format!("grounding image path must contain 1 to {MAX_PATH_BYTES} bytes"),
            false,
        ));
    }
    Ok(())
}

fn admitted_request_timeout(
    issued_at_unix_ms: u64,
    deadline_unix_ms: u64,
    configured_timeout: Duration,
) -> Result<Duration, HttpExchangeError> {
    if deadline_unix_ms <= issued_at_unix_ms {
        return Err(HttpExchangeError::deadline(
            "provider request deadline must be later than its issue time",
        ));
    }
    let now_unix_ms = unix_ms()?;
    let remaining_ms = deadline_unix_ms.checked_sub(now_unix_ms).ok_or_else(|| {
        HttpExchangeError::deadline("provider request deadline has already elapsed")
    })?;
    if remaining_ms == 0 {
        return Err(HttpExchangeError::deadline(
            "provider request deadline has already elapsed",
        ));
    }
    Ok(configured_timeout.min(Duration::from_millis(remaining_ms)))
}

fn unix_ms() -> Result<u64, HttpExchangeError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HttpExchangeError::deadline("system clock is earlier than the Unix epoch"))?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| HttpExchangeError::deadline("system clock exceeds the provider wire range"))
}

fn contract_http_error(error: HttpExchangeError) -> ContractGenerationError {
    ContractGenerationError::new(
        format!("test.agent.contract_generation.http.{}", error.code),
        error.message,
        error.retryable,
    )
}

fn grounding_http_error(error: HttpExchangeError) -> GroundingError {
    GroundingError::new(
        format!("test.agent.grounding.http.{}", error.code),
        error.message,
        error.retryable,
    )
}

fn design_audit_http_error(error: HttpExchangeError) -> DesignAuditError {
    DesignAuditError::new(
        format!("test.agent.design_audit.http.{}", error.code),
        error.message,
        error.retryable,
    )
}

fn llm_http_error(error: HttpExchangeError) -> LlmError {
    LlmError::new(
        format!("test.agent.llm.http.{}", error.code),
        error.message,
        error.retryable,
    )
}

fn serialize_bounded_request<Request>(
    request: &Request,
    max_request_bytes: usize,
) -> Result<Vec<u8>, HttpExchangeError>
where
    Request: Serialize,
{
    let mut writer = BoundedBodyWriter::new(max_request_bytes);
    if let Err(error) = serde_json::to_writer(&mut writer, request) {
        if writer.exceeded {
            return Err(HttpExchangeError::protocol(format!(
                "provider request exceeds {max_request_bytes} bytes"
            )));
        }
        return Err(HttpExchangeError::protocol(format!(
            "failed to serialize provider request: {error}"
        )));
    }
    Ok(writer.body)
}

fn admit_response_protocol(actual: &str, expected: &'static str) -> Result<(), HttpExchangeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(HttpExchangeError::protocol(
            "provider HTTP response protocol does not match the requested protocol",
        ))
    }
}

fn validate_identity(provider: &str, model: &str) -> Result<(), HttpProviderConfigError> {
    if provider.trim().is_empty()
        || model.trim().is_empty()
        || provider.len() > 1_024
        || model.len() > 1_024
    {
        return Err(HttpProviderConfigError::new(
            "provider and model identities must be non-empty and bounded",
        ));
    }
    Ok(())
}

fn is_json_content_type(value: Option<&reqwest::header::HeaderValue>) -> bool {
    value
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(JSON_MEDIA_TYPE))
}

async fn read_bounded_body(
    mut response: reqwest::Response,
    max_response_bytes: usize,
) -> Result<Vec<u8>, HttpExchangeError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        HttpExchangeError::transport(
            format!("failed to read provider HTTP response: {error}"),
            error.is_timeout(),
        )
    })? {
        let remaining = max_response_bytes.saturating_sub(body.len());
        if chunk.len() > remaining {
            return Err(HttpExchangeError::protocol(format!(
                "provider response exceeds {max_response_bytes} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    if body.is_empty() {
        return Err(HttpExchangeError::protocol(
            "provider HTTP response body is empty",
        ));
    }
    Ok(body)
}

fn valid_remote_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 128
        && code
            .split('.')
            .all(|segment| !segment.is_empty() && segment.chars().all(valid_code_character))
}

fn valid_code_character(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
}
