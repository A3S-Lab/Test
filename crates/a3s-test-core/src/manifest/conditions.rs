use std::collections::BTreeSet;

use a3s_acl::{Block, Value};

use crate::{
    AssertionMode, AssertionStability, ElementState, Expectation, LayoutRelation, LoadState,
    SpecError, Target, WaitCondition, WaitMode, DEFAULT_ASSERTION_SAMPLE_INTERVAL_MS,
    MAX_ASSERTION_STABILITY_MS, MAX_ASSERTION_STABILITY_SAMPLES, MAX_LAYOUT_TOLERANCE_PX,
    MAX_RENDERED_TEXT_ITEMS, MIN_ASSERTION_STABILITY_MS,
};

use super::{
    optional_integer, parse_target, positive_integer, required_nonnegative_u32, required_target,
    type_error, value_string,
};

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
    let conditions = [
        "text",
        "url",
        "visible",
        "hidden",
        "in_viewport",
        "pointer_reachable",
        "rendered_text",
        "rendered_texts",
        "visible_count",
        "value",
        "enabled",
        "disabled",
        "checked",
        "unchecked",
        "selected",
        "unselected",
        "focused",
        "unfocused",
        "focus_within",
        "focus_outside",
        "expanded",
        "collapsed",
        "pressed",
        "unpressed",
        "readonly",
        "writable",
        "required",
        "optional",
        "invalid",
        "valid",
        "selected_values",
        "layout",
    ];
    let configured = conditions
        .iter()
        .filter(|name| block.attributes.contains_key(**name))
        .copied()
        .collect::<Vec<_>>();
    if configured.len() != 1 {
        return Err(condition_count_error(path, configured.len()));
    }
    let condition = configured[0];
    let uses_separate_target = matches!(
        condition,
        "rendered_text"
            | "rendered_texts"
            | "visible_count"
            | "value"
            | "selected_values"
            | "layout"
    );
    if block.attributes.contains_key("target") && !uses_separate_target {
        return Err(SpecError::new(
            "test.spec.attribute_unexpected",
            format!("{path}.target"),
            "target is valid only with rendered_text, rendered_texts, visible_count, value, selected_values, or layout expectations",
        ));
    }
    if condition != "layout" {
        for name in ["relative_to", "tolerance_px"] {
            if block.attributes.contains_key(name) {
                return Err(SpecError::new(
                    "test.spec.attribute_unexpected",
                    format!("{path}.{name}"),
                    "attribute is valid only with layout expectations",
                ));
            }
        }
    }

    let positive = match condition {
        "text" => Expectation::TextVisible(value_string(
            &block.attributes[condition],
            format!("{path}.text"),
        )?),
        "url" => Expectation::Url(value_string(
            &block.attributes[condition],
            format!("{path}.url"),
        )?),
        "visible" => Expectation::Visible(parse_target(
            &block.attributes[condition],
            &format!("{path}.visible"),
        )?),
        "in_viewport" => Expectation::InViewport(stable_interactability_target(
            parse_target(&block.attributes[condition], &format!("{path}.in_viewport"))?,
            path,
            "in_viewport",
        )?),
        "pointer_reachable" => Expectation::PointerReachable(stable_interactability_target(
            parse_target(
                &block.attributes[condition],
                &format!("{path}.pointer_reachable"),
            )?,
            path,
            "pointer_reachable",
        )?),
        "hidden" => {
            let target = parse_target(&block.attributes[condition], &format!("{path}.hidden"))?;
            if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
                return Err(SpecError::new(
                    "test.spec.hidden_target_unstable",
                    format!("{path}.hidden"),
                    "hidden assertions require a stable semantic or CSS locator, not an observation-bound ref or visual point",
                ));
            }
            return Ok((Expectation::Visible(target), AssertionMode::Hidden));
        }
        "rendered_text" => Expectation::RenderedText {
            target: required_target(block, "target", path)?,
            value: value_string(
                &block.attributes[condition],
                format!("{path}.rendered_text"),
            )?,
        },
        "rendered_texts" => {
            let target = required_target(block, "target", path)?;
            if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
                return Err(SpecError::new(
                    "test.spec.rendered_texts_target_unstable",
                    format!("{path}.target"),
                    "rendered_texts requires a stable semantic or CSS locator, not an observation-bound ref or visual point",
                ));
            }
            Expectation::RenderedTexts {
                target,
                values: rendered_texts(
                    &block.attributes[condition],
                    &format!("{path}.rendered_texts"),
                )?,
            }
        }
        "visible_count" => {
            let target = required_target(block, "target", path)?;
            if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
                return Err(SpecError::new(
                    "test.spec.visible_count_target_unstable",
                    format!("{path}.target"),
                    "visible_count requires a stable semantic or CSS locator, not an observation-bound ref or visual point",
                ));
            }
            Expectation::VisibleCount {
                target,
                count: required_nonnegative_u32(block, condition, path)?,
            }
        }
        "value" => Expectation::Value {
            target: required_target(block, "target", path)?,
            value: value_string(&block.attributes[condition], format!("{path}.value"))?,
        },
        "selected_values" => Expectation::SelectedValues {
            target: required_target(block, "target", path)?,
            values: selected_values(
                &block.attributes[condition],
                &format!("{path}.selected_values"),
            )?,
        },
        "layout" => {
            let target =
                stable_layout_target(required_target(block, "target", path)?, path, "target")?;
            let relative_to = stable_layout_target(
                required_target(block, "relative_to", path)?,
                path,
                "relative_to",
            )?;
            let tolerance_px = if block.attributes.contains_key("tolerance_px") {
                required_nonnegative_u32(block, "tolerance_px", path)?
            } else {
                0
            };
            if tolerance_px > MAX_LAYOUT_TOLERANCE_PX {
                return Err(SpecError::new(
                    "test.spec.layout_tolerance_limit",
                    format!("{path}.tolerance_px"),
                    format!("layout tolerance cannot exceed {MAX_LAYOUT_TOLERANCE_PX} pixels"),
                ));
            }
            Expectation::Layout {
                target,
                relative_to,
                relation: layout_relation(&block.attributes[condition], &format!("{path}.layout"))?,
                tolerance_px,
            }
        }
        "enabled" | "disabled" | "checked" | "unchecked" | "selected" | "unselected"
        | "focused" | "unfocused" | "focus_within" | "focus_outside" | "expanded" | "collapsed"
        | "pressed" | "unpressed" | "readonly" | "writable" | "required" | "optional"
        | "invalid" | "valid" => {
            let (state, expected) = match condition {
                "enabled" => (ElementState::Enabled, true),
                "disabled" => (ElementState::Enabled, false),
                "checked" => (ElementState::Checked, true),
                "unchecked" => (ElementState::Checked, false),
                "selected" => (ElementState::Selected, true),
                "unselected" => (ElementState::Selected, false),
                "focused" => (ElementState::Focused, true),
                "unfocused" => (ElementState::Focused, false),
                "focus_within" => (ElementState::FocusWithin, true),
                "focus_outside" => (ElementState::FocusWithin, false),
                "expanded" => (ElementState::Expanded, true),
                "collapsed" => (ElementState::Expanded, false),
                "pressed" => (ElementState::Pressed, true),
                "unpressed" => (ElementState::Pressed, false),
                "readonly" => (ElementState::ReadOnly, true),
                "writable" => (ElementState::ReadOnly, false),
                "required" => (ElementState::Required, true),
                "optional" => (ElementState::Required, false),
                "invalid" => (ElementState::Invalid, true),
                "valid" => (ElementState::Invalid, false),
                _ => unreachable!("bounded state condition"),
            };
            let target =
                parse_target(&block.attributes[condition], &format!("{path}.{condition}"))?;
            let target = if matches!(state, ElementState::Focused | ElementState::FocusWithin) {
                stable_focus_target(target, path, condition)?
            } else if matches!(
                state,
                ElementState::Expanded
                    | ElementState::Pressed
                    | ElementState::ReadOnly
                    | ElementState::Required
                    | ElementState::Invalid
            ) {
                stable_semantic_state_target(target, path, condition)?
            } else {
                target
            };
            Expectation::State {
                target,
                state,
                expected,
            }
        }
        _ => unreachable!("condition list and parser must remain aligned"),
    };
    Ok((positive, AssertionMode::Positive))
}

