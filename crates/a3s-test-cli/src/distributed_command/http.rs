use std::net::IpAddr;
use std::time::Duration;

use a3s_test_worker::{
    RemoteArtifactOutcome, RemoteArtifactRequest, RemoteArtifactResponse, RemoteWorkerError,
    RemoteWorkerOutcome, RemoteWorkerRequest, RemoteWorkerResponse, REMOTE_ARTIFACT_PROTOCOL,
    REMOTE_WORKER_PROTOCOL,
};
use anyhow::{Context, Result};
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{redirect, Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use super::config::WorkerConfig;

const JSON_MEDIA_TYPE: &str = "application/json";
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct RemoteHttpClient {
    client: Client,
    worker_endpoint: Url,
    artifact_endpoint: Url,
    authorization: HeaderValue,
    instance_id: String,
}

impl std::fmt::Debug for RemoteHttpClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteHttpClient")
            .field("worker_endpoint", &self.worker_endpoint)
            .field("artifact_endpoint", &self.artifact_endpoint)
            .field("authorization", &"<redacted>")
            .field("instance_id", &self.instance_id)
            .finish()
    }
}

impl RemoteHttpClient {
    pub(super) fn new(config: &WorkerConfig, timeout: Duration) -> Result<Self> {
        let endpoint = validate_endpoint(&config.endpoint)?;
        let worker_endpoint = endpoint
            .join("v1/worker")
            .context("failed to construct worker endpoint")?;
        let artifact_endpoint = endpoint
            .join("v1/artifacts")
            .context("failed to construct artifact endpoint")?;
        let authorization = read_authorization(&config.authorization_env)?;
        let client = Client::builder()
            .no_proxy()
            .redirect(redirect::Policy::none())
            .connect_timeout(timeout)
            .timeout(timeout)
            .user_agent(concat!("a3s-test-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to construct distributed worker HTTP client")?;
        Ok(Self {
            client,
            worker_endpoint,
            artifact_endpoint,
            authorization,
            instance_id: config.instance_id.clone(),
        })
    }

    pub(super) async fn worker(
        &self,
        request: &RemoteWorkerRequest,
    ) -> Result<RemoteWorkerResponse> {
        request.validate().map_err(anyhow::Error::new)?;
        let mut response: RemoteWorkerResponse = self
            .exchange(&self.worker_endpoint, request)
            .await
            .with_context(|| format!("worker '{}' request failed", self.instance_id))?;
        if response.protocol != REMOTE_WORKER_PROTOCOL || response.request_id != request.request_id
        {
            anyhow::bail!("remote worker returned a mismatched protocol or request ID");
        }
        match &mut response.outcome {
            RemoteWorkerOutcome::Job { job } => {
                if let Some(error) = &mut job.error {
                    self.sanitize_remote_error(error);
                }
            }
            RemoteWorkerOutcome::Error { error } => self.sanitize_remote_error(error),
            RemoteWorkerOutcome::Descriptor { .. } => {}
        }
        Ok(response)
    }

    pub(super) async fn artifacts(
        &self,
        request: &RemoteArtifactRequest,
    ) -> Result<RemoteArtifactResponse> {
        request.validate().map_err(anyhow::Error::new)?;
        let mut response: RemoteArtifactResponse = self
            .exchange(&self.artifact_endpoint, request)
            .await
            .with_context(|| format!("worker '{}' artifact request failed", self.instance_id))?;
        if response.protocol != REMOTE_ARTIFACT_PROTOCOL
            || response.request_id != request.request_id
        {
            anyhow::bail!("remote artifact service returned a mismatched protocol or request ID");
        }
        match &mut response.outcome {
            RemoteArtifactOutcome::Reports { page } => {
                for report in &mut page.reports {
                    if let Some(error) = &mut report.job.error {
                        self.sanitize_remote_error(error);
                    }
                }
            }
            RemoteArtifactOutcome::Error { error } => self.sanitize_remote_error(error),
            _ => {}
        }
        Ok(response)
    }

    async fn exchange<Request, Response>(
        &self,
        endpoint: &Url,
        request: &Request,
    ) -> Result<Response>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let body = serde_json::to_vec(request).context("failed to encode remote worker request")?;
        let response = self
            .client
            .post(endpoint.clone())
            .header(CONTENT_TYPE, JSON_MEDIA_TYPE)
            .header(ACCEPT, JSON_MEDIA_TYPE)
            .header("authorization", self.authorization.clone())
            .body(body)
            .send()
            .await
            .map_err(|error| self.transport_error(error.to_string()))?;
        if response.status().is_redirection() {
            anyhow::bail!("remote worker redirects are not allowed");
        }
        if response.status() != StatusCode::OK {
            anyhow::bail!("remote worker endpoint returned {}", response.status());
        }
        if !response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|value| value.trim().eq_ignore_ascii_case(JSON_MEDIA_TYPE))
        {
            anyhow::bail!("remote worker response must use application/json");
        }
        if response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .is_some_and(|length| length > MAX_RESPONSE_BYTES)
        {
            anyhow::bail!("remote worker response exceeds its body limit");
        }
        let bytes = read_bounded(response).await?;
        serde_json::from_slice(&bytes).context("remote worker returned invalid strict JSON")
    }

    fn transport_error(&self, message: String) -> anyhow::Error {
        anyhow::anyhow!(
            "remote worker HTTP request failed: {}",
            self.sanitize_message(&message)
        )
    }

    fn sanitize_remote_error(&self, error: &mut RemoteWorkerError) {
        error.code = self.sanitize_message(&error.code);
        error.message = self.sanitize_message(&error.message);
        if error.code.is_empty()
            || error.code.len() > 128
            || !error
                .code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            error.code = "test.worker.remote.error_code_invalid".to_string();
            error.message =
                "remote worker returned an invalid error code or reflected authorization data"
                    .to_string();
            error.retryable = false;
        }
        error.message = bounded(&error.message, 2_048);
        if error.message.trim().is_empty() {
            error.message = "remote worker returned an empty error message".to_string();
        }
    }

    fn sanitize_message(&self, message: &str) -> String {
        let mut sanitized = message.to_string();
        if let Ok(authorization) = self.authorization.to_str() {
            sanitized = sanitized.replace(authorization, "[REDACTED]");
            if let Some((_, credential)) = authorization.split_once(' ') {
                if credential.len() >= 8 {
                    sanitized = sanitized.replace(credential, "[REDACTED]");
                }
            }
        }
        sanitized
    }
}

