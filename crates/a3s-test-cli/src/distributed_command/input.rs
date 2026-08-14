use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use a3s_test_core::{Action, SurfaceContractDraft, TestSuite};
use a3s_test_worker::{RemoteInputBundle, RemoteInputFile};
use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::config::DistributedConfig;

const MAX_PREPARED_FILES: usize = 1_024;
const MAX_PREPARED_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PREPARED_TOTAL_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) struct PreparedSuite {
    pub suite: TestSuite,
    pub bundle: RemoteInputBundle,
    pub suite_digest: String,
}

#[derive(Serialize)]
struct BundleDigest<'a> {
    manifest: &'a str,
    files: Vec<BundleDigestFile<'a>>,
}

#[derive(Serialize)]
struct BundleDigestFile<'a> {
    path: &'a str,
    sha256: &'a str,
}

pub(super) async fn prepare_suite(config: &DistributedConfig) -> Result<PreparedSuite> {
    let requested_root = config.config_directory.join(&config.input_root);
    let root_metadata = tokio::fs::symlink_metadata(&requested_root)
        .await
        .with_context(|| format!("failed to inspect input root {}", requested_root.display()))?;
    if !root_metadata.is_dir() || is_link_like(&root_metadata) {
        anyhow::bail!("distributed input_root must be a real directory");
    }
    let root = tokio::fs::canonicalize(&requested_root)
        .await
        .with_context(|| format!("failed to resolve input root {}", requested_root.display()))?;
    let manifest_relative = normalize_relative(&config.manifest, "manifest")?;
    let mut sources = BTreeMap::<String, PathBuf>::new();
    let manifest = insert_relative(&root, &manifest_relative, &mut sources, "manifest").await?;
    let admitted = crate::read_suite(&manifest).await?;
    for path in &config.additional_inputs {
        let relative = normalize_relative(path, "additional input")?;
        insert_relative(&root, &relative, &mut sources, "additional input").await?;
    }
    collect_action_inputs(&root, &manifest_relative, &admitted.suite, &mut sources).await?;

    if sources.len() > MAX_PREPARED_FILES {
        anyhow::bail!("distributed input bundle exceeds {MAX_PREPARED_FILES} files");
    }
    let mut files = Vec::with_capacity(sources.len());
    let mut total_bytes = 0_u64;
    for (remote_path, canonical) in sources {
        let metadata = tokio::fs::metadata(&canonical)
            .await
            .with_context(|| format!("failed to inspect input {}", canonical.display()))?;
        if metadata.len() == 0 || metadata.len() > MAX_PREPARED_FILE_BYTES {
            anyhow::bail!(
                "distributed input '{}' must contain 1 to {MAX_PREPARED_FILE_BYTES} bytes",
                remote_path
            );
        }
        let bytes = tokio::fs::read(&canonical)
            .await
            .with_context(|| format!("failed to read input {}", canonical.display()))?;
        let bytes_len = u64::try_from(bytes.len())
            .with_context(|| format!("distributed input '{remote_path}' size overflowed"))?;
        let current_metadata = tokio::fs::symlink_metadata(&canonical)
            .await
            .with_context(|| format!("failed to re-inspect input {}", canonical.display()))?;
        if is_link_like(&current_metadata)
            || !current_metadata.is_file()
            || bytes_len == 0
            || bytes_len > MAX_PREPARED_FILE_BYTES
            || current_metadata.len() != bytes_len
        {
            anyhow::bail!(
                "distributed input '{remote_path}' changed or exceeded its bound while being read"
            );
        }
        total_bytes = total_bytes.checked_add(bytes_len).with_context(|| {
            format!("distributed input size overflowed while adding '{remote_path}'")
        })?;
        if total_bytes > MAX_PREPARED_TOTAL_BYTES {
            anyhow::bail!(
                "distributed input bundle exceeds {MAX_PREPARED_TOTAL_BYTES} decoded bytes"
            );
        }
        files.push(RemoteInputFile::from_bytes(remote_path, bytes));
    }
    let manifest_path = portable_path(&manifest_relative)?;
    let digest_document = BundleDigest {
        manifest: &manifest_path,
        files: files
            .iter()
            .map(|file| BundleDigestFile {
                path: &file.path,
                sha256: &file.sha256,
            })
            .collect(),
    };
    let digest_bytes = serde_json::to_vec(&digest_document)
        .context("failed to encode distributed suite digest input")?;
    Ok(PreparedSuite {
        suite: admitted.suite,
        bundle: RemoteInputBundle {
            manifest: manifest_path,
            files,
        },
        suite_digest: format!("sha256:{:x}", Sha256::digest(digest_bytes)),
    })
}