fn stable_semantic_state_target(
    target: Target,
    path: &str,
    condition: &str,
) -> Result<Target, SpecError> {
    if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
        return Err(SpecError::new(
            "test.spec.semantic_state_target_unstable",
            format!("{path}.{condition}"),
            "semantic state assertions require a stable semantic or CSS locator, not an observation-bound ref or visual point",
        ));
    }
    Ok(target)
}

fn stable_focus_target(target: Target, path: &str, condition: &str) -> Result<Target, SpecError> {
    if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
        return Err(SpecError::new(
            "test.spec.focus_target_unstable",
            format!("{path}.{condition}"),
            "focus ownership assertions require a stable semantic or CSS locator, not an observation-bound ref or visual point",
        ));
    }
    Ok(target)
}

fn stable_layout_target(target: Target, path: &str, name: &str) -> Result<Target, SpecError> {
    if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
        return Err(SpecError::new(
            "test.spec.layout_target_unstable",
            format!("{path}.{name}"),
            "layout assertions require stable locators for both elements, not observation-bound refs or visual points",
        ));
    }
    Ok(target)
}

fn stable_interactability_target(
    target: Target,
    path: &str,
    condition: &str,
) -> Result<Target, SpecError> {
    if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
        let code = match condition {
            "in_viewport" => "test.spec.in_viewport_target_unstable",
            "pointer_reachable" => "test.spec.pointer_reachable_target_unstable",
            _ => unreachable!("bounded interactability condition"),
        };
        return Err(SpecError::new(
            code,
            format!("{path}.{condition}"),
            format!(
                "{condition} assertions require a stable semantic or CSS locator, not an observation-bound ref or visual point"
            ),
        ));
    }
    Ok(target)
}

