use std::collections::BTreeSet;
use std::ffi::OsString;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::Duration;

use a3s_acl::{Block, Value};
use a3s_test_driver_gui::{
    ApplicationIdentity, AttachSpec, CuaEndpoint, GuiAppTarget, GuiCaptureScope, GuiDriver,
    GuiDriverConfig, GuiHostPermission, GuiHostPermissionGrant, GuiHostPermissionSource,
    GuiProfile, LaunchSpec, WindowSelector,
};
use a3s_test_worker::{
    WorkerGuiApplication, WorkerGuiCapability, WorkerGuiEndpoint, WorkerGuiPerception,
    WorkerGuiTarget,
};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

const MAX_GUI_PROFILE_BYTES: u64 = 1024 * 1024;
const MAX_GUI_POLICY_BYTES: u64 = 1024 * 1024;
const MAX_GUI_ARGUMENTS: usize = 32;

pub(in crate::worker_command) struct LoadedGuiProfile {
    pub config: GuiDriverConfig,
    pub capability: WorkerGuiCapability,
}

#[derive(Debug)]
struct ParsedGuiProfile {
    id: String,
    config: GuiDriverConfig,
    declared_permissions: GuiHostPermissionGrant,
}

pub(in crate::worker_command) async fn load(
    path: &Path,
    command_timeout: Duration,
    removed_environment: BTreeSet<OsString>,
) -> Result<LoadedGuiProfile> {
    let (canonical_profile, source) =
        read_bounded_regular(path, MAX_GUI_PROFILE_BYTES, "GUI host profile").await?;
    let source_text = std::str::from_utf8(&source).context("GUI host profile must be UTF-8")?;
    let mut parsed = parse(
        source_text,
        &canonical_profile,
        command_timeout,
        removed_environment,
    )?;

    let (policy_file, policy) = read_bounded_regular(
        &parsed.config.policy_file,
        MAX_GUI_POLICY_BYTES,
        "GUI policy",
    )
    .await?;
    parsed.config.policy_file = policy_file;
    canonicalize_proxy(&mut parsed.config).await?;
    parsed.config.validate().map_err(anyhow::Error::new)?;

    let driver = GuiDriver::new(parsed.config.clone());
    let compatibility_profile = driver.execution_profile().map_err(anyhow::Error::new)?;
    let probe = driver.probe_host().await.map_err(anyhow::Error::new)?;
    if probe.permissions != parsed.declared_permissions {
        anyhow::bail!(
            "probed GUI host permissions do not exactly match the declared profile grant"
        );
    }

    let endpoint = match parsed.config.endpoint {
        CuaEndpoint::InstalledDaemon { .. } => WorkerGuiEndpoint::InstalledDaemon,
        CuaEndpoint::EmbeddedSocket { .. } => WorkerGuiEndpoint::EmbeddedSocket,
    };
    let perception = match parsed.config.profile {
        GuiProfile::Semantic => WorkerGuiPerception::Semantic,
        GuiProfile::WindowVision => WorkerGuiPerception::WindowVision,
    };
    let (target, application) = worker_target(&parsed.config.target)?;
    let capability = WorkerGuiCapability {
        profile_id: parsed.id,
        compatibility_profile: compatibility_profile.id().to_string(),
        endpoint,
        perception,
        target,
        application,
        cua_driver_version: probe.driver_version,
        mcp_protocol: probe.protocol_version,
        capability_vocabulary: probe.capability_vocabulary,
        tools_schema: probe.tools_schema,
        configuration_digest: sha256(&source),
        policy_digest: sha256(&policy),
        host_permission_digest: probe.permissions.digest(),
        host_permissions: probe.permissions,
    };
    Ok(LoadedGuiProfile {
        config: parsed.config,
        capability,
    })
}

