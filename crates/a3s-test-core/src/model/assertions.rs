use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use super::pixels::PixelMask;
use super::Target;

pub const DEFAULT_PIXEL_THRESHOLD_MILLIS: u16 = 200;
pub const MAX_PIXEL_THRESHOLD_MILLIS: u16 = 1_000;
pub const MAX_PIXEL_RATIO_MICROS: u32 = 1_000_000;

fn default_pixel_threshold() -> u16 {
    DEFAULT_PIXEL_THRESHOLD_MILLIS
}

/// Closed pixel-compare request. Threshold is thousandths of the pixelmatch
/// color distance (200 means 0.2). Ratio is millionths of the image.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelExpectation {
    pub baseline: String,
    #[serde(default = "default_pixel_threshold")]
    #[schemars(range(max = 1000))]
    pub threshold_millis: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_diff_pixels: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_diff_pixel_ratio_micros: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masks: Vec<PixelMask>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub update: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionMode {
    #[default]
    Positive,
    Hidden,
}

impl AssertionMode {
    #[must_use]
    pub fn is_positive(&self) -> bool {
        *self == Self::Positive
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitMode {
    #[default]
    Positive,
    Hidden,
}

impl WaitMode {
    #[must_use]
    pub fn is_positive(&self) -> bool {
        *self == Self::Positive
    }
}

pub const MIN_ASSERTION_STABILITY_MS: u64 = 10;
pub const MAX_ASSERTION_STABILITY_MS: u64 = 60_000;
pub const DEFAULT_ASSERTION_SAMPLE_INTERVAL_MS: u64 = 50;
pub const MAX_ASSERTION_STABILITY_SAMPLES: u64 = 1_001;
pub const MAX_RENDERED_TEXT_ITEMS: usize = 256;
pub const MAX_LAYOUT_TOLERANCE_PX: u32 = 1_024;
pub const MAX_LAYOUT_COORDINATE_ABS: f64 = 16_777_216.0;
pub const MAX_VIEWPORT_COVERAGE_PERCENT: u8 = 100;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionStability {
    pub stable_for_ms: u64,
    pub sample_interval_ms: u64,
}

impl AssertionStability {
    #[must_use]
    pub fn planned_samples(self) -> u64 {
        if self.sample_interval_ms == 0 {
            return u64::MAX;
        }
        self.stable_for_ms
            .div_ceil(self.sample_interval_ms)
            .saturating_add(1)
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        (MIN_ASSERTION_STABILITY_MS..=MAX_ASSERTION_STABILITY_MS).contains(&self.stable_for_ms)
            && (1..=self.stable_for_ms).contains(&self.sample_interval_ms)
            && self.planned_samples() <= MAX_ASSERTION_STABILITY_SAMPLES
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementState {
    Enabled,
    Checked,
    Selected,
    Focused,
    FocusWithin,
    Expanded,
    Pressed,
    #[serde(rename = "readonly")]
    ReadOnly,
    Required,
    Invalid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutRelation {
    Above,
    Below,
    LeftOf,
    RightOf,
    Contains,
    Inside,
    Overlaps,
    NotOverlapping,
    AlignedLeft,
    AlignedRight,
    AlignedTop,
    AlignedBottom,
    AlignedCenterX,
    AlignedCenterY,
    SameWidth,
    SameHeight,
    SameSize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LayoutRect {
    #[must_use]
    pub fn is_valid(self) -> bool {
        let right = self.right();
        let bottom = self.bottom();
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
            && self.width <= MAX_LAYOUT_COORDINATE_ABS
            && self.height <= MAX_LAYOUT_COORDINATE_ABS
            && self.x.abs() <= MAX_LAYOUT_COORDINATE_ABS
            && self.y.abs() <= MAX_LAYOUT_COORDINATE_ABS
            && right.is_finite()
            && bottom.is_finite()
            && right.abs() <= MAX_LAYOUT_COORDINATE_ABS
            && bottom.abs() <= MAX_LAYOUT_COORDINATE_ABS
    }

    #[must_use]
    pub fn intersection_ratio(self, container: Self) -> Option<f64> {
        if !self.is_valid() || !container.is_valid() {
            return None;
        }
        let width = (self.right().min(container.right()) - self.x.max(container.x)).max(0.0);
        let height = (self.bottom().min(container.bottom()) - self.y.max(container.y)).max(0.0);
        Some(((width * height) / (self.width * self.height)).clamp(0.0, 1.0))
    }

    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }

    fn center_x(self) -> f64 {
        self.x + self.width / 2.0
    }

    fn center_y(self) -> f64 {
        self.y + self.height / 2.0
    }
}

impl LayoutRelation {
    #[must_use]
    pub fn matches(self, target: LayoutRect, relative_to: LayoutRect, tolerance_px: u32) -> bool {
        if tolerance_px > MAX_LAYOUT_TOLERANCE_PX || !target.is_valid() || !relative_to.is_valid() {
            return false;
        }
        let tolerance = f64::from(tolerance_px);
        let overlap_width = target.right().min(relative_to.right()) - target.x.max(relative_to.x);
        let overlap_height =
            target.bottom().min(relative_to.bottom()) - target.y.max(relative_to.y);
        match self {
            Self::Above => target.bottom() <= relative_to.y + tolerance,
            Self::Below => target.y + tolerance >= relative_to.bottom(),
            Self::LeftOf => target.right() <= relative_to.x + tolerance,
            Self::RightOf => target.x + tolerance >= relative_to.right(),
            Self::Contains => {
                target.x <= relative_to.x + tolerance
                    && target.y <= relative_to.y + tolerance
                    && target.right() + tolerance >= relative_to.right()
                    && target.bottom() + tolerance >= relative_to.bottom()
            }
            Self::Inside => {
                relative_to.x <= target.x + tolerance
                    && relative_to.y <= target.y + tolerance
                    && relative_to.right() + tolerance >= target.right()
                    && relative_to.bottom() + tolerance >= target.bottom()
            }
            Self::Overlaps => overlap_width > tolerance && overlap_height > tolerance,
            Self::NotOverlapping => overlap_width <= tolerance || overlap_height <= tolerance,
            Self::AlignedLeft => (target.x - relative_to.x).abs() <= tolerance,
            Self::AlignedRight => (target.right() - relative_to.right()).abs() <= tolerance,
            Self::AlignedTop => (target.y - relative_to.y).abs() <= tolerance,
            Self::AlignedBottom => (target.bottom() - relative_to.bottom()).abs() <= tolerance,
            Self::AlignedCenterX => (target.center_x() - relative_to.center_x()).abs() <= tolerance,
            Self::AlignedCenterY => (target.center_y() - relative_to.center_y()).abs() <= tolerance,
            Self::SameWidth => (target.width - relative_to.width).abs() <= tolerance,
            Self::SameHeight => (target.height - relative_to.height).abs() <= tolerance,
            Self::SameSize => {
                (target.width - relative_to.width).abs() <= tolerance
                    && (target.height - relative_to.height).abs() <= tolerance
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewportCoverageComparison {
    AtLeast,
    AtMost,
}

impl ViewportCoverageComparison {
    #[must_use]
    pub fn threshold_is_valid(self, percent: u8) -> bool {
        match self {
            Self::AtLeast => (1..=MAX_VIEWPORT_COVERAGE_PERCENT).contains(&percent),
            Self::AtMost => percent < MAX_VIEWPORT_COVERAGE_PERCENT,
        }
    }

    #[must_use]
    pub fn matches(self, intersection_ratio: f64, percent: u8) -> bool {
        if !self.threshold_is_valid(percent)
            || !intersection_ratio.is_finite()
            || !(0.0..=1.0).contains(&intersection_ratio)
        {
            return false;
        }
        let threshold_ratio = f64::from(percent) / f64::from(MAX_VIEWPORT_COVERAGE_PERCENT);
        match self {
            Self::AtLeast => intersection_ratio >= threshold_ratio,
            Self::AtMost => intersection_ratio <= threshold_ratio,
        }
    }
}

/// Admit a page-text regular expression before it is stored or executed.
///
/// The pattern is a Rust regex of at most 256 characters. Size limits reject
/// expressions that would compile into an unbounded automaton.
pub fn admit_text_pattern(pattern: &str) -> Result<(), &'static str> {
    if pattern.is_empty() || pattern.chars().count() > super::MAX_TEXT_PATTERN_CHARS {
        return Err("text pattern must contain 1 to 256 characters");
    }
    text_pattern(pattern).map(|_| ())
}

/// Report whether `page_text` contains a match for an already admitted pattern.
pub fn text_pattern_matches(page_text: &str, pattern: &str) -> Result<bool, &'static str> {
    Ok(text_pattern(pattern)?.is_match(page_text))
}

fn text_pattern(pattern: &str) -> Result<regex::Regex, &'static str> {
    regex::RegexBuilder::new(pattern)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()
        .map_err(|_| "text pattern is not a supported regular expression")
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Expectation {
    TextVisible(String),
    TextContains(String),
    TextMatches(String),
    Url(String),
    Visible(Target),
    InViewport(Target),
    ViewportCoverage {
        target: Target,
        comparison: ViewportCoverageComparison,
        #[schemars(range(max = 100))]
        percent: u8,
    },
    PointerReachable(Target),
    RenderedText {
        target: Target,
        value: String,
    },
    RenderedTexts {
        target: Target,
        #[schemars(length(max = 256))]
        values: Vec<String>,
    },
    VisibleCount {
        target: Target,
        count: u32,
    },
    State {
        target: Target,
        state: ElementState,
        expected: bool,
    },
    Value {
        target: Target,
        value: String,
    },
    SelectedValues {
        target: Target,
        values: Vec<String>,
    },
    Layout {
        target: Target,
        relative_to: Target,
        relation: LayoutRelation,
        #[schemars(range(max = 1024))]
        tolerance_px: u32,
    },
    Pixels(PixelExpectation),
}