async fn collect_action_inputs(
    root: &Path,
    manifest: &Path,
    suite: &TestSuite,
    sources: &mut BTreeMap<String, PathBuf>,
) -> Result<()> {
    let manifest_parent = manifest.parent().unwrap_or_else(|| Path::new(""));
    for action in suite
        .scenarios
        .iter()
        .flat_map(|scenario| &scenario.steps)
        .map(|step| &step.action)
    {
        match action {
            Action::Upload { paths, .. } => {
                for path in paths {
                    let relative = normalize_relative(Path::new(path), "upload input")?;
                    insert_relative(root, &relative, sources, "upload input").await?;
                }
            }
            Action::VerifyContract { contract, .. } => {
                let contract_relative =
                    normalize_relative(&manifest_parent.join(contract), "surface contract input")?;
                let contract_path =
                    insert_relative(root, &contract_relative, sources, "surface contract input")
                        .await?;
                let source = tokio::fs::read_to_string(&contract_path)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to read surface contract {}",
                            contract_path.display()
                        )
                    })?;
                let draft = SurfaceContractDraft::from_acl(&source).with_context(|| {
                    format!("invalid surface contract {}", contract_path.display())
                })?;
                let contract_parent = contract_relative.parent().unwrap_or_else(|| Path::new(""));
                for provenance in draft.provenance() {
                    let relative = normalize_relative(
                        &contract_parent.join(&provenance.uri),
                        "contract provenance input",
                    )?;
                    insert_relative(root, &relative, sources, "contract provenance input").await?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

async fn insert_relative(
    root: &Path,
    relative: &Path,
    sources: &mut BTreeMap<String, PathBuf>,
    label: &str,
) -> Result<PathBuf> {
    let remote = portable_path(relative)?;
    if let Some(existing) = sources.get(&remote) {
        return Ok(existing.clone());
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            anyhow::bail!("{label} must use a normalized relative path");
        };
        current.push(component);
        let metadata = tokio::fs::symlink_metadata(&current)
            .await
            .with_context(|| format!("failed to inspect {label} {}", current.display()))?;
        if is_link_like(&metadata) {
            anyhow::bail!("{label} cannot traverse a link or reparse point");
        }
    }
    let metadata = tokio::fs::metadata(&current)
        .await
        .with_context(|| format!("failed to inspect {label} {}", current.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("{label} must resolve to a regular file");
    }
    let canonical = tokio::fs::canonicalize(&current)
        .await
        .with_context(|| format!("failed to resolve {label} {}", current.display()))?;
    if !canonical.starts_with(root) {
        anyhow::bail!("{label} escaped the distributed input root");
    }
    sources.insert(remote, canonical.clone());
    Ok(canonical)
}

fn normalize_relative(path: &Path, label: &str) -> Result<PathBuf> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        anyhow::bail!("{label} must be a non-empty relative path");
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            _ => anyhow::bail!("{label} cannot contain parent or root components"),
        }
    }
    if normalized.as_os_str().is_empty() {
        anyhow::bail!("{label} must name a file");
    }
    Ok(normalized)
}

fn portable_path(path: &Path) -> Result<String> {
    let components = path
        .components()
        .map(|component| match component {
            Component::Normal(component) => component
                .to_str()
                .filter(|component| !component.is_empty())
                .map(ToOwned::to_owned)
                .context("distributed input path must be portable UTF-8"),
            _ => anyhow::bail!("distributed input path must be normalized and relative"),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(components.join("/"))
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
mod tests {
    use super::prepare_suite;
    use crate::distributed_command::config::parse_config;

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[tokio::test]
    async fn prepares_manifest_uploads_and_explicit_inputs_in_canonical_order() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("tests/fixtures")).expect("fixture directory");
        std::fs::write(root.join("tests/fixtures/avatar.png"), b"avatar").expect("upload");
        std::fs::write(root.join("extra.txt"), b"extra").expect("extra input");
        std::fs::write(
            root.join("tests/suite.acl"),
            r#"suite "distributed" {
  scenario "upload" {
    surface = "web"
    upload "avatar" { target = testid("avatar") paths = ["tests/fixtures/avatar.png"] }
  }
}
"#,
        )
        .expect("suite");
        let config_source = format!(
            r#"distributed_run "ci" {{
  manifest = "tests/suite.acl"
  additional_inputs = ["extra.txt"]
  worker "runner" {{
    endpoint = "https://worker.example.test"
    image_digest = "{DIGEST}"
    authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_RUNNER"
  }}
}}
"#
        );
        let config_path = root.join("distributed.acl");
        let config = parse_config(&config_source, &config_path).expect("config");
        let prepared = prepare_suite(&config).await.expect("prepared suite");
        let paths = prepared
            .bundle
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec!["extra.txt", "tests/fixtures/avatar.png", "tests/suite.acl"]
        );
        assert!(prepared.suite_digest.starts_with("sha256:"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_linked_input_components() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("suite.acl"), b"suite \"x\" {}").expect("outside suite");
        symlink(outside.path(), temp.path().join("linked")).expect("linked directory");
        let source = format!(
            r#"distributed_run "ci" {{
  manifest = "linked/suite.acl"
  worker "runner" {{
    endpoint = "https://worker.example.test"
    image_digest = "{DIGEST}"
    authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_RUNNER"
  }}
}}
"#
        );
        let config = parse_config(&source, &temp.path().join("distributed.acl")).expect("config");
        let error = prepare_suite(&config).await.expect_err("linked input");
        assert!(error.to_string().contains("link"), "{error:#}");
    }
}