fn parse(
    source: &str,
    config_path: &Path,
    command_timeout: Duration,
    removed_environment: BTreeSet<OsString>,
) -> Result<ParsedGuiProfile> {
    let document = a3s_acl::parse(source).context("invalid GUI host profile ACL")?;
    if document.blocks.len() != 1 || document.blocks[0].name != "gui_host" {
        anyhow::bail!("GUI profile must contain exactly one gui_host block");
    }
    let root = &document.blocks[0];
    if !root.blocks.is_empty() {
        anyhow::bail!("gui_host cannot contain nested blocks");
    }
    ensure_attributes(
        root,
        &[
            "endpoint",
            "proxy_executable",
            "embedded_socket",
            "policy_file",
            "macos_bundle_id",
            "target",
            "attach_pid",
            "arguments",
            "profile",
            "window_title",
            "window_automation_id",
            "permission_source",
            "permissions",
        ],
    )?;
    let id = one_label(root)?.to_string();
    validate_identifier(&id, "gui_host label")?;
    let directory = config_path
        .parent()
        .context("GUI host profile path has no parent directory")?;
    let proxy_executable = resolve_path(
        directory,
        required_string(root, "proxy_executable")?,
        "gui_host.proxy_executable",
    )?;
    let endpoint_name = required_string(root, "endpoint")?;
    let endpoint = match endpoint_name {
        "installed_daemon" => {
            reject_attribute(root, "embedded_socket", "installed_daemon endpoint")?;
            CuaEndpoint::InstalledDaemon { proxy_executable }
        }
        "embedded_socket" => CuaEndpoint::EmbeddedSocket {
            proxy_executable,
            socket: resolve_path(
                directory,
                required_string(root, "embedded_socket")?,
                "gui_host.embedded_socket",
            )?,
        },
        _ => anyhow::bail!("gui_host.endpoint must be installed_daemon or embedded_socket"),
    };
    let declared_source = match required_string(root, "permission_source")? {
        "driver_daemon" => GuiHostPermissionSource::DriverDaemon,
        "host" => GuiHostPermissionSource::Host,
        _ => anyhow::bail!("gui_host.permission_source must be driver_daemon or host"),
    };
    let expected_source = match endpoint {
        CuaEndpoint::InstalledDaemon { .. } => GuiHostPermissionSource::DriverDaemon,
        CuaEndpoint::EmbeddedSocket { .. } => GuiHostPermissionSource::Host,
    };
    if declared_source != expected_source {
        anyhow::bail!("gui_host.permission_source does not match endpoint ownership");
    }
    let permissions = required_string_list(root, "permissions")?;
    if permissions.as_slice() != ["accessibility", "screen_recording"] {
        anyhow::bail!(
            "gui_host.permissions must explicitly list accessibility then screen_recording"
        );
    }
    let declared_permissions = GuiHostPermissionGrant {
        protocol: a3s_test_driver_gui::GUI_HOST_PERMISSION_PROTOCOL.to_string(),
        source: declared_source,
        permissions: vec![
            GuiHostPermission::Accessibility,
            GuiHostPermission::ScreenRecording,
        ],
    };
    declared_permissions
        .validate()
        .map_err(anyhow::Error::new)?;

    let application = ApplicationIdentity::MacOsBundle {
        bundle_id: bounded_string(root, "macos_bundle_id", 256)?,
    };
    let arguments = optional_string_list(root, "arguments")?;
    if arguments.len() > MAX_GUI_ARGUMENTS
        || arguments
            .iter()
            .any(|argument| argument.is_empty() || argument.len() > 1_024)
    {
        anyhow::bail!("gui_host.arguments exceeds its count or value bound");
    }
    let target_name = optional_string(root, "target", "launch")?;
    let target = match target_name {
        "launch" => {
            reject_attribute(root, "attach_pid", "launch target")?;
            GuiAppTarget::Launch(LaunchSpec {
                application,
                arguments: arguments.into_iter().map(OsString::from).collect(),
                working_directory: None,
            })
        }
        "attach" => {
            if !arguments.is_empty() {
                anyhow::bail!("gui_host.arguments is only valid for launch targets");
            }
            let process_id = optional_positive_u32(root, "attach_pid")?.and_then(NonZeroU32::new);
            GuiAppTarget::Attach(AttachSpec {
                application,
                process_id,
            })
        }
        _ => anyhow::bail!("gui_host.target must be launch or attach"),
    };
    let window_title = optional_string_value(root, "window_title")?;
    let window_automation_id = optional_string_value(root, "window_automation_id")?;
    let window = match (window_title, window_automation_id) {
        (Some(_), Some(_)) => {
            anyhow::bail!("gui_host can select a window by title or automation ID, not both")
        }
        (Some(title), None) => WindowSelector::ExactTitle(title),
        (None, Some(automation_id)) => WindowSelector::AutomationId(automation_id),
        (None, None) => WindowSelector::Primary,
    };
    let profile = match optional_string(root, "profile", "semantic")? {
        "semantic" => GuiProfile::Semantic,
        "window_vision" => GuiProfile::WindowVision,
        _ => anyhow::bail!("gui_host.profile must be semantic or window_vision"),
    };
    Ok(ParsedGuiProfile {
        id,
        config: GuiDriverConfig {
            endpoint,
            policy_file: resolve_path(
                directory,
                required_string(root, "policy_file")?,
                "gui_host.policy_file",
            )?,
            target,
            window,
            capture_scope: GuiCaptureScope::Window,
            profile,
            command_timeout,
            removed_environment,
        },
        declared_permissions,
    })
}

fn worker_target(target: &GuiAppTarget) -> Result<(WorkerGuiTarget, WorkerGuiApplication)> {
    let (target, application) = match target {
        GuiAppTarget::Launch(spec) => (WorkerGuiTarget::Launch, &spec.application),
        GuiAppTarget::Attach(spec) => (WorkerGuiTarget::Attach, &spec.application),
    };
    let application = match application {
        ApplicationIdentity::MacOsBundle { bundle_id } => WorkerGuiApplication::MacosBundle {
            bundle_id: bundle_id.clone(),
        },
        ApplicationIdentity::WindowsExecutable {
            path,
            expected_publisher,
        } => WorkerGuiApplication::WindowsExecutable {
            path: path.to_string_lossy().into_owned(),
            expected_publisher: expected_publisher.clone(),
        },
        ApplicationIdentity::LinuxDesktop { desktop_id } => WorkerGuiApplication::LinuxDesktop {
            desktop_id: desktop_id.clone(),
        },
    };
    Ok((target, application))
}

