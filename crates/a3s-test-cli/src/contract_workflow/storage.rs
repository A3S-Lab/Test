use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};

pub(super) const MAX_WORKFLOW_BYTES: u64 = 32 * 1024 * 1024;
pub(super) const MAX_REVIEW_BYTES: u64 = 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) async fn canonical_regular_file(path: &Path, description: &str) -> Result<PathBuf> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect {description} {}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        anyhow::bail!("{description} must be a regular non-symbolic-link file");
    }
    tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("failed to resolve {description} {}", path.display()))
}

pub(super) async fn read_bounded(
    path: &Path,
    max_bytes: u64,
    description: &str,
) -> Result<Vec<u8>> {
    let metadata = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("failed to inspect {description} {}", path.display()))?;
    if metadata.len() == 0 || metadata.len() > max_bytes {
        anyhow::bail!("{description} must contain 1 to {max_bytes} bytes");
    }
    tokio::fs::read(path)
        .await
        .with_context(|| format!("failed to read {description} {}", path.display()))
}

pub(super) async fn write_atomic(
    path: &Path,
    bytes: &[u8],
    force: bool,
    description: &str,
) -> Result<()> {
    ensure_output_target(path, force, description)?;
    let temporary = write_temporary(path, bytes, description).await?;
    publish_temporary(&temporary, path, force, description).await
}

pub(super) async fn publish_review_outputs(
    contract_path: &Path,
    contract_bytes: &[u8],
    audit_path: &Path,
    audit_bytes: &[u8],
    force: bool,
) -> Result<()> {
    ensure_output_target(contract_path, force, "reviewed contract")?;
    ensure_output_target(audit_path, force, "reviewed contract audit")?;
    let contract_temp = write_temporary(contract_path, contract_bytes, "reviewed contract").await?;
    let audit_temp = match write_temporary(audit_path, audit_bytes, "reviewed contract audit").await
    {
        Ok(path) => path,
        Err(error) => {
            let _ = tokio::fs::remove_file(&contract_temp).await;
            return Err(error);
        }
    };

    let contract_backup = backup_existing(contract_path, force, "reviewed contract").await?;
    let audit_backup = match backup_existing(audit_path, force, "reviewed contract audit").await {
        Ok(path) => path,
        Err(error) => {
            restore_backup(contract_backup.as_deref(), contract_path).await?;
            remove_files([&contract_temp, &audit_temp]).await;
            return Err(error);
        }
    };
    if let Err(error) = rename_temporary(&audit_temp, audit_path, "reviewed contract audit").await {
        restore_backup(audit_backup.as_deref(), audit_path).await?;
        restore_backup(contract_backup.as_deref(), contract_path).await?;
        remove_files([&contract_temp, &audit_temp]).await;
        return Err(error);
    }
    if let Err(error) = rename_temporary(&contract_temp, contract_path, "reviewed contract").await {
        let _ = tokio::fs::remove_file(audit_path).await;
        restore_backup(audit_backup.as_deref(), audit_path).await?;
        restore_backup(contract_backup.as_deref(), contract_path).await?;
        remove_files([&contract_temp]).await;
        return Err(error);
    }
    remove_optional_files([contract_backup.as_deref(), audit_backup.as_deref()]).await;
    Ok(())
}

pub(super) fn ensure_distinct_outputs(contract: &Path, audit: &Path) -> Result<()> {
    let contract = absolute_lexical(contract)?;
    let audit = absolute_lexical(audit)?;
    if contract == audit {
        anyhow::bail!("reviewed contract and audit outputs must be different files");
    }
    Ok(())
}

async fn write_temporary(path: &Path, bytes: &[u8], description: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    let temporary = sibling_path(path, "tmp")?;
    tokio::fs::write(&temporary, bytes)
        .await
        .with_context(|| format!("failed to write temporary {description}"))?;
    Ok(temporary)
}

async fn publish_temporary(
    temporary: &Path,
    path: &Path,
    _force: bool,
    description: &str,
) -> Result<()> {
    #[cfg(windows)]
    if _force && path.exists() {
        tokio::fs::remove_file(path)
            .await
            .with_context(|| format!("failed to replace {description} {}", path.display()))?;
    }
    if let Err(error) = rename_temporary(temporary, path, description).await {
        let _ = tokio::fs::remove_file(temporary).await;
        return Err(error);
    }
    Ok(())
}

async fn backup_existing(path: &Path, force: bool, description: &str) -> Result<Option<PathBuf>> {
    if !force || !path.exists() {
        return Ok(None);
    }
    let backup = sibling_path(path, "backup")?;
    tokio::fs::rename(path, &backup)
        .await
        .with_context(|| format!("failed to stage existing {description} {}", path.display()))?;
    Ok(Some(backup))
}

async fn rename_temporary(temporary: &Path, path: &Path, description: &str) -> Result<()> {
    tokio::fs::rename(temporary, path)
        .await
        .with_context(|| format!("failed to publish {description} {}", path.display()))
}

async fn restore_backup(backup: Option<&Path>, destination: &Path) -> Result<()> {
    if let Some(backup) = backup {
        tokio::fs::rename(backup, destination)
            .await
            .with_context(|| {
                format!(
                    "failed to restore output backup {} to {}",
                    backup.display(),
                    destination.display()
                )
            })?;
    }
    Ok(())
}

async fn remove_files<const N: usize>(paths: [&Path; N]) {
    for path in paths {
        let _ = tokio::fs::remove_file(path).await;
    }
}

async fn remove_optional_files<const N: usize>(paths: [Option<&Path>; N]) {
    for path in paths.into_iter().flatten() {
        let _ = tokio::fs::remove_file(path).await;
    }
}

fn sibling_path(path: &Path, suffix: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .context("output path must have a file name")?
        .to_string_lossy();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{name}.{}.{}.{suffix}",
        std::process::id(),
        sequence
    )))
}

pub(super) fn ensure_output_target(path: &Path, force: bool, description: &str) -> Result<()> {
    if path.file_name().is_none() {
        anyhow::bail!("{description} output must have a file name");
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("{description} output must not be a symbolic link");
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            anyhow::bail!("{description} output must be a regular file");
        }
        Ok(_) if !force => {
            anyhow::bail!("{description} output already exists; pass --force to replace it");
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to inspect {description} output {}", path.display())
            });
        }
    }
    Ok(())
}

fn absolute_lexical(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(path))
    }
}
