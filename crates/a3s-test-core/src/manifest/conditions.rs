use a3s_acl::Block;

use crate::{
    AssertionMode, AssertionStability, Expectation, LoadState, SpecError, Target, WaitCondition,
    WaitMode, DEFAULT_ASSERTION_SAMPLE_INTERVAL_MS, MAX_ASSERTION_STABILITY_MS,
    MAX_ASSERTION_STABILITY_SAMPLES, MIN_ASSERTION_STABILITY_MS,
};

use super::{optional_integer, parse_target, positive_integer, type_error, value_string};

pub(super) fn parse_wait(
    block: &Block,
    path: &str,
) -> Result<(WaitCondition, WaitMode), SpecError> {
    let count = ["load", "text", "regex", "url", "visible", "hidden"]
        .iter()
        .filter(|name| block.attributes.contains_key(**name))
        .count();
    if count != 1 {
        return Err(condition_count_error(path, count));
    }

    if let Some(value) = block.attributes.get("load") {
        let state = value
            .as_str()
            .ok_or_else(|| type_error(format!("{path}.load"), "load condition must be a string"))?;
        return match state {
            "networkidle" => Ok((
                WaitCondition::Load(LoadState::NetworkIdle),
                WaitMode::Positive,
            )),
            "domcontentloaded" => Ok((
                WaitCondition::Load(LoadState::DomContentLoaded),
                WaitMode::Positive,
            )),
            _ => Err(SpecError::new(
                "test.spec.load_state_unknown",
                format!("{path}.load"),
                "load must be networkidle or domcontentloaded",
            )),
        };
    }
    if let Some(value) = block.attributes.get("text") {
        return Ok((
            WaitCondition::Text(value_string(value, format!("{path}.text"))?),
            WaitMode::Positive,
        ));
    }
    if let Some(value) = block.attributes.get("regex") {
        return Ok((
            WaitCondition::Regex(value_string(value, format!("{path}.regex"))?),
            WaitMode::Positive,
        ));
    }
    if let Some(value) = block.attributes.get("url") {
        return Ok((
            WaitCondition::Url(value_string(value, format!("{path}.url"))?),
            WaitMode::Positive,
        ));
    }
    if let Some(value) = block.attributes.get("hidden") {
        let target = parse_target(value, &format!("{path}.hidden"))?;
        if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
            return Err(SpecError::new(
                "test.spec.hidden_target_unstable",
                format!("{path}.hidden"),
                "hidden waits require a stable semantic or CSS locator, not an observation-bound ref or visual point",
            ));
        }
        return Ok((WaitCondition::Visible(target), WaitMode::Hidden));
    }
    let value = block
        .attributes
        .get("visible")
        .expect("condition count guarantees a visible target");
    Ok((
        WaitCondition::Visible(parse_target(value, &format!("{path}.visible"))?),
        WaitMode::Positive,
    ))
}

pub(super) fn parse_expectation(
    block: &Block,
    path: &str,
) -> Result<(Expectation, AssertionMode), SpecError> {
    let count = ["text", "url", "visible", "hidden"]
        .iter()
        .filter(|name| block.attributes.contains_key(**name))
        .count();
    if count != 1 {
        return Err(condition_count_error(path, count));
    }

    if let Some(value) = block.attributes.get("text") {
        return Ok((
            Expectation::TextVisible(value_string(value, format!("{path}.text"))?),
            AssertionMode::Positive,
        ));
    }
    if let Some(value) = block.attributes.get("url") {
        return Ok((
            Expectation::Url(value_string(value, format!("{path}.url"))?),
            AssertionMode::Positive,
        ));
    }
    if let Some(value) = block.attributes.get("hidden") {
        let target = parse_target(value, &format!("{path}.hidden"))?;
        if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
            return Err(SpecError::new(
                "test.spec.hidden_target_unstable",
                format!("{path}.hidden"),
                "hidden assertions require a stable semantic or CSS locator, not an observation-bound ref or visual point",
            ));
        }
        return Ok((Expectation::Visible(target), AssertionMode::Hidden));
    }
    let value = block
        .attributes
        .get("visible")
        .expect("condition count guarantees a visible target");
    Ok((
        Expectation::Visible(parse_target(value, &format!("{path}.visible"))?),
        AssertionMode::Positive,
    ))
}

pub(super) fn parse_assertion_stability(
    block: &Block,
    path: &str,
) -> Result<Option<AssertionStability>, SpecError> {
    let Some(stable_for_value) = block.attributes.get("stable_for_ms") else {
        if block.attributes.contains_key("sample_interval_ms") {
            return Err(SpecError::new(
                "test.spec.stability_duration_required",
                format!("{path}.sample_interval_ms"),
                "sample_interval_ms requires stable_for_ms",
            ));
        }
        return Ok(None);
    };
    let stable_for_ms = positive_integer(stable_for_value, &format!("{path}.stable_for_ms"))?;
    if !(MIN_ASSERTION_STABILITY_MS..=MAX_ASSERTION_STABILITY_MS).contains(&stable_for_ms) {
        return Err(SpecError::new(
            "test.spec.stability_range",
            format!("{path}.stable_for_ms"),
            format!(
                "stable_for_ms must be between {MIN_ASSERTION_STABILITY_MS} and {MAX_ASSERTION_STABILITY_MS}"
            ),
        ));
    }
    let sample_interval_ms = optional_integer(
        block,
        "sample_interval_ms",
        DEFAULT_ASSERTION_SAMPLE_INTERVAL_MS.min(stable_for_ms),
        path,
    )?;
    if sample_interval_ms > stable_for_ms {
        return Err(SpecError::new(
            "test.spec.stability_interval",
            format!("{path}.sample_interval_ms"),
            "sample_interval_ms must not exceed stable_for_ms",
        ));
    }
    let stability = AssertionStability {
        stable_for_ms,
        sample_interval_ms,
    };
    if stability.planned_samples() > MAX_ASSERTION_STABILITY_SAMPLES {
        return Err(SpecError::new(
            "test.spec.stability_sample_limit",
            path,
            format!(
                "assertion stability cannot require more than {MAX_ASSERTION_STABILITY_SAMPLES} samples"
            ),
        ));
    }
    Ok(Some(stability))
}

pub(super) fn condition_count_error(path: &str, count: usize) -> SpecError {
    let (code, message) = if count == 0 {
        (
            "test.spec.condition_required",
            "exactly one condition is required",
        )
    } else {
        (
            "test.spec.condition_ambiguous",
            "only one condition can be configured",
        )
    };
    SpecError::new(code, path, message)
}
