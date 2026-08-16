use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use a3s_test_core::DriverError;
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserCommand {
    A3s { executable: PathBuf },
    Standalone { executable: PathBuf },
}

/// Explicit microphone behavior for a browser session.
///
/// Real device capture is intentionally not automated. The synthetic profile
/// is opt-in and gives Chromium a deterministic local media device while
/// accepting the browser permission prompt without human intervention.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BrowserMicrophone {
    #[default]
    Disabled,
    Synthetic,
}

impl BrowserCommand {
    pub(crate) fn program(&self) -> &Path {
        match self {
            Self::A3s { executable } | Self::Standalone { executable } => executable,
        }
    }

    pub(crate) fn prefix(&self) -> Vec<OsString> {
        match self {
            Self::A3s { .. } => vec![OsString::from("use"), OsString::from("browser")],
            Self::Standalone { .. } => Vec::new(),
        }
    }

    pub(crate) fn namespace_environment(&self, namespace: &str) -> (OsString, OsString) {
        let name = match self {
            Self::A3s { .. } => "A3S_USE_BROWSER_NAMESPACE",
            Self::Standalone { .. } => "AGENT_BROWSER_NAMESPACE",
        };
        (OsString::from(name), OsString::from(namespace))
    }

    pub(crate) fn idle_environment(&self, timeout: Duration) -> (OsString, OsString) {
        let name = match self {
            Self::A3s { .. } => "A3S_USE_BROWSER_IDLE_TIMEOUT_MS",
            Self::Standalone { .. } => "AGENT_BROWSER_IDLE_TIMEOUT_MS",
        };
        (
            OsString::from(name),
            OsString::from(timeout.as_millis().to_string()),
        )
    }

    pub(crate) fn launch_arguments_environment(
        &self,
        headed: bool,
        microphone: BrowserMicrophone,
    ) -> Option<(OsString, OsString)> {
        if headed && microphone == BrowserMicrophone::Disabled {
            return None;
        }
        let (name, inherited) = match self {
            Self::A3s { .. } => (
                "A3S_USE_BROWSER_ARGS",
                first_environment_value(&["A3S_USE_BROWSER_ARGS", "AGENT_BROWSER_ARGS"]),
            ),
            Self::Standalone { .. } => (
                "AGENT_BROWSER_ARGS",
                first_environment_value(&["AGENT_BROWSER_ARGS"]),
            ),
        };
        let mut arguments = inherited.unwrap_or_default();
        if !headed {
            append_launch_argument(&mut arguments, "--headless=new");
        }
        if microphone == BrowserMicrophone::Synthetic {
            append_launch_argument(&mut arguments, "--use-fake-device-for-media-stream");
            append_launch_argument(&mut arguments, "--use-fake-ui-for-media-stream");
        }
        (!arguments.is_empty()).then(|| (OsString::from(name), OsString::from(arguments)))
    }

    pub(crate) fn runtime_environment(&self, runtime_dir: &Path) -> (OsString, OsString) {
        let name = match self {
            Self::A3s { .. } => "A3S_USE_BROWSER_SOCKET_DIR",
            Self::Standalone { .. } => "AGENT_BROWSER_SOCKET_DIR",
        };
        (OsString::from(name), runtime_dir.as_os_str().to_os_string())
    }

    pub(crate) fn network_policy_environment(
        &self,
        policy: &BrowserNetworkPolicy,
    ) -> Vec<(OsString, OsString)> {
        let mut values = Vec::new();
        match self {
            Self::A3s { .. } => {
                if let Some(domains) = policy.domains_environment_value() {
                    values.push((OsString::from("A3S_USE_BROWSER_ALLOWED_DOMAINS"), domains));
                }
                if let Some(origins) = policy.origins_environment_value() {
                    values.push((OsString::from("A3S_USE_BROWSER_ALLOWED_ORIGINS"), origins));
                }
            }
            Self::Standalone { .. } => {
                if let Some(domains) = policy.standalone_domains_environment_value() {
                    values.push((OsString::from("AGENT_BROWSER_ALLOWED_DOMAINS"), domains));
                }
            }
        }
        values
    }

    pub(crate) fn domain_policy_args(&self, policy: &BrowserNetworkPolicy) -> Vec<OsString> {
        let Some(value) = (match self {
            Self::A3s { .. } => policy.domains_environment_value(),
            Self::Standalone { .. } => policy.standalone_domains_environment_value(),
        }) else {
            return Vec::new();
        };
        match self {
            Self::A3s { .. } => Vec::new(),
            Self::Standalone { .. } => vec![
                OsString::from("--allowed-domains"),
                value,
                // Standalone 0.26.x does not install its domain interceptor
                // from the implicit auto-launch path. An explicit engine
                // makes it dispatch the launch command that installs Fetch
                // interception before the first navigation.
                OsString::from("--engine"),
                OsString::from("chrome"),
            ],
        }
    }

