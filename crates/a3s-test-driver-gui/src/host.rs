use a3s_test_core::DriverError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::CuaPermissions;
use crate::CuaEndpoint;

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
    #[schemars(length(min = 2, max = 2))]
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

    pub fn validate(&self) -> Result<(), DriverError> {
        if self.protocol != GUI_HOST_PERMISSION_PROTOCOL {
            return Err(permission_contract_error(format!(
                "unsupported GUI host permission protocol {:?}",
                self.protocol
            )));
        }
        if self.permissions
            != [
                GuiHostPermission::Accessibility,
                GuiHostPermission::ScreenRecording,
            ]
        {
            return Err(permission_contract_error(
                "GUI host permissions must contain accessibility and screen_recording in canonical order",
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
    permissions: &CuaPermissions,
) -> Result<GuiHostPermissionGrant, DriverError> {
    let expected = match endpoint {
        CuaEndpoint::InstalledDaemon { .. } => GuiHostPermissionSource::DriverDaemon,
        CuaEndpoint::EmbeddedSocket { .. } => GuiHostPermissionSource::Host,
    };
    let actual = match permissions.source.attribution.as_str() {
        "driver-daemon" => GuiHostPermissionSource::DriverDaemon,
        "host" => GuiHostPermissionSource::Host,
        attribution => {
            return Err(DriverError::new(
                "test.driver.gui.permission_identity_invalid",
                format!("CUA returned unsupported permission attribution '{attribution}'"),
            ));
        }
    };
    if actual != expected {
        return Err(DriverError::new(
            "test.driver.gui.permission_identity_invalid",
            format!(
                "CUA permission status is attributed to '{}', expected '{}'",
                permissions.source.attribution,
                match expected {
                    GuiHostPermissionSource::DriverDaemon => "driver-daemon",
                    GuiHostPermissionSource::Host => "host",
                }
            ),
        ));
    }
    if !permissions.accessibility || !permissions.screen_recording {
        let mut missing = Vec::new();
        if !permissions.accessibility {
            missing.push("accessibility");
        }
        if !permissions.screen_recording {
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
    let grant = GuiHostPermissionGrant::required(actual);
    grant.validate()?;
    Ok(grant)
}

fn permission_contract_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.gui.permission_contract_invalid", message)
}