fn bounded(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

async fn read_bounded(mut response: reqwest::Response) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("failed to read remote worker response body")?
    {
        let next = bytes
            .len()
            .checked_add(chunk.len())
            .context("remote worker response size overflowed")?;
        if next > MAX_RESPONSE_BYTES {
            anyhow::bail!("remote worker response exceeds its body limit");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn validate_endpoint(value: &str) -> Result<Url> {
    let mut endpoint = Url::parse(value).context("invalid distributed worker endpoint")?;
    if endpoint.username() != "" || endpoint.password().is_some() {
        anyhow::bail!("worker endpoint credentials must come from authorization_env");
    }
    if endpoint.query().is_some() || endpoint.fragment().is_some() {
        anyhow::bail!("worker endpoint cannot contain a query or fragment");
    }
    let host = endpoint
        .host_str()
        .context("worker endpoint must include a hostname")?;
    match endpoint.scheme() {
        "https" => {}
        "http" if is_loopback_host(host) => {}
        "http" => anyhow::bail!("plaintext worker endpoints are allowed only on loopback"),
        _ => anyhow::bail!("worker endpoint must use HTTPS or loopback HTTP"),
    }
    if endpoint.path() != "/" && !endpoint.path().is_empty() {
        anyhow::bail!("worker endpoint must be an origin without a path");
    }
    endpoint.set_path("/");
    Ok(endpoint)
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(host)
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn read_authorization(name: &str) -> Result<HeaderValue> {
    let value = std::env::var(name).with_context(|| {
        format!("worker authorization environment variable '{name}' is missing")
    })?;
    if value.trim().is_empty()
        || value.len() > 16 * 1024
        || value.contains('\r')
        || value.contains('\n')
    {
        anyhow::bail!("worker authorization must be bounded, non-empty, and single-line");
    }
    HeaderValue::from_str(&value).context("worker authorization is not a valid HTTP header value")
}

#[cfg(test)]
mod tests {
    use super::{validate_endpoint, RemoteHttpClient};
    use crate::distributed_command::config::WorkerConfig;
    use a3s_test_worker::RemoteWorkerError;
    use std::time::Duration;

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn worker(endpoint: &str, authorization_env: &str) -> WorkerConfig {
        WorkerConfig {
            instance_id: "runner".to_string(),
            endpoint: endpoint.to_string(),
            image_digest: DIGEST.to_string(),
            inventory_digest: None,
            authorization_env: authorization_env.to_string(),
            max_parallel_scenarios: 1,
        }
    }

    #[test]
    fn endpoint_policy_requires_https_or_explicit_loopback() {
        assert!(validate_endpoint("https://worker.example.test").is_ok());
        assert!(validate_endpoint("http://127.0.0.1:9400").is_ok());
        assert!(validate_endpoint("http://worker.example.test").is_err());
        assert!(validate_endpoint("https://user:secret@worker.example.test").is_err());
        assert!(validate_endpoint("https://worker.example.test/path").is_err());
    }

    #[test]
    fn debug_output_redacts_authorization() {
        let name = "A3S_TEST_WORKER_AUTHORIZATION_HTTP_TEST";
        std::env::set_var(name, "Bearer distributed-secret-value");
        let client = RemoteHttpClient::new(
            &worker("http://127.0.0.1:9400", name),
            Duration::from_secs(1),
        )
        .expect("HTTP client");
        let debug = format!("{client:?}");
        assert!(!debug.contains("distributed-secret-value"));
        assert!(debug.contains("<redacted>"));
        let reflected = client.sanitize_message(
            "server echoed Bearer distributed-secret-value and distributed-secret-value",
        );
        assert!(!reflected.contains("distributed-secret-value"));
        assert_eq!(reflected.matches("[REDACTED]").count(), 2);
        let mut remote_error = RemoteWorkerError::new(
            "distributed-secret-value",
            "server reflected Bearer distributed-secret-value",
            true,
        );
        client.sanitize_remote_error(&mut remote_error);
        assert_eq!(remote_error.code, "test.worker.remote.error_code_invalid");
        assert!(!remote_error.message.contains("distributed-secret-value"));
        assert!(!remote_error.retryable);
        std::env::remove_var(name);
    }
}
