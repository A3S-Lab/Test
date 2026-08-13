use std::fmt;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{redirect, Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

mod config;
mod wire;

pub use config::{HttpProviderConfig, HttpProviderConfigError, HttpProviderEndpoint};
pub use wire::{
    HttpContractGenerationRequest, HttpContractGenerationResponse, HttpProviderErrorResponse,
    HttpVisualGroundingRequest, HttpVisualGroundingResponse,
};
use wire::{HttpProviderRemoteError, HttpProviderRequestEnvelope, HttpProviderResponseEnvelope};

use crate::{
    ContractGenerationError, ContractGenerationProvider, ContractGenerationProviderIdentity,
    ContractGenerationProviderRequest, ContractGenerationProviderResponse, GroundingError,
    GroundingProviderIdentity, GroundingProviderRequest, GroundingProviderResponse,
    VisualGroundingProvider, CONTRACT_GENERATION_PROVIDER_PROTOCOL,
    VISUAL_GROUNDING_PROVIDER_PROTOCOL,
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
        admitted_request_timeout(issued_at_unix_ms, deadline_unix_ms, self.config.timeout)?;
        let envelope = HttpProviderRequestEnvelope { protocol, request };
        let body = serialize_bounded_request(&envelope, self.config.max_request_bytes)?;
        let request_timeout =
            admitted_request_timeout(issued_at_unix_ms, deadline_unix_ms, self.config.timeout)?;

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
        self.transport
            .exchange(
                VISUAL_GROUNDING_PROVIDER_PROTOCOL,
                &request,
                request.issued_at_unix_ms,
                request.deadline_unix_ms,
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
