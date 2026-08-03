use std::collections::{BTreeMap, BTreeSet};

use a3s_acl::{Block, Value};
use a3s_test_core::DriverError;
use semver::Version;
use serde::Serialize;

const LOCK_SOURCE: &str = include_str!("../../../compat/cua-stack.acl");
const SUPPORTED_LOCK_SCHEMA: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiPlatform {
    #[serde(rename = "macos")]
    MacOs,
    Windows,
    Linux,
}

impl GuiPlatform {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MacOs => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiEndpointMode {
    InstalledDaemon,
    EmbeddedSocket,
}

impl GuiEndpointMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InstalledDaemon => "installed_daemon",
            Self::EmbeddedSocket => "embedded_socket",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiCertificationStatus {
    ContractTested,
    Unsupported,
}

impl GuiCertificationStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContractTested => "contract_tested",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GuiExecutionProfile {
    id: String,
    platform: GuiPlatform,
    endpoint: GuiEndpointMode,
    semantic: bool,
    window_vision: bool,
    lifecycle: bool,
    status: GuiCertificationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl GuiExecutionProfile {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn platform(&self) -> GuiPlatform {
        self.platform
    }

    #[must_use]
    pub fn endpoint(&self) -> GuiEndpointMode {
        self.endpoint
    }

    #[must_use]
    pub fn semantic(&self) -> bool {
        self.semantic
    }

    #[must_use]
    pub fn window_vision(&self) -> bool {
        self.window_vision
    }

    #[must_use]
    pub fn lifecycle(&self) -> bool {
        self.lifecycle
    }

    #[must_use]
    pub fn status(&self) -> GuiCertificationStatus {
        self.status
    }

    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GuiCertificationMatrix {
    lock_schema: u32,
    cua_repository: String,
    cua_revision: String,
    cua_driver_version: String,
    mcp_protocol: String,
    profiles: Vec<GuiExecutionProfile>,
}

impl GuiCertificationMatrix {
    pub fn locked() -> Result<Self, DriverError> {
        let compatibility = CuaCompatibility::locked()?;
        Ok(Self {
            lock_schema: compatibility.schema_version,
            cua_repository: compatibility.repository,
            cua_revision: compatibility.revision,
            cua_driver_version: compatibility.driver_version.to_string(),
            mcp_protocol: compatibility.mcp_protocol,
            profiles: compatibility.execution_profiles.into_values().collect(),
        })
    }

    #[must_use]
    pub fn profiles(&self) -> &[GuiExecutionProfile] {
        &self.profiles
    }

    #[must_use]
    pub fn cua_revision(&self) -> &str {
        &self.cua_revision
    }

    #[must_use]
    pub fn cua_driver_version(&self) -> &str {
        &self.cua_driver_version
    }

    #[must_use]
    pub fn mcp_protocol(&self) -> &str {
        &self.mcp_protocol
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CuaToolRequirement {
    capabilities: BTreeSet<String>,
}

impl CuaToolRequirement {
    #[must_use]
    pub fn capabilities(&self) -> &BTreeSet<String> {
        &self.capabilities
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CuaCompatibility {
    schema_version: u32,
    repository: String,
    revision: String,
    driver_version: Version,
    mcp_protocol: String,
    tools_schema: String,
    capability_vocabulary: String,
    tools: BTreeMap<String, CuaToolRequirement>,
    observation_fields: BTreeSet<String>,
    visual_observation_fields: BTreeSet<String>,
    execution_profiles: BTreeMap<String, GuiExecutionProfile>,
}

impl CuaCompatibility {
    pub fn locked() -> Result<Self, DriverError> {
        parse_lock(LOCK_SOURCE)
    }

    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    #[must_use]
    pub fn driver_version(&self) -> &Version {
        &self.driver_version
    }

    #[must_use]
    pub fn mcp_protocol(&self) -> &str {
        &self.mcp_protocol
    }

    #[must_use]
    pub fn tools_schema(&self) -> &str {
        &self.tools_schema
    }

    #[must_use]
    pub fn capability_vocabulary(&self) -> &str {
        &self.capability_vocabulary
    }

    #[must_use]
    pub fn tools(&self) -> &BTreeMap<String, CuaToolRequirement> {
        &self.tools
    }

    #[must_use]
    pub fn observation_fields(&self) -> &BTreeSet<String> {
        &self.observation_fields
    }

    #[must_use]
    pub fn visual_observation_fields(&self) -> &BTreeSet<String> {
        &self.visual_observation_fields
    }

    #[must_use]
    pub fn execution_profiles(&self) -> &BTreeMap<String, GuiExecutionProfile> {
        &self.execution_profiles
    }

    #[must_use]
    pub fn execution_profile(
        &self,
        platform: GuiPlatform,
        endpoint: GuiEndpointMode,
    ) -> Option<&GuiExecutionProfile> {
        self.execution_profiles
            .values()
            .find(|profile| profile.platform == platform && profile.endpoint == endpoint)
    }
}

fn parse_lock(source: &str) -> Result<CuaCompatibility, DriverError> {
    let document = a3s_acl::parse(source)
        .map_err(|error| lock_error(format!("failed to parse compat/cua-stack.acl: {error}")))?;
    if document.blocks.len() != 1 {
        return Err(lock_error(
            "compat/cua-stack.acl must contain exactly one cua_stack block",
        ));
    }
    let root = &document.blocks[0];
    if root.name != "cua_stack" || root.labels.as_slice() != ["gui"] {
        return Err(lock_error(
            "compat/cua-stack.acl must declare cua_stack \"gui\"",
        ));
    }
    ensure_attributes(
        root,
        &[
            "schema_version",
            "repository",
            "revision",
            "driver_version",
            "mcp_protocol",
            "tools_schema",
            "capability_vocabulary",
        ],
        "cua_stack.gui",
    )?;

    let schema_version = required_u32(root, "schema_version", "cua_stack.gui")?;
    if schema_version != SUPPORTED_LOCK_SCHEMA {
        return Err(lock_error(format!(
            "unsupported CUA compatibility lock schema {schema_version}; expected {SUPPORTED_LOCK_SCHEMA}"
        )));
    }
    let repository = required_string(root, "repository", "cua_stack.gui")?;
    let revision = required_string(root, "revision", "cua_stack.gui")?;
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(lock_error(
            "cua_stack.gui.revision must be a full 40-character Git revision",
        ));
    }
    let driver_version_text = required_string(root, "driver_version", "cua_stack.gui")?;
    let driver_version = Version::parse(&driver_version_text).map_err(|error| {
        lock_error(format!(
            "cua_stack.gui.driver_version is not semantic versioning: {error}"
        ))
    })?;

    let mut tools = BTreeMap::new();
    let mut execution_profiles = BTreeMap::new();
    let mut profile_pairs = BTreeSet::new();
    let mut observation_fields = None;
    let mut visual_observation_fields = None;
    for child in &root.blocks {
        match child.name.as_str() {
            "profile" => {
                let id = one_label(child, "cua_stack.gui.profile")?;
                let path = format!("cua_stack.gui.profile.{id}");
                ensure_attributes(
                    child,
                    &[
                        "platform",
                        "endpoint",
                        "semantic",
                        "window_vision",
                        "lifecycle",
                        "status",
                        "reason",
                    ],
                    &path,
                )?;
                ensure_no_blocks(child, &path)?;
                let profile = parse_execution_profile(child, id.clone(), &path)?;
                if !profile_pairs.insert((profile.platform, profile.endpoint)) {
                    return Err(lock_error(format!(
                        "duplicate platform/endpoint pair at {path}"
                    )));
                }
                if execution_profiles.insert(id, profile).is_some() {
                    return Err(lock_error(format!(
                        "duplicate CUA execution profile at {path}"
                    )));
                }
            }
            "tool" => {
                let name = one_label(child, "cua_stack.gui.tool")?;
                let path = format!("cua_stack.gui.tool.{name}");
                ensure_attributes(child, &["capabilities"], &path)?;
                ensure_no_blocks(child, &path)?;
                let requirement = CuaToolRequirement {
                    capabilities: required_string_set(child, "capabilities", &path)?,
                };
                if tools.insert(name, requirement).is_some() {
                    return Err(lock_error(format!(
                        "duplicate CUA tool requirement at {path}"
                    )));
                }
            }
            "observation" => {
                let name = one_label(child, "cua_stack.gui.observation")?;
                if name != "get_window_state" || observation_fields.is_some() {
                    return Err(lock_error(
                        "the lock must contain exactly one observation \"get_window_state\" block",
                    ));
                }
                let path = "cua_stack.gui.observation.get_window_state";
                ensure_attributes(child, &["required_fields", "visual_fields"], path)?;
                ensure_no_blocks(child, path)?;
                observation_fields = Some(required_string_set(child, "required_fields", path)?);
                visual_observation_fields =
                    Some(required_string_set(child, "visual_fields", path)?);
            }
            name => {
                return Err(lock_error(format!(
                    "unsupported block cua_stack.gui.{name}"
                )));
            }
        }
    }
    if tools.is_empty() {
        return Err(lock_error("the CUA compatibility lock requires tools"));
    }
    ensure_profile_matrix(&profile_pairs)?;

    Ok(CuaCompatibility {
        schema_version,
        repository,
        revision,
        driver_version,
        mcp_protocol: required_string(root, "mcp_protocol", "cua_stack.gui")?,
        tools_schema: required_string(root, "tools_schema", "cua_stack.gui")?,
        capability_vocabulary: required_string(root, "capability_vocabulary", "cua_stack.gui")?,
        tools,
        observation_fields: observation_fields
            .ok_or_else(|| lock_error("the lock is missing observation \"get_window_state\""))?,
        visual_observation_fields: visual_observation_fields
            .ok_or_else(|| lock_error("the lock is missing get_window_state visual fields"))?,
        execution_profiles,
    })
}

fn parse_execution_profile(
    block: &Block,
    id: String,
    path: &str,
) -> Result<GuiExecutionProfile, DriverError> {
    let platform = match required_string(block, "platform", path)?.as_str() {
        "macos" => GuiPlatform::MacOs,
        "windows" => GuiPlatform::Windows,
        "linux" => GuiPlatform::Linux,
        value => {
            return Err(lock_error(format!(
                "{path}.platform has unsupported value {value}"
            )))
        }
    };
    let endpoint = match required_string(block, "endpoint", path)?.as_str() {
        "installed_daemon" => GuiEndpointMode::InstalledDaemon,
        "embedded_socket" => GuiEndpointMode::EmbeddedSocket,
        value => {
            return Err(lock_error(format!(
                "{path}.endpoint has unsupported value {value}"
            )))
        }
    };
    let status = match required_string(block, "status", path)?.as_str() {
        "contract_tested" => GuiCertificationStatus::ContractTested,
        "unsupported" => GuiCertificationStatus::Unsupported,
        value => {
            return Err(lock_error(format!(
                "{path}.status has unsupported value {value}"
            )))
        }
    };
    let profile = GuiExecutionProfile {
        id,
        platform,
        endpoint,
        semantic: required_bool(block, "semantic", path)?,
        window_vision: required_bool(block, "window_vision", path)?,
        lifecycle: required_bool(block, "lifecycle", path)?,
        status,
        reason: optional_string(block, "reason", path)?,
    };
    validate_execution_profile(&profile, path)?;
    Ok(profile)
}

fn validate_execution_profile(
    profile: &GuiExecutionProfile,
    path: &str,
) -> Result<(), DriverError> {
    let all_capabilities = profile.semantic && profile.window_vision && profile.lifecycle;
    match profile.status {
        GuiCertificationStatus::ContractTested if !all_capabilities => Err(lock_error(format!(
            "{path} must enable semantic, window_vision, and lifecycle when contract tested"
        ))),
        GuiCertificationStatus::ContractTested if profile.reason.is_some() => Err(lock_error(
            format!("{path}.reason is only valid for unsupported profiles"),
        )),
        GuiCertificationStatus::Unsupported
            if profile.semantic || profile.window_vision || profile.lifecycle =>
        {
            Err(lock_error(format!(
                "{path} cannot enable capabilities while unsupported"
            )))
        }
        GuiCertificationStatus::Unsupported if profile.reason.is_none() => Err(lock_error(
            format!("{path}.reason is required for unsupported profiles"),
        )),
        _ => Ok(()),
    }
}

fn ensure_profile_matrix(
    profiles: &BTreeSet<(GuiPlatform, GuiEndpointMode)>,
) -> Result<(), DriverError> {
    let expected = [
        (GuiPlatform::MacOs, GuiEndpointMode::InstalledDaemon),
        (GuiPlatform::MacOs, GuiEndpointMode::EmbeddedSocket),
        (GuiPlatform::Windows, GuiEndpointMode::InstalledDaemon),
        (GuiPlatform::Windows, GuiEndpointMode::EmbeddedSocket),
        (GuiPlatform::Linux, GuiEndpointMode::InstalledDaemon),
        (GuiPlatform::Linux, GuiEndpointMode::EmbeddedSocket),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if profiles != &expected {
        return Err(lock_error(
            "the CUA compatibility lock must cover every platform/endpoint pair",
        ));
    }
    Ok(())
}

fn one_label(block: &Block, path: &str) -> Result<String, DriverError> {
    if block.labels.len() != 1 || block.labels[0].trim().is_empty() {
        return Err(lock_error(format!(
            "{path} requires exactly one non-empty label"
        )));
    }
    Ok(block.labels[0].clone())
}

fn ensure_attributes(block: &Block, allowed: &[&str], path: &str) -> Result<(), DriverError> {
    if let Some(name) = block
        .attributes
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        return Err(lock_error(format!("unsupported attribute {path}.{name}")));
    }
    Ok(())
}

fn ensure_no_blocks(block: &Block, path: &str) -> Result<(), DriverError> {
    if block.blocks.is_empty() {
        Ok(())
    } else {
        Err(lock_error(format!("{path} cannot contain nested blocks")))
    }
}

fn required_string(block: &Block, name: &str, path: &str) -> Result<String, DriverError> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| lock_error(format!("{path}.{name} must be a non-empty string")))
}

fn required_u32(block: &Block, name: &str, path: &str) -> Result<u32, DriverError> {
    let number = block
        .attributes
        .get(name)
        .and_then(Value::as_number)
        .ok_or_else(|| lock_error(format!("{path}.{name} must be an integer")))?;
    if !number.is_finite() || number.fract() != 0.0 || number < 1.0 || number > u32::MAX as f64 {
        return Err(lock_error(format!(
            "{path}.{name} must be a positive 32-bit integer"
        )));
    }
    Ok(number as u32)
}

fn required_bool(block: &Block, name: &str, path: &str) -> Result<bool, DriverError> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| lock_error(format!("{path}.{name} must be a boolean")))
}

fn optional_string(block: &Block, name: &str, path: &str) -> Result<Option<String>, DriverError> {
    match block.attributes.get(name) {
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .filter(|value| !value.trim().is_empty())
            .map(Some)
            .ok_or_else(|| lock_error(format!("{path}.{name} must be a non-empty string"))),
        None => Ok(None),
    }
}

fn required_string_set(
    block: &Block,
    name: &str,
    path: &str,
) -> Result<BTreeSet<String>, DriverError> {
    let Some(Value::List(values)) = block.attributes.get(name) else {
        return Err(lock_error(format!(
            "{path}.{name} must be a non-empty string list"
        )));
    };
    if values.is_empty() {
        return Err(lock_error(format!("{path}.{name} cannot be empty")));
    }
    let mut result = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let item = value
            .as_str()
            .filter(|item| !item.trim().is_empty())
            .ok_or_else(|| {
                lock_error(format!("{path}.{name}[{index}] must be a non-empty string"))
            })?;
        if !result.insert(item.to_owned()) {
            return Err(lock_error(format!(
                "{path}.{name} contains duplicate value {item}"
            )));
        }
    }
    Ok(result)
}

fn lock_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.gui.compatibility_lock_invalid", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_lock_is_canonical_and_complete() {
        let compatibility = CuaCompatibility::locked().expect("locked compatibility");
        assert_eq!(compatibility.schema_version(), 2);
        assert_eq!(compatibility.driver_version(), &Version::new(0, 10, 0));
        assert_eq!(compatibility.mcp_protocol(), "2025-06-18");
        assert_eq!(compatibility.tools_schema(), "1");
        assert_eq!(compatibility.capability_vocabulary(), "1");
        assert!(compatibility.tools().contains_key("get_window_state"));
        assert!(compatibility.tools().contains_key("start_session"));
        assert!(compatibility.tools().contains_key("end_session"));
        assert!(compatibility.observation_fields().contains("elements"));
        assert!(compatibility
            .visual_observation_fields()
            .contains("screenshot_mime_type"));
        assert_eq!(compatibility.execution_profiles().len(), 6);
        let macos = compatibility
            .execution_profile(GuiPlatform::MacOs, GuiEndpointMode::InstalledDaemon)
            .expect("macOS installed profile");
        assert_eq!(macos.status(), GuiCertificationStatus::ContractTested);
        let windows = compatibility
            .execution_profile(GuiPlatform::Windows, GuiEndpointMode::EmbeddedSocket)
            .expect("Windows embedded profile");
        assert_eq!(windows.status(), GuiCertificationStatus::Unsupported);
        assert!(windows.reason().is_some());
    }

    #[test]
    fn rejects_unreviewed_lock_schema() {
        let source = LOCK_SOURCE.replacen("schema_version = 2", "schema_version = 3", 1);
        let error = parse_lock(&source).expect_err("future lock schema");
        assert_eq!(error.code(), "test.driver.gui.compatibility_lock_invalid");
    }

    #[test]
    fn rejects_incomplete_platform_matrix() {
        let start = LOCK_SOURCE
            .find("    profile \"linux-embedded\"")
            .expect("profile start");
        let end = LOCK_SOURCE[start..]
            .find("\n    profile \"linux-installed\"")
            .map(|offset| start + offset)
            .expect("next profile");
        let source = format!("{}{}", &LOCK_SOURCE[..start], &LOCK_SOURCE[end..]);
        let error = parse_lock(&source).expect_err("missing profile");
        assert_eq!(error.code(), "test.driver.gui.compatibility_lock_invalid");
    }

    #[test]
    fn certification_json_uses_the_canonical_macos_name() {
        let matrix = GuiCertificationMatrix::locked().expect("certification matrix");
        let value = serde_json::to_value(matrix).expect("matrix JSON");
        assert!(value["profiles"]
            .as_array()
            .expect("profiles")
            .iter()
            .any(|profile| profile["platform"] == "macos"));
        assert!(!value.to_string().contains("mac_os"));
    }

    #[test]
    fn rejects_duplicate_tool_requirements() {
        let duplicate = r#"
    tool "click" {
        capabilities = ["input.pointer.click"]
    }
"#;
        let source = LOCK_SOURCE.replacen(
            "\n    tool \"start_session\"",
            &format!("{duplicate}\n    tool \"start_session\""),
            1,
        );
        let error = parse_lock(&source).expect_err("duplicate tool");
        assert_eq!(error.code(), "test.driver.gui.compatibility_lock_invalid");
    }
}
