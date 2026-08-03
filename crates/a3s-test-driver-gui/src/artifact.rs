use std::path::{Component, Path, PathBuf};

use a3s_test_core::{DriverError, Evidence};
use sha2::{Digest, Sha256};

use crate::api::CuaWindowState;
use crate::semantic::VisualAddress;

const MAX_GROUNDING_IMAGE_BYTES: u64 = 64 * 1_024 * 1_024;

pub(crate) async fn prepare_artifact_root(path: &Path) -> Result<PathBuf, DriverError> {
    let absolute = absolute_path(path)?;
    tokio::fs::create_dir_all(&absolute)
        .await
        .map_err(|error| {
            DriverError::new(
                "test.driver.gui.artifact_create_failed",
                format!("failed to create GUI artifact directory: {error}"),
            )
        })?;
    tokio::fs::canonicalize(&absolute).await.map_err(|error| {
        DriverError::new(
            "test.driver.gui.artifact_root_invalid",
            format!("failed to canonicalize GUI artifact directory: {error}"),
        )
    })
}

pub(crate) async fn prepare_png_artifact(
    root: &Path,
    requested: &str,
) -> Result<PathBuf, DriverError> {
    let requested_path = resolve_artifact_path(root, requested)?;
    if requested_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("png"))
    {
        return Err(DriverError::new(
            "test.driver.gui.artifact_path_invalid",
            "GUI screenshots must use a .png artifact path",
        ));
    }
    let relative_parent = Path::new(requested).parent().ok_or_else(|| {
        DriverError::new(
            "test.driver.gui.artifact_path_invalid",
            "GUI artifact path has no parent directory",
        )
    })?;
    let canonical_parent = prepare_contained_directory(root, relative_parent).await?;
    let file_name = requested_path.file_name().ok_or_else(|| {
        DriverError::new(
            "test.driver.gui.artifact_path_invalid",
            "GUI artifact path has no file name",
        )
    })?;
    let path = canonical_parent.join(file_name);
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if is_link_like(&metadata) || !metadata.is_file() => {
            return Err(DriverError::new(
                "test.driver.gui.artifact_path_invalid",
                "existing GUI artifact is not a regular file",
            ));
        }
        Ok(_) => tokio::fs::remove_file(&path).await.map_err(|error| {
            DriverError::new(
                "test.driver.gui.artifact_replace_failed",
                format!("failed to replace existing GUI screenshot: {error}"),
            )
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(DriverError::new(
                "test.driver.gui.artifact_inspect_failed",
                format!("failed to inspect existing GUI artifact: {error}"),
            ));
        }
    }
    Ok(path)
}

pub(crate) async fn validate_screenshot(
    root: &Path,
    state: &CuaWindowState,
    path: &Path,
) -> Result<String, DriverError> {
    let reported = state
        .screenshot_file_path
        .as_deref()
        .map(Path::new)
        .filter(|reported| *reported == path);
    if reported.is_none()
        || state.screenshot_width.is_none_or(|width| width == 0)
        || state.screenshot_height.is_none_or(|height| height == 0)
        || state.screenshot_mime_type.as_deref() != Some("image/png")
    {
        return Err(DriverError::new(
            "test.driver.gui.screenshot_invalid",
            "CUA returned incomplete or mismatched window screenshot metadata",
        ));
    }
    image_digest(root, path).await
}

pub(crate) async fn validate_grounding_image(
    root: &Path,
    address: &VisualAddress,
) -> Result<(), DriverError> {
    let path = Path::new(&address.evidence_path);
    let digest = image_digest(root, path).await.map_err(|_| {
        DriverError::new(
            "test.driver.gui.stale_image",
            "grounding image is missing or unreadable",
        )
    })?;
    if digest != address.digest {
        return Err(DriverError::new(
            "test.driver.gui.stale_image",
            "grounding image changed after the visual observation",
        ));
    }
    Ok(())
}

pub(crate) fn image_evidence(name: &str, path: &Path) -> Evidence {
    Evidence {
        name: name.to_string(),
        path: path.display().to_string(),
        media_type: "image/png".to_string(),
    }
}

