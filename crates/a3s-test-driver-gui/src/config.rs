use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Duration;

use a3s_test_core::DriverError;

use crate::{
    CuaCompatibility, GuiCertificationStatus, GuiEndpointMode, GuiExecutionProfile, GuiPlatform,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CuaEndpoint {
    InstalledDaemon {
        proxy_executable: PathBuf,
    },
    EmbeddedSocket {
        proxy_executable: PathBuf,
        socket: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationIdentity {
    MacOsBundle {
        bundle_id: String,
    },
    WindowsExecutable {
        path: PathBuf,
        expected_publisher: Option<String>,
    },
    LinuxDesktop {
        desktop_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchSpec {
    pub application: ApplicationIdentity,
    pub arguments: Vec<OsString>,
    pub working_directory: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachSpec {
    pub application: ApplicationIdentity,
    pub process_id: Option<NonZeroU32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuiAppTarget {
    Launch(LaunchSpec),
    Attach(AttachSpec),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowSelector {
    Primary,
    ExactTitle(String),
    AutomationId(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiCaptureScope {
    Window,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiProfile {
    Semantic,
    WindowVision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuiDriverConfig {
    pub endpoint: CuaEndpoint,
    pub policy_file: PathBuf,
    pub target: GuiAppTarget,
    pub window: WindowSelector,
    pub capture_scope: GuiCaptureScope,
    pub profile: GuiProfile,
    pub command_timeout: Duration,
    pub removed_environment: BTreeSet<OsString>,
}

impl GuiDriverConfig {
    pub fn validate(&self) -> Result<(), DriverError> {
        validate_endpoint(&self.endpoint)?;
        validate_path(&self.policy_file, "CUA policy file")?;
        if !self.policy_file.is_absolute() {
            return Err(config_error("CUA policy file must be an absolute path"));
        }
        validate_target(&self.target)?;
        validate_window(&self.window)?;
        if self.command_timeout.is_zero() {
            return Err(config_error("command timeout must be greater than zero"));
        }
        if self.removed_environment.len() > 128
            || self
                .removed_environment
                .iter()
                .any(|name| !valid_environment_name(name))
        {
            return Err(config_error(
                "removed environment variable names must be bounded and non-empty",
            ));
        }
        self.execution_profile()?;
        Ok(())
    }

    pub fn execution_profile(&self) -> Result<GuiExecutionProfile, DriverError> {
        let platform = application_identity(&self.target).platform();
        let endpoint = self.endpoint.mode();
        let compatibility = CuaCompatibility::locked()?;
        let profile = compatibility
            .execution_profile(platform, endpoint)
            .cloned()
            .ok_or_else(|| {
                DriverError::new(
                    "test.driver.gui.platform_unsupported",
                    format!(
                        "the compatibility lock has no {} {} execution profile",
                        platform.as_str(),
                        endpoint.as_str()
                    ),
                )
            })?;
        if profile.status() == GuiCertificationStatus::Unsupported {
            return Err(DriverError::new(
                "test.driver.gui.platform_unsupported",
                profile.reason().unwrap_or(
                    "the selected platform and endpoint are unsupported by the compatibility lock",
                ),
            ));
        }
        match self.profile {
            GuiProfile::Semantic if !profile.semantic() => {
                return Err(profile_capability_error(&profile, "semantic"));
            }
            GuiProfile::WindowVision if !profile.window_vision() => {
                return Err(profile_capability_error(&profile, "window_vision"));
            }
            _ => {}
        }
        if !profile.lifecycle() {
            return Err(profile_capability_error(&profile, "lifecycle"));
        }
        Ok(profile)
    }
}

impl CuaEndpoint {
    #[must_use]
    pub fn mode(&self) -> GuiEndpointMode {
        match self {
            Self::InstalledDaemon { .. } => GuiEndpointMode::InstalledDaemon,
            Self::EmbeddedSocket { .. } => GuiEndpointMode::EmbeddedSocket,
        }
    }
}

impl ApplicationIdentity {
    #[must_use]
    pub fn platform(&self) -> GuiPlatform {
        match self {
            Self::MacOsBundle { .. } => GuiPlatform::MacOs,
            Self::WindowsExecutable { .. } => GuiPlatform::Windows,
            Self::LinuxDesktop { .. } => GuiPlatform::Linux,
        }
    }
}

fn application_identity(target: &GuiAppTarget) -> &ApplicationIdentity {
    match target {
        GuiAppTarget::Launch(spec) => &spec.application,
        GuiAppTarget::Attach(spec) => &spec.application,
    }
}

fn profile_capability_error(profile: &GuiExecutionProfile, capability: &str) -> DriverError {
    DriverError::new(
        "test.driver.gui.profile_unsupported",
        format!(
            "GUI execution profile '{}' does not admit {capability}",
            profile.id()
        ),
    )
}

fn validate_endpoint(endpoint: &CuaEndpoint) -> Result<(), DriverError> {
    match endpoint {
        CuaEndpoint::InstalledDaemon { proxy_executable } => {
            validate_path(proxy_executable, "installed CUA proxy executable")
        }
        CuaEndpoint::EmbeddedSocket {
            proxy_executable,
            socket,
        } => {
            validate_path(proxy_executable, "embedded CUA proxy executable")?;
            validate_path(socket, "embedded CUA socket")
        }
    }
}

fn validate_target(target: &GuiAppTarget) -> Result<(), DriverError> {
    match target {
        GuiAppTarget::Launch(spec) => {
            validate_application(&spec.application)?;
            if let Some(path) = &spec.working_directory {
                validate_path(path, "application working directory")?;
            }
            Ok(())
        }
        GuiAppTarget::Attach(spec) => validate_application(&spec.application),
    }
}

fn validate_application(application: &ApplicationIdentity) -> Result<(), DriverError> {
    match application {
        ApplicationIdentity::MacOsBundle { bundle_id } => {
            validate_text(bundle_id, "macOS bundle identifier")
        }
        ApplicationIdentity::WindowsExecutable {
            path,
            expected_publisher,
        } => {
            validate_path(path, "Windows application executable")?;
            if let Some(publisher) = expected_publisher {
                validate_text(publisher, "Windows publisher identity")?;
            }
            Ok(())
        }
        ApplicationIdentity::LinuxDesktop { desktop_id } => {
            validate_text(desktop_id, "Linux desktop identifier")
        }
    }
}

fn validate_window(selector: &WindowSelector) -> Result<(), DriverError> {
    match selector {
        WindowSelector::Primary => Ok(()),
        WindowSelector::ExactTitle(value) => validate_text(value, "window title"),
        WindowSelector::AutomationId(value) => validate_text(value, "window automation id"),
    }
}

fn validate_path(path: &std::path::Path, name: &str) -> Result<(), DriverError> {
    if path.as_os_str().is_empty() {
        Err(config_error(format!("{name} must not be empty")))
    } else {
        Ok(())
    }
}

fn validate_text(value: &str, name: &str) -> Result<(), DriverError> {
    if value.trim().is_empty() {
        Err(config_error(format!("{name} must not be empty")))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn valid_environment_name(name: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 4_096
        && !bytes.iter().any(|byte| matches!(byte, b'=' | b'\0'))
}

#[cfg(windows)]
fn valid_environment_name(name: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;

    let mut length = 0_usize;
    for character in name.encode_wide() {
        length = length.saturating_add(1);
        if character == u16::from(b'=') || character == 0 {
            return false;
        }
    }
    (1..=4_096).contains(&length)
}

#[cfg(not(any(unix, windows)))]
fn valid_environment_name(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        !name.is_empty() && name.len() <= 4_096 && !name.bytes().any(|byte| byte == b'=')
    })
}

fn config_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.gui.config_invalid", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> GuiDriverConfig {
        GuiDriverConfig {
            endpoint: CuaEndpoint::InstalledDaemon {
                proxy_executable: PathBuf::from("cua-driver"),
            },
            policy_file: std::env::temp_dir().join("a3s-test-policy.yaml"),
            target: GuiAppTarget::Launch(LaunchSpec {
                application: ApplicationIdentity::MacOsBundle {
                    bundle_id: "com.example.Calculator".to_string(),
                },
                arguments: Vec::new(),
                working_directory: None,
            }),
            window: WindowSelector::Primary,
            capture_scope: GuiCaptureScope::Window,
            profile: GuiProfile::Semantic,
            command_timeout: Duration::from_secs(30),
            removed_environment: BTreeSet::new(),
        }
    }

    #[test]
    fn accepts_typed_window_scoped_configuration() {
        valid_config().validate().expect("valid GUI config");
    }

    #[test]
    fn rejects_empty_application_identity() {
        let mut config = valid_config();
        config.target = GuiAppTarget::Attach(AttachSpec {
            application: ApplicationIdentity::LinuxDesktop {
                desktop_id: " ".to_string(),
            },
            process_id: None,
        });
        let error = config.validate().expect_err("empty application identity");
        assert_eq!(error.code(), "test.driver.gui.config_invalid");
    }

    #[test]
    fn rejects_uncertified_platform_before_transport_startup() {
        let mut config = valid_config();
        config.target = GuiAppTarget::Launch(LaunchSpec {
            application: ApplicationIdentity::WindowsExecutable {
                path: PathBuf::from("C:/Program Files/Example/editor.exe"),
                expected_publisher: Some("Example, Inc.".to_string()),
            },
            arguments: Vec::new(),
            working_directory: None,
        });
        let error = config.validate().expect_err("unsupported Windows profile");
        assert_eq!(error.code(), "test.driver.gui.platform_unsupported");
    }

    #[test]
    fn rejects_unbounded_environment_removal() {
        let mut config = valid_config();
        config
            .removed_environment
            .insert(OsString::from("x".repeat(4_097)));

        let error = config
            .validate()
            .expect_err("unbounded environment removal");
        assert_eq!(error.code(), "test.driver.gui.config_invalid");

        let mut config = valid_config();
        config
            .removed_environment
            .insert(OsString::from("INVALID=NAME"));
        let error = config
            .validate()
            .expect_err("invalid environment removal name");
        assert_eq!(error.code(), "test.driver.gui.config_invalid");
    }
}
