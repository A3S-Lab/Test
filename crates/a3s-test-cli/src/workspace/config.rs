use std::path::{Component, Path, PathBuf};

use a3s_acl::{Block, Value};
use anyhow::{Context, Result};
use serde::Serialize;
use url::Url;

use super::discovery::DiscoveredProject;

pub(super) const PROJECT_PROFILE_VERSION: u32 = 1;
const MAX_CONFIG_BYTES: u64 = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProjectBrowserDriver {
    A3s,
    Standalone,
}

#[derive(Debug)]
pub(super) struct ProjectProfile {
    pub(super) id: String,
    pub(super) config_path: PathBuf,
    pub(super) root: PathBuf,
    pub(super) dev_server: DevServerProfile,
    pub(super) browser: BrowserProfile,
    pub(super) testkit: TestKitProfile,
}

#[derive(Debug)]
pub(super) struct DevServerProfile {
    pub(super) executable: String,
    pub(super) arguments: Vec<String>,
    pub(super) working_directory: PathBuf,
    pub(super) url: Url,
    pub(super) startup_timeout_ms: u64,
    pub(super) cleanup_timeout_ms: u64,
}

#[derive(Debug)]
pub(super) struct BrowserProfile {
    pub(super) driver: ProjectBrowserDriver,
    pub(super) executable: Option<PathBuf>,
    pub(super) session: String,
    pub(super) headed: bool,
    pub(super) command_timeout_ms: u64,
    pub(super) idle_timeout_ms: u64,
}

#[derive(Debug)]
pub(super) struct TestKitProfile {
    pub(super) required: bool,
}

pub(super) async fn resolve_config_path(root: &Path, configured: &Path) -> Result<PathBuf> {
    let root = tokio::fs::canonicalize(root)
        .await
        .with_context(|| format!("failed to resolve project root {}", root.display()))?;
    let requested = if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        root.join(configured)
    };
    let parent = requested
        .parent()
        .context("project profile path must have a parent directory")?;
    let canonical_parent = tokio::fs::canonicalize(parent)
        .await
        .with_context(|| format!("failed to resolve profile directory {}", parent.display()))?;
    if !canonical_parent.starts_with(&root) {
        anyhow::bail!("project profile must stay inside the project root");
    }
    let name = requested
        .file_name()
        .context("project profile path must name a file")?;
    Ok(canonical_parent.join(name))
}

pub(super) async fn load(root: &Path, configured: &Path) -> Result<ProjectProfile> {
    let admitted_root = tokio::fs::canonicalize(root)
        .await
        .with_context(|| format!("failed to resolve project root {}", root.display()))?;
    let config_path = resolve_config_path(&admitted_root, configured).await?;
    let metadata = tokio::fs::symlink_metadata(&config_path)
        .await
        .with_context(|| {
            format!(
                "failed to inspect project profile {}",
                config_path.display()
            )
        })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        anyhow::bail!("project profile must be a regular non-link file");
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        anyhow::bail!("project profile exceeds the {MAX_CONFIG_BYTES}-byte limit");
    }
    let source = tokio::fs::read_to_string(&config_path)
        .await
        .with_context(|| format!("failed to read project profile {}", config_path.display()))?;
    parse(&source, config_path, &admitted_root).await
}

async fn parse(source: &str, config_path: PathBuf, admitted_root: &Path) -> Result<ProjectProfile> {
    let document = a3s_acl::parse(source).context("invalid A3S Test project profile ACL")?;
    if document.blocks.len() != 1 || document.blocks[0].name != "project" {
        anyhow::bail!("project profile must contain exactly one project block");
    }
    let root_block = &document.blocks[0];
    let id = one_label(root_block, "project")?.to_string();
    validate_identifier(&id, "project")?;
    ensure_attributes(root_block, &["version", "root"], "project")?;
    let version = required_u64(root_block, "version", "project")?;
    if version != u64::from(PROJECT_PROFILE_VERSION) {
        anyhow::bail!(
            "unsupported project profile version {version}; expected {PROJECT_PROFILE_VERSION}"
        );
    }
    let config_directory = config_path
        .parent()
        .context("project profile path must have a parent")?;
    let root_value = required_string(root_block, "root", "project")?;
    let root = resolve_root(config_directory, root_value, &config_path, admitted_root).await?;
    let dev_server = parse_dev_server(
        exactly_one_block(root_block, "dev_server", "project")?,
        &root,
    )
    .await?;
    let browser = parse_browser(exactly_one_block(root_block, "browser", "project")?, &root)?;
    let testkit = parse_testkit(exactly_one_block(root_block, "testkit", "project")?)?;
    if let Some(block) = root_block
        .blocks
        .iter()
        .find(|block| !matches!(block.name.as_str(), "dev_server" | "browser" | "testkit"))
    {
        anyhow::bail!("unsupported project block '{}'", block.name);
    }
    Ok(ProjectProfile {
        id,
        config_path,
        root,
        dev_server,
        browser,
        testkit,
    })
}

