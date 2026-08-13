use std::path::{Component, Path, PathBuf};

use a3s_test_core::DriverError;
use tokio::io::AsyncReadExt;

use crate::path_security::{is_link_like, normalize_canonical_path};

pub(crate) const MAX_GROUNDING_IMAGE_BYTES: u64 = 32 * 1_024 * 1_024;

pub(crate) async fn prepare_artifact_root(path: &Path) -> Result<PathBuf, DriverError> {
    let absolute = absolute_path(path)?;
    tokio::fs::create_dir_all(&absolute)
        .await
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_create_failed",
                format!("failed to create artifact directory: {error}"),
            )
        })?;
    tokio::fs::canonicalize(&absolute)
        .await
        .map(normalize_canonical_path)
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_root_invalid",
                format!("failed to canonicalize artifact directory: {error}"),
            )
        })
}

pub(crate) async fn prepare_artifact_path(
    root: &Path,
    requested: &str,
) -> Result<PathBuf, DriverError> {
    prepare_path(root, requested, true).await
}

pub(crate) async fn admit_artifact_path(
    root: &Path,
    requested: &str,
) -> Result<PathBuf, DriverError> {
    prepare_path(root, requested, false).await
}

async fn prepare_path(
    root: &Path,
    requested: &str,
    replace_existing: bool,
) -> Result<PathBuf, DriverError> {
    let relative = validate_relative_path(requested)?;
    let relative_parent = relative.parent().ok_or_else(|| {
        DriverError::new(
            "test.driver.web.artifact_path_invalid",
            "artifact path has no parent directory",
        )
    })?;
    let canonical_parent = prepare_contained_directory(root, relative_parent).await?;
    let file_name = relative.file_name().ok_or_else(|| {
        DriverError::new(
            "test.driver.web.artifact_path_invalid",
            "artifact path has no file name",
        )
    })?;
    let path = canonical_parent.join(file_name);
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if is_link_like(&metadata) || !metadata.is_file() => {
            return Err(DriverError::new(
                "test.driver.web.artifact_path_invalid",
                "existing artifact is a link or non-file entry",
            ));
        }
        Ok(_) if replace_existing => tokio::fs::remove_file(&path).await.map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_replace_failed",
                format!("failed to replace existing artifact: {error}"),
            )
        })?,
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(DriverError::new(
                "test.driver.web.artifact_inspect_failed",
                format!("failed to inspect existing artifact: {error}"),
            ));
        }
    }
    Ok(path)
}

pub(crate) async fn validate_artifact_file(root: &Path, path: &Path) -> Result<(), DriverError> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        DriverError::new(
            "test.driver.web.artifact_output_invalid",
            format!("browser did not create a readable artifact: {error}"),
        )
    })?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err(DriverError::new(
            "test.driver.web.artifact_output_invalid",
            "browser artifact is a link or non-regular file",
        ));
    }
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map(normalize_canonical_path)
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_output_invalid",
                format!("failed to resolve browser artifact: {error}"),
            )
        })?;
    if !canonical.starts_with(root) {
        return Err(DriverError::new(
            "test.driver.web.artifact_output_invalid",
            "browser artifact resolved outside the session artifact directory",
        ));
    }
    Ok(())
}

pub(crate) async fn read_bounded_artifact(
    root: &Path,
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, DriverError> {
    validate_artifact_file(root, path).await?;
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        DriverError::new(
            "test.driver.web.artifact_output_invalid",
            format!("failed to inspect browser artifact: {error}"),
        )
    })?;
    if metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(DriverError::new(
            "test.driver.web.artifact_output_invalid",
            format!("browser artifact must contain 1 to {max_bytes} bytes"),
        ));
    }
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        DriverError::new(
            "test.driver.web.artifact_output_invalid",
            format!("failed to open browser artifact: {error}"),
        )
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.artifact_output_invalid",
                format!("failed to read browser artifact: {error}"),
            )
        })?;
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        return Err(DriverError::new(
            "test.driver.web.artifact_output_invalid",
            format!("browser artifact must contain 1 to {max_bytes} bytes"),
        ));
    }
    Ok(bytes)
}

async fn prepare_contained_directory(root: &Path, relative: &Path) -> Result<PathBuf, DriverError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            if component == Component::CurDir {
                continue;
            }
            return Err(DriverError::new(
                "test.driver.web.artifact_path_invalid",
                "artifact directory contains an invalid path component",
            ));
        };
        let candidate = current.join(name);
        let metadata = match tokio::fs::symlink_metadata(&candidate).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match tokio::fs::create_dir(&candidate).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(DriverError::new(
                            "test.driver.web.artifact_create_failed",
                            format!("failed to create artifact directory: {error}"),
                        ));
                    }
                }
                tokio::fs::symlink_metadata(&candidate)
                    .await
                    .map_err(|error| {
                        DriverError::new(
                            "test.driver.web.artifact_inspect_failed",
                            format!("failed to inspect artifact directory: {error}"),
                        )
                    })?
            }
            Err(error) => {
                return Err(DriverError::new(
                    "test.driver.web.artifact_inspect_failed",
                    format!("failed to inspect artifact directory: {error}"),
                ));
            }
        };
        if is_link_like(&metadata) || !metadata.is_dir() {
            return Err(DriverError::new(
                "test.driver.web.artifact_path_invalid",
                "artifact directory contains a link or non-directory component",
            ));
        }
        current = tokio::fs::canonicalize(&candidate)
            .await
            .map(normalize_canonical_path)
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.artifact_inspect_failed",
                    format!("failed to resolve artifact directory: {error}"),
                )
            })?;
        if !current.starts_with(root) {
            return Err(DriverError::new(
                "test.driver.web.artifact_path_invalid",
                "artifact path resolves outside the session artifact directory",
            ));
        }
    }
    Ok(current)
}

fn validate_relative_path(requested: &str) -> Result<&Path, DriverError> {
    let relative = Path::new(requested);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(DriverError::new(
            "test.driver.web.artifact_path_invalid",
            "artifact path must be a non-empty relative path without parent traversal",
        ));
    }
    Ok(relative)
}

fn absolute_path(path: &Path) -> Result<PathBuf, DriverError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|error| {
            DriverError::new(
                "test.driver.web.working_directory_failed",
                format!("failed to resolve current directory: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn prepares_a_canonical_nested_artifact_path() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = prepare_artifact_root(&temp.path().join("artifacts"))
            .await
            .expect("artifact root");

        let path = prepare_artifact_path(&root, "screens/nested/page.png")
            .await
            .expect("artifact path");

        assert!(path.starts_with(&root));
        assert!(path.parent().is_some_and(Path::is_dir));
    }

    #[test]
    fn rejects_parent_traversal_before_filesystem_access() {
        let error = validate_relative_path("../outside.png").expect_err("traversal");

        assert_eq!(error.code(), "test.driver.web.artifact_path_invalid");
    }
}