async fn canonicalize_proxy(config: &mut GuiDriverConfig) -> Result<()> {
    let path = match &config.endpoint {
        CuaEndpoint::InstalledDaemon { proxy_executable }
        | CuaEndpoint::EmbeddedSocket {
            proxy_executable, ..
        } => proxy_executable.clone(),
    };
    let canonical = canonical_regular_path(&path, "CUA proxy executable").await?;
    match &mut config.endpoint {
        CuaEndpoint::InstalledDaemon { proxy_executable }
        | CuaEndpoint::EmbeddedSocket {
            proxy_executable, ..
        } => *proxy_executable = canonical,
    }
    Ok(())
}

async fn canonical_regular_path(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    if !metadata.is_file() || is_link_like(&metadata) {
        anyhow::bail!("{label} must be a regular non-link file");
    }
    tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("failed to resolve {label} {}", path.display()))
}

async fn read_bounded_regular(
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<(PathBuf, Vec<u8>)> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    if !metadata.is_file()
        || is_link_like(&metadata)
        || metadata.len() == 0
        || metadata.len() > max_bytes
    {
        anyhow::bail!("{label} must be a bounded regular non-link file");
    }
    let canonical = tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("failed to resolve {label} {}", path.display()))?;
    let bytes = tokio::fs::read(&canonical)
        .await
        .with_context(|| format!("failed to read {label} {}", canonical.display()))?;
    if bytes.len() as u64 != metadata.len() {
        anyhow::bail!("{label} changed while it was being admitted");
    }
    Ok((canonical, bytes))
}

fn resolve_path(directory: &Path, value: &str, label: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.as_os_str().is_empty() {
        anyhow::bail!("{label} must not be empty");
    }
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.join(path)
    })
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn one_label(block: &Block) -> Result<&str> {
    if block.labels.len() != 1 || block.labels[0].is_empty() {
        anyhow::bail!("gui_host requires exactly one non-empty label");
    }
    Ok(&block.labels[0])
}

fn ensure_attributes(block: &Block, allowed: &[&str]) -> Result<()> {
    if let Some(name) = block
        .attributes
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        anyhow::bail!("unsupported gui_host attribute '{name}'");
    }
    Ok(())
}

fn reject_attribute(block: &Block, name: &str, context: &str) -> Result<()> {
    if block.attributes.contains_key(name) {
        anyhow::bail!("gui_host.{name} is not valid for the {context}");
    }
    Ok(())
}

fn required_string<'a>(block: &'a Block, name: &str) -> Result<&'a str> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("gui_host.{name} must be a non-empty string"))
}

fn bounded_string(block: &Block, name: &str, max_bytes: usize) -> Result<String> {
    let value = required_string(block, name)?;
    if value.trim().is_empty() || value.len() > max_bytes {
        anyhow::bail!("gui_host.{name} must be bounded and non-empty");
    }
    Ok(value.to_string())
}

fn optional_string<'a>(block: &'a Block, name: &str, default: &'a str) -> Result<&'a str> {
    match block.attributes.get(name) {
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty())
            .with_context(|| format!("gui_host.{name} must be a non-empty string")),
        None => Ok(default),
    }
}

fn optional_string_value(block: &Block, name: &str) -> Result<Option<String>> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .with_context(|| format!("gui_host.{name} must be a non-empty string"))
        })
        .transpose()
}

fn required_string_list(block: &Block, name: &str) -> Result<Vec<String>> {
    let value = block
        .attributes
        .get(name)
        .with_context(|| format!("gui_host.{name} is required"))?;
    string_list(value, name)
}

fn optional_string_list(block: &Block, name: &str) -> Result<Vec<String>> {
    block
        .attributes
        .get(name)
        .map_or_else(|| Ok(Vec::new()), |value| string_list(value, name))
}

fn string_list(value: &Value, name: &str) -> Result<Vec<String>> {
    let Value::List(values) = value else {
        anyhow::bail!("gui_host.{name} must be a string list");
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .with_context(|| format!("gui_host.{name} must contain non-empty strings"))
        })
        .collect()
}

fn optional_positive_u32(block: &Block, name: &str) -> Result<Option<u32>> {
    block
        .attributes
        .get(name)
        .map(|value| {
            let number = value
                .as_number()
                .with_context(|| format!("gui_host.{name} must be a positive integer"))?;
            if !number.is_finite()
                || number < 1.0
                || number.fract() != 0.0
                || number > f64::from(u32::MAX)
            {
                anyhow::bail!("gui_host.{name} must be a positive 32-bit integer");
            }
            Ok(number as u32)
        })
        .transpose()
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        anyhow::bail!("{label} must be a bounded portable identifier");
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
#[path = "gui_profile/tests.rs"]
mod tests;
