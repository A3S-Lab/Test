use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use url::Url;

use super::TestKitRequirementArg;

pub(super) const TESTKIT_PACKAGE: &str = "@a3s-lab/testkit";
pub(super) const TESTKIT_INSTALL_SPEC: &str = "@a3s-lab/testkit@0.6.1";
const MAX_PACKAGE_JSON_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    pub(super) fn executable(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
        }
    }

    pub(super) fn run_arguments(self, script: &str) -> Vec<String> {
        match self {
            Self::Npm | Self::Pnpm | Self::Yarn | Self::Bun => {
                vec!["run".to_string(), script.to_string()]
            }
        }
    }

    pub(super) fn install_command(self) -> String {
        match self {
            Self::Npm => format!("npm install --save-dev {TESTKIT_INSTALL_SPEC}"),
            Self::Pnpm => format!("pnpm add --save-dev {TESTKIT_INSTALL_SPEC}"),
            Self::Yarn => format!("yarn add --dev {TESTKIT_INSTALL_SPEC}"),
            Self::Bun => format!("bun add --dev {TESTKIT_INSTALL_SPEC}"),
        }
    }
}

pub(super) fn testkit_install_command(executable: &str) -> String {
    let package_manager = match Path::new(executable)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(executable)
    {
        "pnpm" => PackageManager::Pnpm,
        "yarn" => PackageManager::Yarn,
        "bun" => PackageManager::Bun,
        _ => PackageManager::Npm,
    };
    package_manager.install_command()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Framework {
    Vite,
    Next,
    Rspress,
    ReactScripts,
    Unknown,
}

impl Framework {
    fn default_port(self) -> u16 {
        match self {
            Self::Vite => 5173,
            Self::Next | Self::Rspress | Self::ReactScripts | Self::Unknown => 3000,
        }
    }
}

#[derive(Debug)]
pub(super) struct DiscoveredProject {
    pub(super) id: String,
    pub(super) root: PathBuf,
    pub(super) package_manager: PackageManager,
    pub(super) framework: Framework,
    pub(super) script: String,
    pub(super) executable: String,
    pub(super) arguments: Vec<String>,
    pub(super) url: Url,
    pub(super) testkit_required: bool,
    pub(super) testkit_declared: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageJson {
    name: Option<String>,
    package_manager: Option<String>,
    #[serde(default)]
    scripts: BTreeMap<String, String>,
    #[serde(default)]
    dependencies: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    dev_dependencies: BTreeMap<String, serde_json::Value>,
}

pub(super) async fn discover(
    root: &Path,
    requested_script: Option<&str>,
    requested_url: Option<&str>,
    testkit: TestKitRequirementArg,
) -> Result<DiscoveredProject> {
    let root = tokio::fs::canonicalize(root)
        .await
        .with_context(|| format!("failed to resolve project root {}", root.display()))?;
    let metadata = tokio::fs::metadata(&root)
        .await
        .with_context(|| format!("failed to inspect project root {}", root.display()))?;
    if !metadata.is_dir() {
        anyhow::bail!("project root must be a directory");
    }
    let package_path = root.join("package.json");
    let package_bytes = read_bounded(&package_path, MAX_PACKAGE_JSON_BYTES, "package.json").await?;
    let package: PackageJson = serde_json::from_slice(&package_bytes)
        .with_context(|| format!("invalid package metadata {}", package_path.display()))?;
    let package_manager = detect_package_manager(&root, package.package_manager.as_deref()).await?;
    let framework = detect_framework(&package);
    let script = select_script(&package.scripts, requested_script)?;
    let script_source = package
        .scripts
        .get(&script)
        .context("selected development script disappeared from package metadata")?;
    let url = match requested_url {
        Some(value) => parse_web_url(value, "--url")?,
        None => inferred_url(script_source, framework)?,
    };
    let id = project_id(package.name.as_deref().unwrap_or("web-app"));
    let testkit_declared = package.dependencies.contains_key(TESTKIT_PACKAGE)
        || package.dev_dependencies.contains_key(TESTKIT_PACKAGE);
    Ok(DiscoveredProject {
        id,
        root,
        package_manager,
        framework,
        script: script.clone(),
        executable: package_manager.executable().to_string(),
        arguments: package_manager.run_arguments(&script),
        url,
        testkit_required: testkit == TestKitRequirementArg::Required,
        testkit_declared,
    })
}

async fn detect_package_manager(root: &Path, declared: Option<&str>) -> Result<PackageManager> {
    if let Some(declared) = declared {
        let name = declared.split('@').next().unwrap_or(declared);
        return match name {
            "npm" => Ok(PackageManager::Npm),
            "pnpm" => Ok(PackageManager::Pnpm),
            "yarn" => Ok(PackageManager::Yarn),
            "bun" => Ok(PackageManager::Bun),
            _ => anyhow::bail!("unsupported packageManager '{declared}'"),
        };
    }
    let candidates = [
        ("package-lock.json", PackageManager::Npm),
        ("npm-shrinkwrap.json", PackageManager::Npm),
        ("pnpm-lock.yaml", PackageManager::Pnpm),
        ("yarn.lock", PackageManager::Yarn),
        ("bun.lock", PackageManager::Bun),
        ("bun.lockb", PackageManager::Bun),
    ];
    let mut found = Vec::new();
    for (name, manager) in candidates {
        if tokio::fs::try_exists(root.join(name)).await? && !found.contains(&manager) {
            found.push(manager);
        }
    }
    match found.as_slice() {
        [] => Ok(PackageManager::Npm),
        [manager] => Ok(*manager),
        _ => anyhow::bail!(
            "multiple package-manager lockfiles were found; declare packageManager in package.json"
        ),
    }
}

fn detect_framework(package: &PackageJson) -> Framework {
    let contains = |name: &str| {
        package.dependencies.contains_key(name) || package.dev_dependencies.contains_key(name)
    };
    if contains("@rspress/core") {
        Framework::Rspress
    } else if contains("next") {
        Framework::Next
    } else if contains("vite") {
        Framework::Vite
    } else if contains("react-scripts") {
        Framework::ReactScripts
    } else {
        Framework::Unknown
    }
}

fn select_script(scripts: &BTreeMap<String, String>, requested: Option<&str>) -> Result<String> {
    if let Some(requested) = requested {
        if requested.trim().is_empty() || requested.len() > 128 {
            anyhow::bail!("--script must contain 1-128 characters");
        }
        if !scripts.contains_key(requested) {
            anyhow::bail!("package.json does not define script '{requested}'");
        }
        return Ok(requested.to_string());
    }
    for candidate in ["dev", "start"] {
        if scripts.contains_key(candidate) {
            return Ok(candidate.to_string());
        }
    }
    anyhow::bail!("package.json requires a dev or start script; select another with --script")
}

fn inferred_url(script: &str, framework: Framework) -> Result<Url> {
    let port = script_port(script).unwrap_or_else(|| framework.default_port());
    parse_web_url(&format!("http://127.0.0.1:{port}/"), "inferred project URL")
}

fn script_port(script: &str) -> Option<u16> {
    let parts = script.split_whitespace().collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        if let Some(value) = part.strip_prefix("--port=") {
            return value.parse().ok().filter(|port| *port > 0);
        }
        if matches!(*part, "--port" | "-p") {
            return parts
                .get(index + 1)
                .and_then(|value| value.parse().ok())
                .filter(|port| *port > 0);
        }
    }
    None
}

fn parse_web_url(value: &str, path: &str) -> Result<Url> {
    let parsed = Url::parse(value).with_context(|| format!("{path} is not a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        anyhow::bail!("{path} must be an HTTP or HTTPS URL with a hostname");
    }
    if parsed.username() != "" || parsed.password().is_some() {
        anyhow::bail!("{path} must not contain user information");
    }
    Ok(parsed)
}

fn project_id(value: &str) -> String {
    let mut id = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
            separator = false;
        } else if !id.is_empty() && !separator {
            id.push('-');
            separator = true;
        }
        if id.len() == 64 {
            break;
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    if id.is_empty() {
        "web-app".to_string()
    } else {
        id
    }
}

pub(super) async fn read_package(root: &Path) -> Result<serde_json::Value> {
    let path = root.join("package.json");
    let bytes = read_bounded(&path, MAX_PACKAGE_JSON_BYTES, "package.json").await?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid package metadata {}", path.display()))
}

async fn read_bounded(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        anyhow::bail!("{label} must be a regular non-link file");
    }
    if metadata.len() > max_bytes {
        anyhow::bail!("{label} exceeds the {max_bytes}-byte limit");
    }
    tokio::fs::read(path)
        .await
        .with_context(|| format!("failed to read {label} {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn conflicting_lockfiles_require_an_explicit_package_manager() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("package-lock.json"), "{}\n").expect("npm lock");
        fs::write(temp.path().join("pnpm-lock.yaml"), "lockfileVersion: 9\n").expect("pnpm lock");

        let error = detect_package_manager(temp.path(), None)
            .await
            .expect_err("conflicting lockfiles must fail");

        assert!(error.to_string().contains("multiple package-manager"));
    }

    #[tokio::test]
    async fn package_manager_metadata_wins_over_lockfile_heuristics() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("package-lock.json"), "{}\n").expect("npm lock");
        fs::write(temp.path().join("pnpm-lock.yaml"), "lockfileVersion: 9\n").expect("pnpm lock");

        let manager = detect_package_manager(temp.path(), Some("pnpm@10.14.0"))
            .await
            .expect("declared package manager");

        assert_eq!(manager, PackageManager::Pnpm);
    }

    #[test]
    fn package_managers_install_the_pinned_registry_version() {
        assert_eq!(
            PackageManager::Npm.install_command(),
            "npm install --save-dev @a3s-lab/testkit@0.6.1"
        );
        assert_eq!(
            PackageManager::Pnpm.install_command(),
            "pnpm add --save-dev @a3s-lab/testkit@0.6.1"
        );
        assert_eq!(
            PackageManager::Yarn.install_command(),
            "yarn add --dev @a3s-lab/testkit@0.6.1"
        );
        assert_eq!(
            PackageManager::Bun.install_command(),
            "bun add --dev @a3s-lab/testkit@0.6.1"
        );
        assert_eq!(
            testkit_install_command("/workspace/node_modules/.bin/pnpm"),
            "pnpm add --save-dev @a3s-lab/testkit@0.6.1"
        );
    }

    #[test]
    fn script_ports_support_split_and_equals_forms() {
        assert_eq!(script_port("vite --port 4317"), Some(4317));
        assert_eq!(script_port("next dev --port=4318"), Some(4318));
        assert_eq!(script_port("next dev -p 4319"), Some(4319));
        assert_eq!(script_port("vite --port 0"), None);
        assert_eq!(script_port("vite --port nope"), None);
    }

    #[test]
    fn web_urls_reject_embedded_user_information() {
        let error = parse_web_url("http://user:secret@127.0.0.1:3000", "--url")
            .expect_err("URL credentials must fail");

        assert!(error.to_string().contains("user information"));
    }
}
