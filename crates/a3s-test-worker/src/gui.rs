use a3s_test_driver_gui::{
    GuiHostPermissionGrant, GuiHostPermissionSource, GUI_HOST_PERMISSION_PROTOCOL,
};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use super::{inventory_error, WorkerCapabilityError, WorkerOperatingSystem};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerGuiEndpoint {
    InstalledDaemon,
    EmbeddedSocket,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerGuiPerception {
    Semantic,
    WindowVision,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerGuiTarget {
    Launch,
    Attach,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerGuiApplication {
    MacosBundle {
        #[schemars(length(min = 1, max = 256))]
        bundle_id: String,
    },
    WindowsExecutable {
        #[schemars(length(min = 1, max = 1024))]
        path: String,
        #[schemars(length(min = 1, max = 256))]
        expected_publisher: Option<String>,
    },
    LinuxDesktop {
        #[schemars(length(min = 1, max = 256))]
        desktop_id: String,
    },
}

impl WorkerGuiApplication {
    #[must_use]
    pub fn operating_system(&self) -> WorkerOperatingSystem {
        match self {
            Self::MacosBundle { .. } => WorkerOperatingSystem::Macos,
            Self::WindowsExecutable { .. } => WorkerOperatingSystem::Windows,
            Self::LinuxDesktop { .. } => WorkerOperatingSystem::Linux,
        }
    }

    fn validate(&self) -> Result<(), WorkerCapabilityError> {
        match self {
            Self::MacosBundle { bundle_id } => validate_text(bundle_id, 256, "macOS bundle ID"),
            Self::WindowsExecutable {
                path,
                expected_publisher,
            } => {
                validate_text(path, 1_024, "Windows executable path")?;
                if let Some(publisher) = expected_publisher {
                    validate_text(publisher, 256, "Windows publisher")?;
                }
                Ok(())
            }
            Self::LinuxDesktop { desktop_id } => validate_text(desktop_id, 256, "Linux desktop ID"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerGuiCapability {
    #[schemars(length(min = 1, max = 128))]
    pub profile_id: String,
    #[schemars(length(min = 1, max = 128))]
    pub compatibility_profile: String,
    pub endpoint: WorkerGuiEndpoint,
    pub perception: WorkerGuiPerception,
    pub target: WorkerGuiTarget,
    pub application: WorkerGuiApplication,
    #[schemars(length(min = 1, max = 128))]
    pub cua_driver_version: String,
    #[schemars(length(min = 1, max = 128))]
    pub mcp_protocol: String,
    #[schemars(length(min = 1, max = 128))]
    pub capability_vocabulary: String,
    #[schemars(length(min = 1, max = 128))]
    pub tools_schema: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub configuration_digest: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub policy_digest: String,
    pub host_permissions: GuiHostPermissionGrant,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub host_permission_digest: String,
}

impl WorkerGuiCapability {
    pub(crate) fn validate(&self) -> Result<(), WorkerCapabilityError> {
        validate_identifier(&self.profile_id, "GUI profile ID")?;
        validate_identifier(&self.compatibility_profile, "GUI compatibility profile ID")?;
        self.application.validate()?;
        Version::parse(&self.cua_driver_version).map_err(|_| {
            inventory_error(
                "test.worker.inventory.gui_capability_invalid",
                "GUI driver version must be semantic",
            )
        })?;
        for (value, label) in [
            (&self.mcp_protocol, "GUI MCP protocol"),
            (&self.capability_vocabulary, "GUI capability vocabulary"),
            (&self.tools_schema, "GUI tools schema"),
        ] {
            validate_text(value, 128, label)?;
        }
        validate_digest(&self.configuration_digest, "GUI configuration digest")?;
        validate_digest(&self.policy_digest, "GUI policy digest")?;
        self.host_permissions.validate().map_err(|error| {
            inventory_error(
                "test.worker.inventory.host_permission_invalid",
                format!("GUI host permission grant is invalid: {error}"),
            )
        })?;
        if self.host_permissions.protocol != GUI_HOST_PERMISSION_PROTOCOL {
            return Err(inventory_error(
                "test.worker.inventory.host_permission_invalid",
                "GUI host permission protocol is unsupported",
            ));
        }
        let expected_source = match self.endpoint {
            WorkerGuiEndpoint::InstalledDaemon => GuiHostPermissionSource::DriverDaemon,
            WorkerGuiEndpoint::EmbeddedSocket => GuiHostPermissionSource::Host,
        };
        if self.host_permissions.source != expected_source {
            return Err(inventory_error(
                "test.worker.inventory.host_permission_source_mismatch",
                "GUI host permission source does not match the endpoint ownership model",
            ));
        }
        validate_digest(&self.host_permission_digest, "GUI host permission digest")?;
        if self.host_permission_digest != self.host_permissions.digest() {
            return Err(inventory_error(
                "test.worker.inventory.host_permission_digest_mismatch",
                "GUI host permission grant does not match its digest",
            ));
        }
        Ok(())
    }
}

fn validate_identifier(value: &str, label: &str) -> Result<(), WorkerCapabilityError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(inventory_error(
            "test.worker.inventory.gui_capability_invalid",
            format!("{label} must be a bounded portable identifier"),
        ));
    }
    Ok(())
}

fn validate_text(value: &str, max_bytes: usize, label: &str) -> Result<(), WorkerCapabilityError> {
    if value.trim().is_empty() || value.len() > max_bytes {
        return Err(inventory_error(
            "test.worker.inventory.gui_capability_invalid",
            format!("{label} must be bounded and non-empty"),
        ));
    }
    Ok(())
}

fn validate_digest(value: &str, label: &str) -> Result<(), WorkerCapabilityError> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(inventory_error(
            "test.worker.inventory.gui_capability_invalid",
            format!("{label} must be a canonical lowercase SHA-256 digest"),
        ));
    }
    Ok(())
}