async fn parse_dev_server(block: &Block, root: &Path) -> Result<DevServerProfile> {
    no_labels_or_blocks(block, "project.dev_server")?;
    ensure_attributes(
        block,
        &[
            "executable",
            "args",
            "working_directory",
            "url",
            "startup_timeout_ms",
            "cleanup_timeout_ms",
        ],
        "project.dev_server",
    )?;
    let executable = required_string(block, "executable", "project.dev_server")?.to_string();
    if executable.len() > 4096 || executable.contains('\0') {
        anyhow::bail!("project.dev_server.executable is too large or invalid");
    }
    let arguments = required_string_list(block, "args", "project.dev_server", 64, 4096)?;
    let working_relative = required_string(block, "working_directory", "project.dev_server")?;
    let working_directory = resolve_contained_directory(root, working_relative).await?;
    let url = parse_web_url(required_string(block, "url", "project.dev_server")?)?;
    let startup_timeout_ms = bounded_timeout(
        optional_u64(block, "startup_timeout_ms", 120_000, "project.dev_server")?,
        "project.dev_server.startup_timeout_ms",
        600_000,
    )?;
    let cleanup_timeout_ms = bounded_timeout(
        optional_u64(block, "cleanup_timeout_ms", 10_000, "project.dev_server")?,
        "project.dev_server.cleanup_timeout_ms",
        60_000,
    )?;
    Ok(DevServerProfile {
        executable,
        arguments,
        working_directory,
        url,
        startup_timeout_ms,
        cleanup_timeout_ms,
    })
}

fn parse_browser(block: &Block, root: &Path) -> Result<BrowserProfile> {
    no_labels_or_blocks(block, "project.browser")?;
    ensure_attributes(
        block,
        &[
            "driver",
            "executable",
            "session",
            "headed",
            "command_timeout_ms",
            "idle_timeout_ms",
        ],
        "project.browser",
    )?;
    let driver = match required_string(block, "driver", "project.browser")? {
        "a3s" => ProjectBrowserDriver::A3s,
        "standalone" => ProjectBrowserDriver::Standalone,
        value => anyhow::bail!("unsupported project.browser.driver '{value}'"),
    };
    let executable = optional_string(block, "executable", "project.browser")?
        .map(|value| resolve_optional_executable(root, &value))
        .transpose()?;
    let session = required_string(block, "session", "project.browser")?.to_string();
    validate_identifier(&session, "project.browser.session")?;
    let headed = optional_bool(block, "headed", true, "project.browser")?;
    let command_timeout_ms = bounded_timeout(
        optional_u64(block, "command_timeout_ms", 25_000, "project.browser")?,
        "project.browser.command_timeout_ms",
        600_000,
    )?;
    let idle_timeout_ms = bounded_timeout(
        optional_u64(block, "idle_timeout_ms", 300_000, "project.browser")?,
        "project.browser.idle_timeout_ms",
        3_600_000,
    )?;
    Ok(BrowserProfile {
        driver,
        executable,
        session,
        headed,
        command_timeout_ms,
        idle_timeout_ms,
    })
}

fn parse_testkit(block: &Block) -> Result<TestKitProfile> {
    no_labels_or_blocks(block, "project.testkit")?;
    ensure_attributes(block, &["required"], "project.testkit")?;
    Ok(TestKitProfile {
        required: optional_bool(block, "required", true, "project.testkit")?,
    })
}

async fn resolve_root(
    config_directory: &Path,
    value: &str,
    config: &Path,
    admitted_root: &Path,
) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() || contains_unsafe_component(path) {
        anyhow::bail!("project.root must be a relative filesystem path");
    }
    let root = tokio::fs::canonicalize(config_directory.join(path))
        .await
        .context("failed to resolve project.root")?;
    if !config.starts_with(&root) {
        anyhow::bail!("project profile must be stored inside project.root");
    }
    if root != admitted_root {
        anyhow::bail!("project.root must match the project root admitted by --root");
    }
    Ok(root)
}

async fn resolve_contained_directory(root: &Path, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        anyhow::bail!("project.dev_server.working_directory must stay inside project.root");
    }
    let directory = tokio::fs::canonicalize(root.join(path))
        .await
        .context("failed to resolve project.dev_server.working_directory")?;
    if !directory.starts_with(root) || !directory.is_dir() {
        anyhow::bail!("project.dev_server.working_directory must be a contained directory");
    }
    Ok(directory)
}

fn resolve_optional_executable(root: &Path, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.trim().is_empty() || value.len() > 4096 || value.contains('\0') {
        anyhow::bail!("project.browser.executable must be a bounded non-empty path");
    }
    if path.is_absolute() || path.components().count() == 1 {
        Ok(path.to_path_buf())
    } else if path
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
    {
        Ok(root.join(path))
    } else {
        anyhow::bail!("project.browser.executable must not escape project.root")
    }
}

fn contains_unsafe_component(path: &Path) -> bool {
    path.components().any(|component| {
        !matches!(
            component,
            Component::Normal(_) | Component::CurDir | Component::ParentDir
        )
    })
}

