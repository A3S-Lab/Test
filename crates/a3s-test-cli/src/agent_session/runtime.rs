use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const RUNTIME_OWNER_FILE: &str = ".a3s-test-owner";

pub(super) fn session_namespace(workspace: &Path, session: &str) -> String {
    let mut hasher = DefaultHasher::new();
    workspace.hash(&mut hasher);
    session.hash(&mut hasher);
    format!("a3st-{:016x}", hasher.finish())
}

pub(super) async fn create_runtime_directory(workspace: &Path, session: &str) -> Result<PathBuf> {
    let base = runtime_base();
    for attempt in 0..32_u64 {
        let mut hasher = DefaultHasher::new();
        workspace.hash(&mut hasher);
        session.hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        super::unix_ms().hash(&mut hasher);
        attempt.hash(&mut hasher);
        let path = base.join(format!("a3st-i-{:016x}", hasher.finish()));
        match tokio::fs::create_dir(&path).await {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                        .await
                        .with_context(|| {
                            format!("failed to secure runtime directory {}", path.display())
                        })?;
                }
                if let Err(error) = tokio::fs::write(
                    path.join(RUNTIME_OWNER_FILE),
                    runtime_owner(workspace, session),
                )
                .await
                {
                    let _ = tokio::fs::remove_dir_all(&path).await;
                    return Err(error).with_context(|| {
                        format!("failed to mark runtime directory {}", path.display())
                    });
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create runtime directory {}", path.display())
                });
            }
        }
    }
    anyhow::bail!("failed to allocate a unique browser runtime directory")
}

pub(super) async fn validate_runtime_directory(
    path: &Path,
    workspace: &Path,
    session: &str,
    must_exist: bool,
) -> Result<()> {
    validate_runtime_path(path)?;
    if !tokio::fs::try_exists(path)
        .await
        .with_context(|| format!("failed to inspect runtime directory {}", path.display()))?
    {
        if must_exist {
            anyhow::bail!(
                "active agent session runtime is missing: {}",
                path.display()
            );
        }
        return Ok(());
    }

    let owner = tokio::fs::read_to_string(path.join(RUNTIME_OWNER_FILE))
        .await
        .with_context(|| format!("runtime ownership marker is missing for {}", path.display()))?;
    if owner != runtime_owner(workspace, session) {
        anyhow::bail!(
            "runtime ownership marker does not match agent session '{}'",
            session
        );
    }
    Ok(())
}

pub(super) async fn remove_runtime_directory(
    path: &Path,
    workspace: &Path,
    session: &str,
) -> Result<()> {
    validate_runtime_directory(path, workspace, session, false).await?;
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to remove runtime directory {}", path.display())),
    }
}

fn validate_runtime_path(path: &Path) -> Result<()> {
    let expected_parent = runtime_base();
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        anyhow::bail!("agent runtime path has no valid directory name");
    };
    let Some(suffix) = name.strip_prefix("a3st-i-") else {
        anyhow::bail!("agent runtime directory is outside the owned naming scheme");
    };
    if path.parent() != Some(expected_parent.as_path())
        || suffix.len() != 16
        || !suffix
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        anyhow::bail!("agent runtime directory is outside the owned runtime root");
    }
    Ok(())
}

fn runtime_owner(workspace: &Path, session: &str) -> String {
    format!(
        "a3s-test-agent-runtime-v1\n{}\nagent-{session}\n",
        session_namespace(workspace, session)
    )
}

#[cfg(unix)]
fn runtime_base() -> PathBuf {
    PathBuf::from("/tmp")
}

#[cfg(not(unix))]
fn runtime_base() -> PathBuf {
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_marker_binds_directory_to_workspace_and_session() {
        let workspace = tempfile::tempdir().expect("workspace");
        let runtime = create_runtime_directory(workspace.path(), "checkout")
            .await
            .expect("create runtime");

        validate_runtime_directory(&runtime, workspace.path(), "checkout", true)
            .await
            .expect("validate owner");
        assert!(
            validate_runtime_directory(&runtime, workspace.path(), "other", true)
                .await
                .is_err()
        );

        remove_runtime_directory(&runtime, workspace.path(), "checkout")
            .await
            .expect("remove runtime");
        assert!(!runtime.exists());
    }

    #[tokio::test]
    async fn runtime_path_must_use_the_private_naming_scheme() {
        let workspace = tempfile::tempdir().expect("workspace");
        assert!(
            validate_runtime_directory(workspace.path(), workspace.path(), "checkout", false)
                .await
                .is_err()
        );
    }
}
