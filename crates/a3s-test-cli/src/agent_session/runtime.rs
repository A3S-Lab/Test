use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

const RUNTIME_OWNER_FILE: &str = ".a3s-test-owner";
const DRIVER_SESSION_MAX_BYTES: usize = 28;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

pub(super) fn session_namespace(workspace: &Path, session: &str) -> String {
    let mut hasher = DefaultHasher::new();
    workspace.hash(&mut hasher);
    session.hash(&mut hasher);
    format!("a3st-{:016x}", hasher.finish())
}

pub(super) fn driver_session_id(session: &str) -> String {
    let requested = format!("agent-{session}");
    if requested.len() <= DRIVER_SESSION_MAX_BYTES {
        return requested;
    }

    let readable_bytes = DRIVER_SESSION_MAX_BYTES - 17;
    let digest = Sha256::digest(requested.as_bytes());
    let mut suffix = String::with_capacity(16);
    for byte in &digest[..8] {
        suffix.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        suffix.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    format!("{}-{suffix}", &requested[..readable_bytes])
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
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if must_exist {
                anyhow::bail!(
                    "active agent session runtime is missing: {}",
                    path.display()
                );
            }
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to inspect runtime directory {}", path.display())
            });
        }
    };
    if is_link_like(&metadata) || !metadata.is_dir() {
        anyhow::bail!("agent runtime path is a link or non-directory entry");
    }

    let owner_path = path.join(RUNTIME_OWNER_FILE);
    let owner_metadata = tokio::fs::symlink_metadata(&owner_path)
        .await
        .with_context(|| format!("runtime ownership marker is missing for {}", path.display()))?;
    if is_link_like(&owner_metadata) || !owner_metadata.is_file() {
        anyhow::bail!("runtime ownership marker is a link or non-file entry");
    }
    let owner = tokio::fs::read_to_string(&owner_path)
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

fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn runtime_owner(workspace: &Path, session: &str) -> String {
    format!(
        "a3s-test-agent-runtime-v1\n{}\n{}\n",
        session_namespace(workspace, session),
        driver_session_id(session),
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

    #[cfg(unix)]
    fn symlink_directory(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_directory(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(unix)]
    fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    fn unavailable_without_host_privilege(error: &std::io::Error) -> bool {
        cfg!(windows)
            && matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
            )
    }

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

    #[tokio::test]
    async fn runtime_directory_must_not_be_a_link() {
        let workspace = tempfile::tempdir().expect("workspace");
        let runtime = create_runtime_directory(workspace.path(), "checkout")
            .await
            .expect("create runtime");
        let outside = runtime.with_file_name(format!(
            "{}-outside",
            runtime.file_name().unwrap().to_string_lossy()
        ));
        tokio::fs::create_dir(&outside)
            .await
            .expect("outside directory");
        tokio::fs::remove_dir_all(&runtime)
            .await
            .expect("remove runtime");
        if let Err(error) = symlink_directory(&outside, &runtime) {
            tokio::fs::remove_dir_all(&outside)
                .await
                .expect("remove outside directory");
            if unavailable_without_host_privilege(&error) {
                return;
            }
            panic!("failed to create runtime link: {error}");
        }

        assert!(
            validate_runtime_directory(&runtime, workspace.path(), "checkout", true)
                .await
                .is_err()
        );

        #[cfg(unix)]
        tokio::fs::remove_file(&runtime)
            .await
            .expect("remove runtime link");
        #[cfg(windows)]
        tokio::fs::remove_dir(&runtime)
            .await
            .expect("remove runtime link");
        tokio::fs::remove_dir_all(&outside)
            .await
            .expect("remove outside directory");
    }

    #[tokio::test]
    async fn runtime_owner_marker_must_not_be_a_link() {
        let workspace = tempfile::tempdir().expect("workspace");
        let runtime = create_runtime_directory(workspace.path(), "checkout")
            .await
            .expect("create runtime");
        let marker = runtime.join(RUNTIME_OWNER_FILE);
        let target = runtime.join("copied-owner");
        tokio::fs::write(&target, runtime_owner(workspace.path(), "checkout"))
            .await
            .expect("write copied marker");
        tokio::fs::remove_file(&marker)
            .await
            .expect("remove owner marker");
        if let Err(error) = symlink_file(&target, &marker) {
            tokio::fs::remove_dir_all(&runtime)
                .await
                .expect("remove runtime");
            if unavailable_without_host_privilege(&error) {
                return;
            }
            panic!("failed to create owner link: {error}");
        }

        assert!(
            validate_runtime_directory(&runtime, workspace.path(), "checkout", true)
                .await
                .is_err()
        );

        tokio::fs::remove_dir_all(&runtime)
            .await
            .expect("remove runtime");
    }

    #[test]
    fn driver_session_ids_are_stable_and_socket_safe() {
        assert_eq!(driver_session_id("checkout"), "agent-checkout");

        let session = "office-presentation-font-v041";
        let compact = driver_session_id(session);
        assert_eq!(compact, driver_session_id(session));
        assert!(compact.starts_with("agent-offic-"), "{compact}");
        assert_eq!(compact.len(), DRIVER_SESSION_MAX_BYTES);
        assert_ne!(compact, driver_session_id("office-presentation-font-v042"));
    }
}