    pub(crate) fn process_markers(&self) -> Vec<String> {
        let mut markers = Vec::new();
        if let Ok(executable) = self.program().canonicalize() {
            if let Some(name) = executable.file_name().and_then(|name| name.to_str()) {
                markers.push(name.to_string());
            }
        }
        if let Some(name) = self.program().file_name().and_then(|name| name.to_str()) {
            if !markers.iter().any(|marker| marker == name) {
                markers.push(name.to_string());
            }
        }
        match self {
            Self::A3s { .. } => markers.push("a3s-use-browser-driver".to_string()),
            Self::Standalone { .. } => markers.push("agent-browser".to_string()),
        }
        markers
    }
}

fn first_environment_value(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    })
}

fn append_launch_argument(arguments: &mut String, value: &str) {
    if arguments
        .split([',', '\n'])
        .any(|argument| argument.trim() == value)
    {
        return;
    }
    if !arguments.is_empty() {
        arguments.push(',');
    }
    arguments.push_str(value);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BrowserNetworkPolicy {
    allowed_origins: Vec<String>,
    allowed_domains: Vec<String>,
}

impl BrowserNetworkPolicy {
    pub fn restricted<I, O, J, D>(origins: I, domains: J) -> Result<Self, DriverError>
    where
        I: IntoIterator<Item = O>,
        O: Into<String>,
        J: IntoIterator<Item = D>,
        D: Into<String>,
    {
        let mut normalized_origins = BTreeSet::new();
        for origin in origins {
            normalized_origins.insert(normalize_origin(&origin.into())?);
            if normalized_origins.len() > 64 {
                return Err(DriverError::new(
                    "test.driver.web.origin_policy_too_large",
                    "browser origin policy cannot contain more than 64 entries",
                ));
            }
        }
        let mut normalized_domains = BTreeSet::new();
        for domain in domains {
            normalized_domains.insert(normalize_domain_pattern(&domain.into())?);
            if normalized_domains.len() > 64 {
                return Err(DriverError::new(
                    "test.driver.web.domain_policy_too_large",
                    "browser domain policy cannot contain more than 64 entries",
                ));
            }
        }
        if normalized_origins.is_empty() && normalized_domains.is_empty() {
            return Err(DriverError::new(
                "test.driver.web.network_policy_invalid",
                "restricted browser network policy requires at least one origin or domain",
            ));
        }
        Ok(Self {
            allowed_origins: normalized_origins.into_iter().collect(),
            allowed_domains: normalized_domains.into_iter().collect(),
        })
    }

    pub fn restricted_to_domains<I, S>(domains: I) -> Result<Self, DriverError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::restricted(std::iter::empty::<String>(), domains)
    }

    #[must_use]
    pub fn allowed_origins(&self) -> &[String] {
        &self.allowed_origins
    }

    #[must_use]
    pub fn allowed_domains(&self) -> &[String] {
        &self.allowed_domains
    }

    fn domains_environment_value(&self) -> Option<OsString> {
        (!self.allowed_domains.is_empty()).then(|| OsString::from(self.allowed_domains.join(",")))
    }

    fn origins_environment_value(&self) -> Option<OsString> {
        (!self.allowed_origins.is_empty()).then(|| OsString::from(self.allowed_origins.join(",")))
    }

    fn standalone_domains_environment_value(&self) -> Option<OsString> {
        let mut domains = self
            .allowed_domains
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        domains.extend(self.allowed_origins.iter().filter_map(|origin| {
            Url::parse(origin)
                .ok()
                .and_then(|url| url.host_str().map(str::to_string))
        }));
        (!domains.is_empty())
            .then(|| OsString::from(domains.into_iter().collect::<Vec<_>>().join(",")))
    }
}

#[derive(Clone, Debug)]
pub struct AgentBrowserConfig {
    pub command: BrowserCommand,
    /// Exact daemon namespace. When empty, the runner's unique run id is used.
    pub namespace: String,
    pub headed: bool,
    pub command_timeout: Duration,
    pub idle_timeout: Duration,
    pub microphone: BrowserMicrophone,
    pub network_policy: BrowserNetworkPolicy,
}

