use std::path::{Component, Path, PathBuf};

use a3s_test_core::DriverError;

pub(crate) async fn prepare_root(path: &Path) -> Result<PathBuf, DriverError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                artifact_error(format!("failed to resolve working directory: {error}"))
            })?
            .join(path)
    };
    tokio::fs::create_dir_all(&absolute)
        .await
        .map_err(|error| artifact_error(format!("failed to create artifact root: {error}")))?;
    tokio::fs::canonicalize(&absolute)
        .await
        .map_err(|error| artifact_error(format!("failed to canonicalize artifact root: {error}")))
}

pub(crate) async fn write_recording(
    root: &Path,
    requested: &str,
    bytes: &[u8],
) -> Result<PathBuf, DriverError> {
    let relative = validate_relative(requested)?;
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let mut current = root.to_path_buf();
    for component in parent.components() {
        let Component::Normal(name) = component else {
            if component == Component::CurDir {
                continue;
            }
            return Err(artifact_error(
                "artifact directory contains an invalid component",
            ));
        };
        let candidate = current.join(name);
        match tokio::fs::symlink_metadata(&candidate).await {
            Ok(metadata) if is_link_like(&metadata) || !metadata.is_dir() => {
                return Err(artifact_error(
                    "artifact directory contains a link or non-directory entry",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir(&candidate).await.map_err(|error| {
                    artifact_error(format!("failed to create artifact directory: {error}"))
                })?;
            }
            Err(error) => {
                return Err(artifact_error(format!(
                    "failed to inspect artifact directory: {error}"
                )));
            }
        }
        current = tokio::fs::canonicalize(&candidate).await.map_err(|error| {
            artifact_error(format!("failed to resolve artifact directory: {error}"))
        })?;
        if !current.starts_with(root) {
            return Err(artifact_error("artifact path resolved outside its root"));
        }
    }
    let file_name = relative
        .file_name()
        .ok_or_else(|| artifact_error("artifact path has no file name"))?;
    let path = current.join(file_name);
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if is_link_like(&metadata) || !metadata.is_file() => {
            return Err(artifact_error(
                "existing artifact is a link or non-file entry",
            ));
        }
        Ok(_) => {
            tokio::fs::remove_file(&path).await.map_err(|error| {
                artifact_error(format!("failed to replace existing artifact: {error}"))
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(artifact_error(format!(
                "failed to inspect artifact: {error}"
            )));
        }
    }
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|error| artifact_error(format!("failed to write terminal recording: {error}")))?;
    let metadata = tokio::fs::symlink_metadata(&path).await.map_err(|error| {
        artifact_error(format!("failed to inspect terminal recording: {error}"))
    })?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err(artifact_error(
            "terminal recording is a link or non-regular file",
        ));
    }
    let canonical = tokio::fs::canonicalize(&path).await.map_err(|error| {
        artifact_error(format!("failed to resolve terminal recording: {error}"))
    })?;
    if !canonical.starts_with(root) {
        return Err(artifact_error(
            "terminal recording resolved outside its root",
        ));
    }
    Ok(path)
}

fn validate_relative(requested: &str) -> Result<&Path, DriverError> {
    let path = Path::new(requested);
    if requested.is_empty()
        || requested.len() > 1_024
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(artifact_error(
            "artifact path must be a bounded relative path without parent traversal",
        ));
    }
    Ok(path)
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

fn artifact_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.tui.artifact_invalid", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recording_path_stays_inside_the_artifact_root() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = prepare_root(&temp.path().join("artifacts"))
            .await
            .expect("root");
        let path = write_recording(&root, "terminal/run.cast", b"bytes")
            .await
            .expect("recording");
        assert!(path.starts_with(&root));
        assert_eq!(
            write_recording(&root, "../outside", b"bytes")
                .await
                .expect_err("traversal")
                .code(),
            "test.driver.tui.artifact_invalid"
        );
    }
}
