use std::path::Path;

use a3s_test_core::{DriverError, Evidence, RepairDesignReferenceImage, RepairFinding};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::artifact::{prepare_artifact_path, validate_artifact_file};

const MAX_INLINE_DATA_URL_BYTES: usize = 384 * 1_024;

pub(crate) async fn materialize_design_references(
    artifacts_dir: &Path,
    findings: &mut [RepairFinding],
) -> Result<(), DriverError> {
    for finding in findings {
        let Some(reference) = finding.design_reference.as_mut() else {
            continue;
        };
        if reference.width == 0
            || reference.width > 1_600
            || reference.height == 0
            || reference.height > 1_200
            || u64::from(reference.width) * u64::from(reference.height) > 1_920_000
        {
            return Err(invalid_reference(
                "design reference dimensions are outside the admitted bounds",
            ));
        }
        let RepairDesignReferenceImage::Inline {
            media_type,
            data_url,
        } = &reference.image
        else {
            return Err(invalid_reference(
                "browser design references must contain inline image data",
            ));
        };
        validate_finding_id(&finding.id)?;
        let extension = match media_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            _ => {
                return Err(invalid_reference(
                    "design reference media type is unsupported",
                ))
            }
        };
        if data_url.len() > MAX_INLINE_DATA_URL_BYTES {
            return Err(invalid_reference(
                "design reference exceeds the inline image limit",
            ));
        }
        let prefix = format!("data:{media_type};base64,");
        let encoded = data_url.strip_prefix(&prefix).ok_or_else(|| {
            invalid_reference("design reference data URL does not match its media type")
        })?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| invalid_reference("design reference contains invalid base64 data"))?;
        if bytes.is_empty() || bytes.len() > MAX_INLINE_DATA_URL_BYTES {
            return Err(invalid_reference(
                "design reference image is empty or oversized",
            ));
        }
        let (width, height) = image_dimensions(media_type, &bytes)
            .ok_or_else(|| invalid_reference("design reference image header is invalid"))?;
        if width != reference.width || height != reference.height {
            return Err(invalid_reference(
                "design reference dimensions do not match the encoded image",
            ));
        }

        let requested = format!("repairs/{}/design-reference.{extension}", finding.id);
        let path = prepare_artifact_path(artifacts_dir, &requested).await?;
        tokio::fs::write(&path, &bytes).await.map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_reference_write_failed",
                format!("failed to persist design reference: {error}"),
            )
        })?;
        validate_artifact_file(artifacts_dir, &path).await?;
        reference.image = RepairDesignReferenceImage::Artifact {
            evidence: Evidence {
                name: "design-reference".to_string(),
                path: path.display().to_string(),
                media_type: media_type.clone(),
            },
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        };
    }
    Ok(())
}

fn invalid_reference(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.web.repair_reference_invalid", message)
}

fn validate_finding_id(value: &str) -> Result<(), DriverError> {
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | "..")
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(invalid_reference(
            "design reference finding id contains unsupported characters",
        ));
    }
    Ok(())
}

fn image_dimensions(media_type: &str, bytes: &[u8]) -> Option<(u32, u32)> {
    match media_type {
        "image/png" => png_dimensions(bytes),
        "image/jpeg" => jpeg_dimensions(bytes),
        _ => None,
    }
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[..2] != [0xff, 0xd8] {
        return None;
    }
    let mut offset = 2;
    while offset + 4 <= bytes.len() {
        while offset < bytes.len() && bytes[offset] != 0xff {
            offset += 1;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if matches!(marker, 0x01 | 0xd8 | 0xd9) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]));
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        if is_start_of_frame(marker) {
            if length < 7 {
                return None;
            }
            let height = u32::from(u16::from_be_bytes([
                *bytes.get(offset + 3)?,
                *bytes.get(offset + 4)?,
            ]));
            let width = u32::from(u16::from_be_bytes([
                *bytes.get(offset + 5)?,
                *bytes.get(offset + 6)?,
            ]));
            return (width > 0 && height > 0).then_some((width, height));
        }
        offset += length;
    }
    None
}

fn is_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

#[cfg(test)]
mod tests {
    use a3s_test_core::{
        RepairDesignReference, RepairDesignReferenceKind, RepairIntent, RepairSeverity,
        RepairStatus, RepairTarget, RepairTargetKind,
    };
    use serde_json::json;

