use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use a3s_test_core::DriverError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserCommand {
    A3s { executable: PathBuf },
    Standalone { executable: PathBuf },
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

    pub(crate) fn runtime_environment(&self, runtime_dir: &Path) -> (OsString, OsString) {
        let name = match self {
            Self::A3s { .. } => "A3S_USE_BROWSER_SOCKET_DIR",
            Self::Standalone { .. } => "AGENT_BROWSER_SOCKET_DIR",
        };
        (OsString::from(name), runtime_dir.as_os_str().to_os_string())
    }

    pub(crate) fn allowed_domains_environment(
        &self,
        policy: &BrowserNetworkPolicy,
    ) -> Option<(OsString, OsString)> {
        let value = policy.environment_value()?;
        let name = match self {
            Self::A3s { .. } => "A3S_USE_BROWSER_ALLOWED_DOMAINS",
            Self::Standalone { .. } => "AGENT_BROWSER_ALLOWED_DOMAINS",
        };
        Some((OsString::from(name), value))
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BrowserNetworkPolicy {
    allowed_domains: Vec<String>,
}

impl BrowserNetworkPolicy {
    pub fn restricted_to_domains<I, S>(domains: I) -> Result<Self, DriverError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut normalized = BTreeSet::new();
        for domain in domains {
            normalized.insert(normalize_domain_pattern(&domain.into())?);
            if normalized.len() > 64 {
                return Err(DriverError::new(
                    "test.driver.web.domain_policy_too_large",
                    "browser domain policy cannot contain more than 64 entries",
                ));
            }
        }
        if normalized.is_empty() {
            return Err(DriverError::new(
                "test.driver.web.domain_policy_invalid",
                "restricted browser domain policy requires at least one domain",
            ));
        }
        Ok(Self {
            allowed_domains: normalized.into_iter().collect(),
        })
    }

    #[must_use]
    pub fn allowed_domains(&self) -> &[String] {
        &self.allowed_domains
    }

    fn environment_value(&self) -> Option<OsString> {
        (!self.allowed_domains.is_empty()).then(|| OsString::from(self.allowed_domains.join(",")))
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
    use super::BrowserNetworkPolicy;

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
}
