//! Bounded PNG decode and pixel comparison.
//!
//! The distance follows pixelmatch's YIQ formula. A size mismatch returns
//! before any pixel is compared. Masks are painted on both images first.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_PIXEL_DIMENSION: u32 = 4_096;
pub const MAX_PNG_BYTES: usize = 32 * 1_024 * 1_024;

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const MASK_COLOR: [u8; 4] = [255, 0, 255, 255];

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelMask {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedPng {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelDecodeError {
    Invalid,
    TooLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelBaselineDecision {
    Compare,
    Update,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelCompareFailure {
    Size {
        actual_width: u32,
        actual_height: u32,
        baseline_width: u32,
        baseline_height: u32,
    },
    Diff {
        different_pixels: u64,
        allowed_pixels: u64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PixelLimits {
    pub threshold_millis: u16,
    pub max_diff_pixels: Option<u32>,
    pub max_diff_pixel_ratio_micros: Option<u32>,
    pub masks: Vec<PixelMask>,
}

/// `update` is the only writer. A missing baseline is never created without it,
/// including when `ci` is set.
#[must_use]
pub fn pixel_baseline_decision(update: bool, exists: bool, _ci: bool) -> PixelBaselineDecision {
    if update {
        PixelBaselineDecision::Update
    } else if exists {
        PixelBaselineDecision::Compare
    } else {
        PixelBaselineDecision::Missing
    }
}

/// Read width and height from IHDR, reject unbounded images, then decode.
pub fn decode_png_bounded(bytes: &[u8]) -> Result<DecodedPng, PixelDecodeError> {
    if bytes.len() > MAX_PNG_BYTES {
        return Err(PixelDecodeError::TooLarge);
    }
    let (width, height) = png_dimensions(bytes)?;
    if width == 0
        || height == 0
        || width > MAX_PIXEL_DIMENSION
        || height > MAX_PIXEL_DIMENSION
        || u64::from(width) * u64::from(height)
            > u64::from(MAX_PIXEL_DIMENSION) * u64::from(MAX_PIXEL_DIMENSION)
    {
        return Err(PixelDecodeError::TooLarge);
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| PixelDecodeError::Invalid)?;
    let buffer_size = reader.output_buffer_size();
    if buffer_size > MAX_PNG_BYTES {
        return Err(PixelDecodeError::TooLarge);
    }
    let mut buffer = vec![0_u8; buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|_| PixelDecodeError::Invalid)?;
    let frame = &buffer[..info.buffer_size()];
    let rgba = to_rgba(frame, info.color_type, width, height)?;
    Ok(DecodedPng {
        width,
        height,
        rgba,
    })
}

pub fn paint_pixel_masks(image: &mut DecodedPng, masks: &[PixelMask]) {
    let Ok(width) = usize::try_from(image.width) else {
        return;
    };
    for mask in masks {
        let x0 = mask.x.min(image.width);
        let y0 = mask.y.min(image.height);
        let x1 = mask.x.saturating_add(mask.width).min(image.width);
        let y1 = mask.y.saturating_add(mask.height).min(image.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let Ok(y) = usize::try_from(y) else {
                    continue;
                };
                let Ok(x) = usize::try_from(x) else {
                    continue;
                };
                let offset = (y * width + x) * 4;
                if offset + 4 <= image.rgba.len() {
                    image.rgba[offset..offset + 4].copy_from_slice(&MASK_COLOR);
                }
            }
        }
    }
}

/// Compare two images. Size mismatch does not count pixels.
pub fn compare_pixels(
    actual: &DecodedPng,
    baseline: &DecodedPng,
    limits: &PixelLimits,
) -> Result<u64, PixelCompareFailure> {
    if actual.width != baseline.width || actual.height != baseline.height {
        return Err(PixelCompareFailure::Size {
            actual_width: actual.width,
            actual_height: actual.height,
            baseline_width: baseline.width,
            baseline_height: baseline.height,
        });
    }
    let mut left = actual.clone();
    let mut right = baseline.clone();
    paint_pixel_masks(&mut left, &limits.masks);
    paint_pixel_masks(&mut right, &limits.masks);
    let pixel_count = u64::from(actual.width) * u64::from(actual.height);
    let threshold = f64::from(limits.threshold_millis.min(1_000)) / 1_000.0;
    let max_delta = 35_215.0 * threshold * threshold;
    let mut different = 0_u64;
    let samples = left.rgba.len() / 4;
    for index in 0..samples {
        let start = index * 4;
        let mut a = [0_u8; 4];
        let mut b = [0_u8; 4];
        a.copy_from_slice(&left.rgba[start..start + 4]);
        b.copy_from_slice(&right.rgba[start..start + 4]);
        if color_delta(a, b).abs() > max_delta {
            different += 1;
        }
    }
    let allowed = allowed_pixels(
        pixel_count,
        limits.max_diff_pixels,
        limits.max_diff_pixel_ratio_micros,
    );
    if different > allowed {
        return Err(PixelCompareFailure::Diff {
            different_pixels: different,
            allowed_pixels: allowed,
        });
    }
    Ok(different)
}

fn allowed_pixels(
    pixel_count: u64,
    max_diff_pixels: Option<u32>,
    max_diff_pixel_ratio_micros: Option<u32>,
) -> u64 {
    let from_ratio = max_diff_pixel_ratio_micros
        .map(|ratio| pixel_count.saturating_mul(u64::from(ratio.min(1_000_000))) / 1_000_000);
    match (max_diff_pixels.map(u64::from), from_ratio) {
        (None, None) => 0,
        (Some(pixels), None) => pixels,
        (None, Some(ratio)) => ratio,
        (Some(pixels), Some(ratio)) => pixels.min(ratio),
    }
}

fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), PixelDecodeError> {
    if bytes.len() < 24 || bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return Err(PixelDecodeError::Invalid);
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Ok((width, height))
}

fn to_rgba(
    frame: &[u8],
    color: png::ColorType,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, PixelDecodeError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| usize::try_from(height).ok().map(|height| width * height))
        .ok_or(PixelDecodeError::TooLarge)?;
    let mut rgba = Vec::new();
    match color {
        png::ColorType::Rgba => {
            if frame.len() < pixels * 4 {
                return Err(PixelDecodeError::Invalid);
            }
            rgba.extend_from_slice(&frame[..pixels * 4]);
        }
        png::ColorType::Rgb => {
            if frame.len() < pixels * 3 {
                return Err(PixelDecodeError::Invalid);
            }
            rgba.reserve(pixels * 4);
            for pixel in frame[..pixels * 3].chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            if frame.len() < pixels * 2 {
                return Err(PixelDecodeError::Invalid);
            }
            rgba.reserve(pixels * 4);
            for pixel in frame[..pixels * 2].chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Grayscale => {
            if frame.len() < pixels {
                return Err(PixelDecodeError::Invalid);
            }
            rgba.reserve(pixels * 4);
            for gray in &frame[..pixels] {
                rgba.extend_from_slice(&[*gray, *gray, *gray, 255]);
            }
        }
        _ => return Err(PixelDecodeError::Invalid),
    }
    Ok(rgba)
}

fn color_delta(a: [u8; 4], b: [u8; 4]) -> f64 {
    if a == b {
        return 0.0;
    }
    let (r1, g1, b1) = blend(a);
    let (r2, g2, b2) = blend(b);
    let y = rgb_y(r1, g1, b1) - rgb_y(r2, g2, b2);
    let i = rgb_i(r1, g1, b1) - rgb_i(r2, g2, b2);
    let q = rgb_q(r1, g1, b1) - rgb_q(r2, g2, b2);
    let delta = 0.5053 * y * y + 0.299 * i * i + 0.1957 * q * q;
    if y > 0.0 {
        -delta
    } else {
        delta
    }
}

fn blend(pixel: [u8; 4]) -> (f64, f64, f64) {
    if pixel[3] == 255 {
        return (
            f64::from(pixel[0]),
            f64::from(pixel[1]),
            f64::from(pixel[2]),
        );
    }
    let alpha = f64::from(pixel[3]) / 255.0;
    let channel = |value: u8| 255.0 + (f64::from(value) - 255.0) * alpha;
    (channel(pixel[0]), channel(pixel[1]), channel(pixel[2]))
}

fn rgb_y(r: f64, g: f64, b: f64) -> f64 {
    r * 0.298_895_31 + g * 0.586_622_47 + b * 0.114_482_23
}

fn rgb_i(r: f64, g: f64, b: f64) -> f64 {
    r * 0.595_977_99 - g * 0.274_176_10 - b * 0.321_801_89
}

fn rgb_q(r: f64, g: f64, b: f64) -> f64 {
    r * 0.211_470_17 - g * 0.522_617_11 + b * 0.311_146_94
}
