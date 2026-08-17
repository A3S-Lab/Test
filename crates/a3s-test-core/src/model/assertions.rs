use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Target;

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
    Url(String),
    Visible(Target),
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
}