impl AgentBrowserConfig {
    pub(crate) fn validate(&self) -> Result<(), DriverError> {
        if self.command.program().as_os_str().is_empty() {
            return Err(DriverError::new(
                "test.driver.web.program_missing",
                "browser driver executable is empty",
            ));
        }
        if self.command_timeout.is_zero() {
            return Err(DriverError::new(
                "test.driver.web.timeout_invalid",
                "browser command timeout must be greater than zero",
            ));
        }
        if self.idle_timeout.is_zero() {
            return Err(DriverError::new(
                "test.driver.web.idle_timeout_invalid",
                "browser idle timeout must be greater than zero",
            ));
        }
        Ok(())
    }
}

fn normalize_domain_pattern(value: &str) -> Result<String, DriverError> {
    if value.is_empty() || value.len() > 253 || value.trim() != value || !value.is_ascii() {
        return Err(invalid_domain_pattern(value));
    }
    let normalized = value.to_ascii_lowercase();
    let (wildcard, hostname) = match normalized.strip_prefix("*.") {
        Some(hostname) => (true, hostname),
        None => (false, normalized.as_str()),
    };
    if hostname.is_empty()
        || hostname.starts_with('.')
        || hostname.ends_with('.')
        || hostname.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                || !label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                || !label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        })
    {
        return Err(invalid_domain_pattern(value));
    }
    Ok(if wildcard {
        format!("*.{hostname}")
    } else {
        hostname.to_string()
    })
}

fn normalize_origin(value: &str) -> Result<String, DriverError> {
    let parsed = Url::parse(value).map_err(|_| invalid_origin(value))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.host_str().is_none()
    {
        return Err(invalid_origin(value));
    }
    Ok(parsed.origin().ascii_serialization())
}

fn invalid_origin(value: &str) -> DriverError {
    DriverError::new(
        "test.driver.web.origin_policy_invalid",
        format!(
            "invalid browser origin {value:?}; expected an HTTP(S) origin without credentials, path, query, or fragment"
        ),
    )
}

fn invalid_domain_pattern(value: &str) -> DriverError {
    DriverError::new(
        "test.driver.web.domain_policy_invalid",
        format!(
            "invalid browser domain pattern {value:?}; expected an ASCII hostname or a leading '*.' wildcard"
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{BrowserCommand, BrowserMicrophone, BrowserNetworkPolicy};

    #[test]
    fn launch_arguments_are_typed_and_deduplicated() {
        let command = BrowserCommand::Standalone {
            executable: PathBuf::from("agent-browser"),
        };
        assert_eq!(
            command.launch_arguments_environment(true, BrowserMicrophone::Synthetic),
            Some((
                "AGENT_BROWSER_ARGS".into(),
                "--use-fake-device-for-media-stream,--use-fake-ui-for-media-stream".into(),
            ))
        );
        assert_eq!(
            command.launch_arguments_environment(true, BrowserMicrophone::Disabled),
            None
        );
    }

    #[test]
    fn domain_policy_normalizes_and_deduplicates_safe_patterns() {
        let policy = BrowserNetworkPolicy::restricted_to_domains([
            "Example.COM",
            "*.cdn.example.com",
            "example.com",
            "127.0.0.1",
        ])
        .expect("domain policy");
        assert_eq!(
            policy.allowed_domains(),
            ["*.cdn.example.com", "127.0.0.1", "example.com"]
        );
    }

    #[test]
    fn domain_policy_rejects_urls_ports_and_environment_delimiters() {
        for invalid in [
            "https://example.com",
            "example.com:443",
            "example.com,evil.test",
            " example.com",
            "exa_mple.com",
        ] {
            let error = BrowserNetworkPolicy::restricted_to_domains([invalid])
                .expect_err("invalid domain policy");
            assert_eq!(error.code(), "test.driver.web.domain_policy_invalid");
        }
    }

    #[test]
    fn network_policy_normalizes_exact_origins_independently_from_domains() {
        let policy = BrowserNetworkPolicy::restricted(
            ["https://Example.COM", "http://127.0.0.1:8080"],
            ["*.cdn.example.com"],
        )
        .expect("network policy");

        assert_eq!(
            policy.allowed_origins(),
            ["http://127.0.0.1:8080", "https://example.com"]
        );
        assert_eq!(policy.allowed_domains(), ["*.cdn.example.com"]);
    }

    #[test]
    fn origin_policy_rejects_urls_that_are_not_origins() {
        for invalid in [
            "ftp://example.com",
            "https://user@example.com",
            "https://example.com/path",
            "https://example.com?query=1",
            "https://example.com#fragment",
        ] {
            let error = BrowserNetworkPolicy::restricted([invalid], std::iter::empty::<&str>())
                .expect_err("invalid origin policy");
            assert_eq!(error.code(), "test.driver.web.origin_policy_invalid");
        }
    }
}
