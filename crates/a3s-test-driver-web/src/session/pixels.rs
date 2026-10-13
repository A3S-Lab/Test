//! Pixel expectations compare bounded PNG bytes in this process.
//! The browser only receives `screenshot <path>`.

use std::path::Path;

use a3s_test_core::{
    compare_pixels, decode_png_bounded, encode_png, pixel_baseline_decision, render_pixel_diff,
    DriverError, PixelBaselineDecision, PixelCompareFailure, PixelDecodeError, PixelExpectation,
    PixelLimits, StepOutput,
};
use serde_json::json;

use crate::artifact::{
    admit_artifact_path, prepare_artifact_path, read_bounded_artifact, MAX_SCREENSHOT_BYTES,
};

use super::AgentBrowserSession;

impl AgentBrowserSession {
    pub(super) async fn assert_pixels(
        &mut self,
        pixels: &PixelExpectation,
    ) -> Result<StepOutput, DriverError> {
        if self.secret_exposed {
            return Err(DriverError::new(
                "test.driver.web.pixel_after_secret",
                "pixel comparison is refused after a secret was sent to the page",
            ));
        }
        let baseline_path = admit_artifact_path(&self.artifacts_dir, &pixels.baseline).await?;
        let exists = artifact_file_exists(&baseline_path).await?;
        let decision = pixel_baseline_decision(pixels.update, exists, self.pixel_ci);
        if decision == PixelBaselineDecision::Missing {
            return Err(DriverError::new(
                "test.driver.web.pixel_baseline_missing",
                "pixel baseline is missing and update is not set",
            ));
        }

        let index = self.pixel_captures;
        self.execute_command(vec!["eval".into(), PIXEL_HOLD_SCRIPT.into()])
            .await?;
        let actual = self
            .capture_png(&format!("pixel-hold-{index}-a.png"))
            .await?;
        let repeat = self
            .capture_png(&format!("pixel-hold-{index}-b.png"))
            .await?;
        if actual != repeat {
            return Err(DriverError::new(
                "test.driver.web.pixel_unstable",
                "page pixels changed between the two captures",
            ));
        }

        if decision == PixelBaselineDecision::Update {
            let destination = prepare_artifact_path(&self.artifacts_dir, &pixels.baseline).await?;
            tokio::fs::write(&destination, &actual).await.map_err(|_| {
                DriverError::new(
                    "test.driver.web.pixel_decode",
                    "pixel baseline could not be written",
                )
            })?;
            let decoded = decode_capture(&actual)?;
            return Ok(StepOutput::new("pixel baseline updated").with_data(json!({
                "updated": true,
                "width": decoded.width,
                "height": decoded.height,
            })));
        }

        let baseline_bytes =
            read_bounded_artifact(&self.artifacts_dir, &baseline_path, MAX_SCREENSHOT_BYTES)
                .await
                .map_err(|_| {
                    DriverError::new(
                        "test.driver.web.pixel_decode",
                        "pixel baseline could not be read",
                    )
                })?;
        let actual_image = decode_capture(&actual)?;
        let baseline_image = decode_capture(&baseline_bytes)?;
        let limits = PixelLimits {
            threshold_millis: pixels.threshold_millis,
            max_diff_pixels: pixels.max_diff_pixels,
            max_diff_pixel_ratio_micros: pixels.max_diff_pixel_ratio_micros,
            masks: pixels.masks.clone(),
        };
        match compare_pixels(&actual_image, &baseline_image, &limits) {
            Ok(different) => Ok(StepOutput::new("pixels matched").with_data(json!({
                "matched": true,
                "different_pixels": different,
                "width": actual_image.width,
                "height": actual_image.height,
            }))),
            Err(PixelCompareFailure::Size {
                actual_width,
                actual_height,
                baseline_width,
                baseline_height,
            }) => Err(DriverError::new(
                "test.assert.pixel_size",
                format!(
                    "pixel size mismatch: actual {actual_width}x{actual_height}, baseline {baseline_width}x{baseline_height}"
                ),
            )),
            Err(PixelCompareFailure::Diff {
                different_pixels,
                allowed_pixels,
            }) => {
                let actual_name = format!("pixel-mismatch-{index}-actual.png");
                let expected_name = format!("pixel-mismatch-{index}-expected.png");
                let diff_name = format!("pixel-mismatch-{index}-diff.png");
                self.write_png(&actual_name, &actual).await?;
                self.write_png(&expected_name, &baseline_bytes).await?;
                let diff = render_pixel_diff(&actual_image, &baseline_image, &limits).map_err(|_| {
                    DriverError::new(
                        "test.driver.web.pixel_decode",
                        "pixel diff image could not be rendered",
                    )
                })?;
                let bytes = encode_png(&diff).map_err(|_| {
                    DriverError::new(
                        "test.driver.web.pixel_decode",
                        "pixel diff image could not be encoded",
                    )
                })?;
                self.write_png(&diff_name, &bytes).await?;
                Err(DriverError::new(
                    "test.assert.pixel_diff",
                    format!(
                        "pixel difference {different_pixels} exceeds allowed {allowed_pixels}; wrote {actual_name}, {expected_name}, and {diff_name}"
                    ),
                ))
            }
        }
    }

    async fn write_png(&self, name: &str, bytes: &[u8]) -> Result<(), DriverError> {
        let path = prepare_artifact_path(&self.artifacts_dir, name).await?;
        tokio::fs::write(&path, bytes).await.map_err(|_| {
            DriverError::new(
                "test.driver.web.pixel_decode",
                "pixel mismatch image could not be written",
            )
        })
    }

    async fn capture_png(&mut self, name: &str) -> Result<Vec<u8>, DriverError> {
        let path = prepare_artifact_path(&self.artifacts_dir, name).await?;
        self.execute_command(vec!["screenshot".into(), path.as_os_str().to_os_string()])
            .await?;
        self.pixel_captures = self.pixel_captures.saturating_add(1);
        read_bounded_artifact(&self.artifacts_dir, &path, MAX_SCREENSHOT_BYTES).await
    }
}

async fn artifact_file_exists(path: &Path) -> Result<bool, DriverError> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(DriverError::new(
            "test.driver.web.artifact_path_invalid",
            "pixel baseline is a link or non-file entry",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(DriverError::new(
            "test.driver.web.pixel_decode",
            "pixel baseline could not be read",
        )),
    }
}

/// Freeze CSS motion and the caret, then cancel running CSS animations.
const PIXEL_HOLD_SCRIPT: &str = r#"(() => {
  const id = "a3s-pixel-hold";
  let style = document.getElementById(id);
  if (!style) {
    style = document.createElement("style");
    style.id = id;
    (document.head || document.documentElement).appendChild(style);
  }
  style.textContent = "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}";
  const animations = typeof document.getAnimations === "function"
    ? document.getAnimations({ subtree: true })
    : [];
  for (const animation of animations) {
    if (animation && "animationName" in animation) animation.cancel();
  }
  return { held: true };
})()"#;

fn decode_capture(bytes: &[u8]) -> Result<a3s_test_core::DecodedPng, DriverError> {
    decode_png_bounded(bytes).map_err(|error| {
        let message = match error {
            PixelDecodeError::TooLarge => "pixel image exceeds the decode bound",
            PixelDecodeError::Invalid => "pixel image could not be decoded",
        };
        DriverError::new("test.driver.web.pixel_decode", message)
    })
}
