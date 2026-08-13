use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

use url::Url;

const MIN_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MIN_BODY_BYTES: usize = 1;
const MAX_BODY_BYTES: usize = 64 * 1_024 * 1_024;

#[derive(Clone)]
pub struct HttpProviderConfig {
    pub(super) endpoint: HttpProviderEndpoint,
    pub(super) timeout: Duration,
    pub(super) max_request_bytes: usize,
    pub(super) max_response_bytes: usize,
    pub(super) authorization: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpProviderEndpoint(Url);

impl HttpProviderEndpoint {
    fn new(endpoint: Url) -> Result<Self, HttpProviderConfigError> {
        validate_endpoint(&endpoint)?;
        Ok(Self(endpoint))
    }

    #[must_use]
    pub(super) fn as_url(&self) -> &Url {
        &self.0
    }
}

impl FromStr for HttpProviderEndpoint {
    type Err = HttpProviderConfigError;

    fn from_str(endpoint: &str) -> Result<Self, Self::Err> {
        let endpoint = Url::parse(endpoint).map_err(|error| {
            HttpProviderConfigError::new(format!("invalid provider endpoint: {error}"))
        })?;
        Self::new(endpoint)
    }
}

impl fmt::Debug for HttpProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpProviderConfig")
            .field("endpoint", &self.endpoint)
            .field("timeout", &self.timeout)
            .field("max_request_bytes", &self.max_request_bytes)
            .field("max_response_bytes", &self.max_response_bytes)
            .field(
                "authorization",
                &self.authorization.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl HttpProviderConfig {
    #[must_use]
    pub fn new(endpoint: HttpProviderEndpoint) -> Self {
        Self {
            endpoint,
            timeout: Duration::from_secs(30),
            max_request_bytes: 64 * 1_024 * 1_024,
            max_response_bytes: 32 * 1_024 * 1_024,
            authorization: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, HttpProviderConfigError> {
        self.timeout = timeout;
        self.validate()?;
        Ok(self)
    }

    pub fn with_body_limits(
        mut self,
        max_request_bytes: usize,
        max_response_bytes: usize,
    ) -> Result<Self, HttpProviderConfigError> {
        self.max_request_bytes = max_request_bytes;
        self.max_response_bytes = max_response_bytes;
        self.validate()?;
        Ok(self)
    }

    pub fn with_authorization(
        mut self,
        authorization: impl Into<String>,
    ) -> Result<Self, HttpProviderConfigError> {
        self.authorization = Some(authorization.into());
        self.validate()?;
        Ok(self)
    }

    #[must_use]
    pub fn endpoint(&self) -> &HttpProviderEndpoint {
        &self.endpoint
    }

    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    #[must_use]
    pub fn max_request_bytes(&self) -> usize {
        self.max_request_bytes
    }

    #[must_use]
    pub fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }

    pub(super) fn validate(&self) -> Result<(), HttpProviderConfigError> {
        validate_endpoint(self.endpoint.as_url())?;
        if self.timeout < MIN_TIMEOUT || self.timeout > MAX_TIMEOUT {
            return Err(HttpProviderConfigError::new(
                "provider timeout must be between 1 millisecond and 5 minutes",
            ));
        }
        if !(MIN_BODY_BYTES..=MAX_BODY_BYTES).contains(&self.max_request_bytes)
            || !(MIN_BODY_BYTES..=MAX_BODY_BYTES).contains(&self.max_response_bytes)
        {
            return Err(HttpProviderConfigError::new(format!(
                "provider body limits must be between {MIN_BODY_BYTES} and {MAX_BODY_BYTES} bytes"
            )));
        }
        if self.authorization.as_ref().is_some_and(|value| {
            value.trim().is_empty()
                || value.len() > 16 * 1_024
                || value.contains('\r')
                || value.contains('\n')
        }) {
            return Err(HttpProviderConfigError::new(
                "provider authorization must be non-empty, bounded, and single-line",
            ));
        }
        if self
            .authorization
            .as_ref()
            .is_some_and(|value| reqwest::header::HeaderValue::try_from(value.as_str()).is_err())
        {
            return Err(HttpProviderConfigError::new(
                "provider authorization is not a valid HTTP header value",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct HttpProviderConfigError {
    message: String,
}

impl HttpProviderConfigError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

fn validate_endpoint(endpoint: &Url) -> Result<(), HttpProviderConfigError> {
    if endpoint.username() != "" || endpoint.password().is_some() {
        return Err(HttpProviderConfigError::new(
            "provider endpoint credentials must be supplied through the authorization option",
        ));
    }
    if endpoint.query().is_some() || endpoint.fragment().is_some() {
        return Err(HttpProviderConfigError::new(
            "provider endpoint must not contain a query or fragment",
        ));
    }
    let host = endpoint.host_str().ok_or_else(|| {
        HttpProviderConfigError::new("provider endpoint must include an explicit hostname")
    })?;
    match endpoint.scheme() {
        "https" => Ok(()),
        "http" if is_loopback_host(host) => Ok(()),
        "http" => Err(HttpProviderConfigError::new(
            "plaintext provider endpoints are allowed only on explicit loopback addresses",
        )),
        _ => Err(HttpProviderConfigError::new(
            "provider endpoint must use HTTPS or loopback HTTP",
        )),
    }
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
