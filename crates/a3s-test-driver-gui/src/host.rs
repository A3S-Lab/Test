use a3s_test_core::DriverError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::CuaPermissionReport;
use crate::{CuaEndpoint, GuiPlatform};

pub const GUI_HOST_PERMISSION_PROTOCOL: &str = "a3s.test.gui-host-permissions/1";

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum GuiHostPermission {
    Accessibility,
    ScreenRecording,
}

impl GuiHostPermission {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::ScreenRecording => "screen_recording",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiHostPermissionSource {
    DriverDaemon,
    Host,
}

impl GuiHostPermissionSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::DriverDaemon => "driver_daemon",
            Self::Host => "host",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiHostPermissionGrant {
    pub protocol: String,
    pub source: GuiHostPermissionSource,
    #[schemars(length(min = 1, max = 2))]
    pub permissions: Vec<GuiHostPermission>,
}

impl GuiHostPermissionGrant {
    #[must_use]
    pub fn required(source: GuiHostPermissionSource) -> Self {
        Self {
            protocol: GUI_HOST_PERMISSION_PROTOCOL.to_string(),
            source,
            permissions: vec![
                GuiHostPermission::Accessibility,
                GuiHostPermission::ScreenRecording,
            ],
        }
    }

    /// Windows UI Automation grant. Window capture is part of `get_window_state`
    /// and is not a macOS screen-recording TCC grant.
    #[must_use]
    pub fn windows_uia(source: GuiHostPermissionSource) -> Self {
        Self {
            protocol: GUI_HOST_PERMISSION_PROTOCOL.to_string(),
            source,
            permissions: vec![GuiHostPermission::Accessibility],
        }
    }

    pub fn validate(&self) -> Result<(), DriverError> {
        if self.protocol != GUI_HOST_PERMISSION_PROTOCOL {
            return Err(permission_contract_error(format!(
                "unsupported GUI host permission protocol {:?}",
                self.protocol
            )));
        }
        let macos = [
            GuiHostPermission::Accessibility,
            GuiHostPermission::ScreenRecording,
        ];
        let windows = [GuiHostPermission::Accessibility];
        if self.permissions != macos && self.permissions.as_slice() != windows {
            return Err(permission_contract_error(
                "GUI host permissions must be accessibility plus screen_recording, or accessibility alone for Windows UI Automation",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.protocol.as_bytes());
        hasher.update([0]);
        hasher.update(self.source.as_str().as_bytes());
        for permission in &self.permissions {
            hasher.update([0]);
            hasher.update(permission.as_str().as_bytes());
        }
        format!("sha256:{:x}", hasher.finalize())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuiHostProbe {
    pub driver_version: String,
    pub protocol_version: String,
    pub capability_vocabulary: String,
    pub tools_schema: String,
    pub permissions: GuiHostPermissionGrant,
}

pub(crate) fn validate_permissions(
    endpoint: &CuaEndpoint,
    platform: GuiPlatform,
    permissions: &CuaPermissionReport,
) -> Result<GuiHostPermissionGrant, DriverError> {
    let expected = match endpoint {
        CuaEndpoint::InstalledDaemon { .. } => GuiHostPermissionSource::DriverDaemon,
        CuaEndpoint::EmbeddedSocket { .. } => GuiHostPermissionSource::Host,
    };
    let grant = match (platform, permissions) {
        (
            GuiPlatform::MacOs,
            CuaPermissionReport::MacOs {
                accessibility,
                screen_recording,
                source,
            },
        ) => {
            let actual = permission_source(&source.attribution)?;
            if actual != expected {
                return Err(attribution_mismatch(&source.attribution, expected));
            }
            if !accessibility || !screen_recording {
                let mut missing = Vec::new();
                if !accessibility {
                    missing.push("accessibility");
                }
                if !screen_recording {
                    missing.push("screen_recording");
                }
                return Err(DriverError::new(
                    "test.driver.gui.permission_missing",
                    format!(
                        "CUA is missing required permissions: {}",
                        missing.join(", ")
                    ),
                ));
            }
            GuiHostPermissionGrant::required(actual)
        }
        (GuiPlatform::Windows, CuaPermissionReport::Windows { uia, post_message }) => {
            if !uia || !post_message {
                let mut missing = Vec::new();
                if !uia {
                    missing.push("uia");
                }
                if !post_message {
                    missing.push("post_message");
                }
                return Err(DriverError::new(
                    "test.driver.gui.permission_missing",
                    format!(
                        "CUA is missing required permissions: {}",
                        missing.join(", ")
                    ),
                ));
            }
            GuiHostPermissionGrant::windows_uia(expected)
        }
        (GuiPlatform::MacOs, CuaPermissionReport::Windows { .. })
        | (GuiPlatform::Windows, CuaPermissionReport::MacOs { .. })
        | (GuiPlatform::Linux, _) => {
            return Err(DriverError::new(
                "test.driver.gui.permission_identity_invalid",
                "CUA permission report does not match the selected GUI platform",
            ));
        }
    };
    grant.validate()?;
    Ok(grant)
}

fn permission_source(attribution: &str) -> Result<GuiHostPermissionSource, DriverError> {
    match attribution {
        "driver-daemon" => Ok(GuiHostPermissionSource::DriverDaemon),
        "host" => Ok(GuiHostPermissionSource::Host),
        attribution => Err(DriverError::new(
            "test.driver.gui.permission_identity_invalid",
            format!("CUA returned unsupported permission attribution '{attribution}'"),
        )),
    }
}

fn attribution_mismatch(actual: &str, expected: GuiHostPermissionSource) -> DriverError {
    DriverError::new(
        "test.driver.gui.permission_identity_invalid",
        format!(
            "CUA permission status is attributed to '{actual}', expected '{}'",
            match expected {
                GuiHostPermissionSource::DriverDaemon => "driver-daemon",
                GuiHostPermissionSource::Host => "host",
            }
        ),
    )
}

fn permission_contract_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.gui.permission_contract_invalid", message)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        validate_permissions, GuiHostPermission, GuiHostPermissionGrant, GuiHostPermissionSource,
    };
    use crate::api::CuaPermissionReport;
    use crate::{CuaEndpoint, GuiPlatform};

    #[test]
    fn windows_installed_daemon_grant_is_accessibility_only() {
        let endpoint = CuaEndpoint::InstalledDaemon {
            proxy_executable: PathBuf::from("cua-driver"),
        };
        let grant = validate_permissions(
            &endpoint,
            GuiPlatform::Windows,
            &CuaPermissionReport::Windows {
                uia: true,
                post_message: true,
            },
        )
        .expect("Windows UI Automation grant");
        assert_eq!(
            grant,
            GuiHostPermissionGrant::windows_uia(GuiHostPermissionSource::DriverDaemon)
        );
        assert_eq!(grant.permissions, vec![GuiHostPermission::Accessibility]);
    }

    #[test]
    fn linux_permission_report_does_not_select_a_grant() {
        let endpoint = CuaEndpoint::InstalledDaemon {
            proxy_executable: PathBuf::from("cua-driver"),
        };
        let error = validate_permissions(
            &endpoint,
            GuiPlatform::Linux,
            &CuaPermissionReport::Windows {
                uia: true,
                post_message: true,
            },
        )
        .expect_err("Linux has no reviewed permission grant");
        assert_eq!(error.code(), "test.driver.gui.permission_identity_invalid");
    }
}