fn layout_relation(value: &Value, path: &str) -> Result<LayoutRelation, SpecError> {
    let relation = value
        .as_str()
        .ok_or_else(|| type_error(path, "layout relation must be a string"))?;
    match relation {
        "above" => Ok(LayoutRelation::Above),
        "below" => Ok(LayoutRelation::Below),
        "left_of" => Ok(LayoutRelation::LeftOf),
        "right_of" => Ok(LayoutRelation::RightOf),
        "contains" => Ok(LayoutRelation::Contains),
        "inside" => Ok(LayoutRelation::Inside),
        "overlaps" => Ok(LayoutRelation::Overlaps),
        "not_overlapping" => Ok(LayoutRelation::NotOverlapping),
        "aligned_left" => Ok(LayoutRelation::AlignedLeft),
        "aligned_right" => Ok(LayoutRelation::AlignedRight),
        "aligned_top" => Ok(LayoutRelation::AlignedTop),
        "aligned_bottom" => Ok(LayoutRelation::AlignedBottom),
        "aligned_center_x" => Ok(LayoutRelation::AlignedCenterX),
        "aligned_center_y" => Ok(LayoutRelation::AlignedCenterY),
        "same_width" => Ok(LayoutRelation::SameWidth),
        "same_height" => Ok(LayoutRelation::SameHeight),
        "same_size" => Ok(LayoutRelation::SameSize),
        _ => Err(SpecError::new(
            "test.spec.layout_relation_unknown",
            path,
            "unknown layout relation",
        )),
    }
}

fn rendered_texts(value: &Value, path: &str) -> Result<Vec<String>, SpecError> {
    let Value::List(values) = value else {
        return Err(type_error(path, "rendered_texts must be a list of strings"));
    };
    if values.len() > MAX_RENDERED_TEXT_ITEMS {
        return Err(SpecError::new(
            "test.spec.rendered_texts_limit",
            path,
            format!("rendered_texts cannot contain more than {MAX_RENDERED_TEXT_ITEMS} items"),
        ));
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                type_error(
                    format!("{path}[{index}]"),
                    "rendered_texts items must be strings",
                )
            })
        })
        .collect()
}

fn selected_values(value: &Value, path: &str) -> Result<Vec<String>, SpecError> {
    let Value::List(values) = value else {
        return Err(type_error(
            path,
            "selected_values must be a list of strings",
        ));
    };
    let mut selected = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        let item = value
            .as_str()
            .ok_or_else(|| type_error(&item_path, "selected_values items must be strings"))?;
        if !selected.insert(item.to_string()) {
            return Err(SpecError::new(
                "test.spec.selected_value_duplicate",
                item_path,
                "selected_values cannot contain duplicates",
            ));
        }
    }
    Ok(selected.into_iter().collect())
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
