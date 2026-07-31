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

#[derive(Clone, Debug)]
pub struct AgentBrowserConfig {
    pub command: BrowserCommand,
    /// Exact daemon namespace. When empty, the runner's unique run id is used.
    pub namespace: String,
    pub headed: bool,
    pub command_timeout: Duration,
    pub idle_timeout: Duration,
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