fn parse_web_url(value: &str) -> Result<Url> {
    let parsed = Url::parse(value).context("project.dev_server.url is invalid")?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        anyhow::bail!("project.dev_server.url must use HTTP or HTTPS and contain a hostname");
    }
    if parsed.username() != "" || parsed.password().is_some() {
        anyhow::bail!("project.dev_server.url must not contain user information");
    }
    Ok(parsed)
}

pub(super) fn render(discovered: &DiscoveredProject, root_reference: &str) -> String {
    let arguments = discovered
        .arguments
        .iter()
        .map(|value| format!("\"{}\"", escape(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "project \"{}\" {{\n  version = {}\n  root = \"{}\"\n\n  dev_server {{\n    executable = \"{}\"\n    args = [{}]\n    working_directory = \".\"\n    url = \"{}\"\n    startup_timeout_ms = 120000\n    cleanup_timeout_ms = 10000\n  }}\n\n  browser {{\n    driver = \"a3s\"\n    session = \"dev\"\n    headed = true\n    command_timeout_ms = 25000\n    idle_timeout_ms = 300000\n  }}\n\n  testkit {{\n    required = {}\n  }}\n}}\n",
        escape(&discovered.id),
        PROJECT_PROFILE_VERSION,
        escape(root_reference),
        escape(&discovered.executable),
        arguments,
        discovered.url,
        discovered.testkit_required,
    )
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn one_label<'a>(block: &'a Block, path: &str) -> Result<&'a str> {
    match block.labels.as_slice() {
        [label] if !label.trim().is_empty() => Ok(label),
        _ => anyhow::bail!("{path} requires exactly one non-empty label"),
    }
}

fn exactly_one_block<'a>(parent: &'a Block, name: &str, path: &str) -> Result<&'a Block> {
    let matches = parent
        .blocks
        .iter()
        .filter(|block| block.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [block] => Ok(*block),
        _ => anyhow::bail!("{path} requires exactly one {name} block"),
    }
}

fn no_labels_or_blocks(block: &Block, path: &str) -> Result<()> {
    if !block.labels.is_empty() || !block.blocks.is_empty() {
        anyhow::bail!("{path} does not accept labels or nested blocks");
    }
    Ok(())
}

fn ensure_attributes(block: &Block, allowed: &[&str], path: &str) -> Result<()> {
    if let Some(name) = block
        .attributes
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        anyhow::bail!("unsupported {path} attribute '{name}'");
    }
    Ok(())
}

fn required_string<'a>(block: &'a Block, name: &str, path: &str) -> Result<&'a str> {
    block
        .attributes
        .get(name)
        .with_context(|| format!("{path}.{name} is required"))?
        .as_str()
        .filter(|value| !value.trim().is_empty() && value.len() <= 64 * 1024)
        .with_context(|| format!("{path}.{name} must be a bounded non-empty string"))
}

fn optional_string(block: &Block, name: &str, path: &str) -> Result<Option<String>> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty() && value.len() <= 64 * 1024)
                .map(str::to_string)
                .with_context(|| format!("{path}.{name} must be a bounded non-empty string"))
        })
        .transpose()
}

fn required_string_list(
    block: &Block,
    name: &str,
    path: &str,
    max_items: usize,
    max_item_bytes: usize,
) -> Result<Vec<String>> {
    let value = block
        .attributes
        .get(name)
        .with_context(|| format!("{path}.{name} is required"))?;
    let Value::List(values) = value else {
        anyhow::bail!("{path}.{name} must be a string list");
    };
    if values.len() > max_items {
        anyhow::bail!("{path}.{name} must contain no more than {max_items} entries");
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.contains('\0') && value.len() <= max_item_bytes)
                .map(str::to_string)
                .with_context(|| format!("{path}.{name} contains an invalid string"))
        })
        .collect()
}

fn required_u64(block: &Block, name: &str, path: &str) -> Result<u64> {
    let value = block
        .attributes
        .get(name)
        .with_context(|| format!("{path}.{name} is required"))?;
    u64_value(value, name, path)
}

fn optional_u64(block: &Block, name: &str, default: u64, path: &str) -> Result<u64> {
    block
        .attributes
        .get(name)
        .map(|value| u64_value(value, name, path))
        .unwrap_or(Ok(default))
}

fn optional_bool(block: &Block, name: &str, default: bool, path: &str) -> Result<bool> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_bool()
                .with_context(|| format!("{path}.{name} must be a boolean"))
        })
        .unwrap_or(Ok(default))
}

fn u64_value(value: &Value, name: &str, path: &str) -> Result<u64> {
    let number = value
        .as_number()
        .with_context(|| format!("{path}.{name} must be an integer"))?;
    if !number.is_finite() || number.fract() != 0.0 || number < 0.0 || number > u64::MAX as f64 {
        anyhow::bail!("{path}.{name} is outside the supported integer range");
    }
    Ok(number as u64)
}

fn bounded_timeout(value: u64, path: &str, maximum: u64) -> Result<u64> {
    if value == 0 || value > maximum {
        anyhow::bail!("{path} must be between 1 and {maximum}");
    }
    Ok(value)
}

fn validate_identifier(value: &str, path: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("{path} identifier must contain 1-64 ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