    use super::*;
    use crate::artifact::prepare_artifact_root;

    const ONE_PIXEL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    fn finding(width: u32, height: u32) -> RepairFinding {
        RepairFinding {
            id: "finding-reference".to_string(),
            batch_id: "batch-1".to_string(),
            instruction: "Match the reference".to_string(),
            success_criteria: None,
            intent: RepairIntent::Change,
            severity: RepairSeverity::Important,
            relations: Vec::new(),
            design_reference: Some(RepairDesignReference {
                kind: RepairDesignReferenceKind::Sketch,
                width,
                height,
                image: RepairDesignReferenceImage::Inline {
                    media_type: "image/png".to_string(),
                    data_url: format!("data:image/png;base64,{ONE_PIXEL_PNG}"),
                },
            }),
            target: RepairTarget {
                kind: RepairTargetKind::Node,
                node_ids: vec!["n1".to_string()],
                selected_text: None,
                region: None,
                drawing: None,
                layout: None,
            },
            created_at: "2026-08-19T00:00:00Z".to_string(),
            page_id: "page".to_string(),
            url: "http://127.0.0.1/".to_string(),
            context_revision: 1,
            context: json!({ "untrusted": true }),
            status: RepairStatus::Queued,
            submitted_at: "2026-08-19T00:00:01Z".to_string(),
        }
    }

    #[tokio::test]
    async fn writes_an_inline_reference_as_a_hashed_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = prepare_artifact_root(&temp.path().join("artifacts"))
            .await
            .expect("artifact root");
        let mut findings = vec![finding(1, 1)];

        materialize_design_references(&root, &mut findings)
            .await
            .expect("materialize design reference");

        let RepairDesignReferenceImage::Artifact { evidence, sha256 } = &findings[0]
            .design_reference
            .as_ref()
            .expect("reference")
            .image
        else {
            panic!("reference was not materialized");
        };
        assert_eq!(evidence.media_type, "image/png");
        assert!(Path::new(&evidence.path).is_file());
        assert_eq!(sha256.len(), 64);
    }

    #[tokio::test]
    async fn rejects_dimension_mismatches_before_writing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = prepare_artifact_root(&temp.path().join("artifacts"))
            .await
            .expect("artifact root");
        let mut findings = vec![finding(960, 600)];

        let error = materialize_design_references(&root, &mut findings)
            .await
            .expect_err("mismatched dimensions must fail");

        assert_eq!(error.code(), "test.driver.web.repair_reference_invalid");
        assert!(!root
            .join("repairs/finding-reference/design-reference.png")
            .exists());
    }

    #[tokio::test]
    async fn rejects_unbounded_dimensions_and_traversal_ids_before_writing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = prepare_artifact_root(&temp.path().join("artifacts"))
            .await
            .expect("artifact root");
        let mut unbounded = finding(1_601, 1);
        let error = materialize_design_references(&root, std::slice::from_mut(&mut unbounded))
            .await
            .expect_err("unbounded dimensions must fail");
        assert_eq!(error.code(), "test.driver.web.repair_reference_invalid");

        let mut traversal = finding(1, 1);
        traversal.id = "..".to_string();
        let error = materialize_design_references(&root, std::slice::from_mut(&mut traversal))
            .await
            .expect_err("traversal finding id must fail");
        assert_eq!(error.code(), "test.driver.web.repair_reference_invalid");
        assert!(!root.join("design-reference.png").exists());
    }

    #[tokio::test]
    async fn rejects_browser_supplied_artifact_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = prepare_artifact_root(&temp.path().join("artifacts"))
            .await
            .expect("artifact root");
        let mut supplied = finding(1, 1);
        supplied.design_reference.as_mut().expect("reference").image =
            RepairDesignReferenceImage::Artifact {
                evidence: Evidence {
                    name: "forged-reference".to_string(),
                    path: "/tmp/untrusted.png".to_string(),
                    media_type: "image/png".to_string(),
                },
                sha256: "0".repeat(64),
            };

        let error = materialize_design_references(&root, std::slice::from_mut(&mut supplied))
            .await
            .expect_err("browser artifact metadata must be rejected");

        assert_eq!(error.code(), "test.driver.web.repair_reference_invalid");
    }
}
