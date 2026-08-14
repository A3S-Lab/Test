use std::path::Path;

use tokio::fs;

use crate::remote::RemoteWorkerError;

#[cfg(windows)]
pub(super) fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(super) fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
pub(super) async fn sync_directory(path: &Path) -> Result<(), RemoteWorkerError> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(io_error)
    })
    .await
    .map_err(|error| {
        RemoteWorkerError::new(
            "test.worker.artifact.sync_failed",
            format!("artifact directory sync task failed: {error}"),
            true,
        )
    })?
}

#[cfg(not(unix))]
pub(super) async fn sync_directory(_path: &Path) -> Result<(), RemoteWorkerError> {
    Ok(())
}

pub(super) fn io_error(error: std::io::Error) -> RemoteWorkerError {
    RemoteWorkerError::new(
        "test.worker.artifact.io_failed",
        format!("artifact persistence I/O failed: {error}"),
        true,
    )
}

pub(super) fn artifact_error(code: &'static str, message: impl Into<String>) -> RemoteWorkerError {
    RemoteWorkerError::new(code, message, false)
}

pub(super) fn validate_relative_path(path: &str) -> Result<(), RemoteWorkerError> {
    let valid = !path.is_empty()
        && path.len() <= 512
        && !path.starts_with('/')
        && !path.contains('\\')
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && component.len() <= 128
                && !component.chars().any(char::is_control)
        });
    if !valid {
        return Err(artifact_error(
            "test.worker.artifact.path_invalid",
            "artifact path must be a bounded portable relative path",
        ));
    }
    Ok(())
}

pub(super) async fn remove_entry(path: &Path, directory: bool) -> Result<(), RemoteWorkerError> {
    let metadata = match fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(error)),
    };
    if is_link_like(&metadata)
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(artifact_error(
            "test.worker.artifact.payload_invalid",
            "retained payload entry is a link or has the wrong file type",
        ));
    }
    if directory {
        fs::remove_dir_all(path).await.map_err(io_error)
    } else {
        fs::remove_file(path).await.map_err(io_error)
    }
}