async fn image_digest(root: &Path, path: &Path) -> Result<String, DriverError> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        DriverError::new(
            "test.driver.gui.screenshot_invalid",
            format!("failed to inspect GUI screenshot: {error}"),
        )
    })?;
    if is_link_like(&metadata)
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_GROUNDING_IMAGE_BYTES
    {
        return Err(DriverError::new(
            "test.driver.gui.screenshot_invalid",
            "GUI screenshot is not a bounded, non-empty regular file",
        ));
    }
    let canonical_path = tokio::fs::canonicalize(path).await.map_err(|error| {
        DriverError::new(
            "test.driver.gui.screenshot_invalid",
            format!("failed to resolve GUI screenshot: {error}"),
        )
    })?;
    ensure_artifact_containment(root, &canonical_path).map_err(|_| {
        DriverError::new(
            "test.driver.gui.screenshot_invalid",
            "GUI screenshot resolved outside the session artifact directory",
        )
    })?;
    let bytes = tokio::fs::read(&canonical_path).await.map_err(|error| {
        DriverError::new(
            "test.driver.gui.screenshot_invalid",
            format!("failed to read GUI screenshot: {error}"),
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn ensure_artifact_containment(root: &Path, resolved: &Path) -> Result<(), DriverError> {
    if !resolved.starts_with(root) {
        return Err(DriverError::new(
            "test.driver.gui.artifact_path_invalid",
            "GUI artifact path resolves outside the session artifact directory",
        ));
    }
    Ok(())
}

async fn prepare_contained_directory(root: &Path, relative: &Path) -> Result<PathBuf, DriverError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            if component == Component::CurDir {
                continue;
            }
            return Err(DriverError::new(
                "test.driver.gui.artifact_path_invalid",
                "GUI artifact directory contains an invalid path component",
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
                            "test.driver.gui.artifact_create_failed",
                            format!("failed to create GUI artifact directory: {error}"),
                        ));
                    }
                }
                tokio::fs::symlink_metadata(&candidate)
                    .await
                    .map_err(|error| {
                        DriverError::new(
                            "test.driver.gui.artifact_inspect_failed",
                            format!("failed to inspect GUI artifact directory: {error}"),
                        )
                    })?
            }
            Err(error) => {
                return Err(DriverError::new(
                    "test.driver.gui.artifact_inspect_failed",
                    format!("failed to inspect GUI artifact directory: {error}"),
                ));
            }
        };
        if is_link_like(&metadata) || !metadata.is_dir() {
            return Err(DriverError::new(
                "test.driver.gui.artifact_path_invalid",
                "GUI artifact directory contains a link or non-directory component",
            ));
        }
        current = tokio::fs::canonicalize(&candidate).await.map_err(|error| {
            DriverError::new(
                "test.driver.gui.artifact_inspect_failed",
                format!("failed to resolve GUI artifact directory: {error}"),
            )
        })?;
        ensure_artifact_containment(root, &current)?;
    }
    Ok(current)
}

fn resolve_artifact_path(root: &Path, requested: &str) -> Result<PathBuf, DriverError> {
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
            "test.driver.gui.artifact_path_invalid",
            "GUI artifact path must be relative and must not contain parent traversal",
        ));
    }
    Ok(root.join(relative))
}

fn absolute_path(path: &Path) -> Result<PathBuf, DriverError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|error| {
            DriverError::new(
                "test.driver.gui.working_directory_failed",
                format!("failed to resolve the current directory: {error}"),
            )
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containment_rejects_a_sibling_with_the_same_text_prefix() {
        let root = Path::new("artifacts/session");
        let sibling = Path::new("artifacts/session-other/image.png");

        let error = ensure_artifact_containment(root, sibling)
            .expect_err("path prefix must not count as component containment");

        assert_eq!(error.code(), "test.driver.gui.artifact_path_invalid");
    }

    #[tokio::test]
    async fn prepares_a_canonical_nested_png_path_inside_the_root() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = prepare_artifact_root(&temp.path().join("artifacts"))
            .await
            .expect("artifact root");

        let prepared = prepare_png_artifact(&root, "screens/nested/window.png")
            .await
            .expect("contained PNG path");

        assert!(prepared.starts_with(&root));
        assert_eq!(
            prepared.file_name().and_then(|name| name.to_str()),
            Some("window.png")
        );
        assert!(prepared.parent().is_some_and(Path::is_dir));
    }
}
