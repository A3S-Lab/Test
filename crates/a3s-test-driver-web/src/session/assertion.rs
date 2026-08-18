use std::collections::BTreeSet;
use std::ffi::OsString;

use a3s_test_core::{
    DriverError, ElementState, Expectation, LayoutRect, LayoutRelation, StepOutput, Target,
    ViewportCoverageComparison, MAX_LAYOUT_TOLERANCE_PX, MAX_RENDERED_TEXT_ITEMS,
    MAX_VIEWPORT_COVERAGE_PERCENT,
};
use serde_json::{json, Value};

use crate::protocol::{
    assertion_probe_args, interactability_probe_args, layout_probe_args, scalar_bool,
    scalar_string, visibility_args, AssertionProbe, InteractabilityProbe, POINTER_SAMPLE_COUNT,
};

use super::{browser_result, AgentBrowserSession};

impl AgentBrowserSession {
    pub(super) async fn assert(
        &self,
        expectation: &Expectation,
    ) -> Result<StepOutput, DriverError> {
        match expectation {
            Expectation::TextVisible(text) => {
                let data = self
                    .execute_command(vec!["wait".into(), "--text".into(), text.into()])
                    .await?;
                Ok(StepOutput::new("text is visible").with_data(data))
            }
            Expectation::Url(expected) => {
                let data = self
                    .execute_command(vec!["get".into(), "url".into()])
                    .await?;
                let actual = scalar_string(&data).ok_or_else(|| {
                    DriverError::new(
                        "test.driver.web.output_invalid",
                        "browser URL response did not contain a string",
                    )
                })?;
                if actual != expected {
                    return Err(DriverError::new(
                        "test.assert.url",
                        format!("expected URL '{expected}', received '{actual}'"),
                    ));
                }
                Ok(StepOutput::new("URL matched").with_data(data))
            }
            Expectation::Visible(target) => {
                let data = self.execute_command(visibility_args(target)?).await?;
                match scalar_bool(&browser_result(data.clone())).or_else(|| scalar_bool(&data)) {
                    Some(true) => Ok(StepOutput::new("target is visible").with_data(data)),
                    Some(false) => Err(DriverError::new(
                        "test.assert.visible",
                        "target is not visible",
                    )),
                    None => Err(DriverError::new(
                        "test.driver.web.output_invalid",
                        "browser visibility response did not contain a boolean",
                    )),
                }
            }
            Expectation::InViewport(target) => {
                self.assert_interactability(target, InteractabilityProbe::InViewport)
                    .await
            }
            Expectation::ViewportCoverage {
                target,
                comparison,
                percent,
            } => {
                self.assert_viewport_coverage(target, *comparison, *percent)
                    .await
            }
            Expectation::PointerReachable(target) => {
                self.assert_interactability(target, InteractabilityProbe::PointerReachable)
                    .await
            }
            Expectation::RenderedText { target, value } => {
                self.assert_rendered_text(target, value).await
            }
            Expectation::RenderedTexts { target, values } => {
                self.assert_rendered_texts(target, values).await
            }
            Expectation::VisibleCount { target, count } => {
                self.assert_visible_count(target, *count).await
            }
            Expectation::State {
                target,
                state,
                expected,
            } => self.assert_state(target, *state, *expected).await,
            Expectation::Value { target, value } => self.assert_value(target, value).await,
            Expectation::SelectedValues { target, values } => {
                self.assert_selected_values(target, values).await
            }
            Expectation::Layout {
                target,
                relative_to,
                relation,
                tolerance_px,
            } => {
                self.assert_layout(target, relative_to, *relation, *tolerance_px)
                    .await
            }
        }
    }

    async fn assert_viewport_coverage(
        &self,
        target: &Target,
        comparison: ViewportCoverageComparison,
        percent: u8,
    ) -> Result<StepOutput, DriverError> {
        if !comparison.threshold_is_valid(percent) {
            return Err(DriverError::new(
                "test.driver.web.expectation_invalid",
                format!(
                    "viewport coverage threshold must be non-trivial and no greater than {MAX_VIEWPORT_COVERAGE_PERCENT} percent"
                ),
            ));
        }
        let data = self
            .execute_command(interactability_probe_args(
                target,
                InteractabilityProbe::InViewport,
            )?)
            .await?;
        let result = browser_result(data);
        let (target_rect, viewport_rect) = interactability_rects(&result)?;
        let intersection_ratio = target_rect
            .intersection_ratio(viewport_rect)
            .ok_or_else(|| output_invalid("viewport coverage geometry"))?;
        let actual_percent = intersection_ratio * f64::from(MAX_VIEWPORT_COVERAGE_PERCENT);
        if !comparison.matches(intersection_ratio, percent) {
            let (description, code) = viewport_coverage_expectation(comparison);
            return Err(DriverError::new(
                code,
                format!(
                    "expected viewport coverage to be {description} {percent} percent, observed {actual_percent} percent"
                ),
            ));
        }
        Ok(
            StepOutput::new("target viewport coverage matched").with_data(json!({
                "target": target,
                "target_rect": rect_data(target_rect),
                "viewport_rect": rect_data(viewport_rect),
                "intersection_ratio": intersection_ratio,
                "actual_percent": actual_percent,
                "comparison": comparison,
                "threshold_percent": percent,
                "matched": true,
            })),
        )
    }

    async fn assert_interactability(
        &self,
        target: &Target,
        probe: InteractabilityProbe,
    ) -> Result<StepOutput, DriverError> {
        let data = self
            .execute_command(interactability_probe_args(target, probe)?)
            .await?;
        let result = browser_result(data);
        let (target_rect, viewport_rect) = interactability_rects(&result)?;
        let intersection_ratio = target_rect
            .intersection_ratio(viewport_rect)
            .ok_or_else(|| output_invalid("interactability geometry"))?;
        let common = json!({
            "target": target,
            "target_rect": rect_data(target_rect),
            "viewport_rect": rect_data(viewport_rect),
            "intersection_ratio": intersection_ratio,
        });
        match probe {
            InteractabilityProbe::InViewport => {
                if intersection_ratio <= 0.0 {
                    return Err(DriverError::new(
                        "test.assert.in_viewport",
                        "the rendered target has no positive-area intersection with the visual viewport",
                    ));
                }
                let mut data = common;
                data["in_viewport"] = Value::Bool(true);
                Ok(StepOutput::new("target intersects the visual viewport").with_data(data))
            }
            InteractabilityProbe::PointerReachable => {
                let samples = pointer_samples(&result, target_rect, viewport_rect)?;
                let reachable_samples = samples.iter().filter(|sample| sample.reachable).count();
                if reachable_samples == 0 {
                    return Err(DriverError::new(
                        "test.assert.pointer_reachable",
                        "no admitted pointer sample reached the target or a composed-tree descendant",
                    ));
                }
                let mut data = common;
                data["pointer_reachable"] = Value::Bool(true);
                data["sample_count"] = Value::from(samples.len());
                data["reachable_samples"] = Value::from(reachable_samples);
                data["samples"] = Value::Array(
                    samples
                        .into_iter()
                        .map(|sample| {
                            json!({
                                "x": sample.x,
                                "y": sample.y,
                                "reachable": sample.reachable,
                            })
                        })
                        .collect(),
                );
                Ok(StepOutput::new("target accepts an admitted pointer hit").with_data(data))
            }
        }
    }

    async fn assert_layout(
        &self,
        target: &Target,
        relative_to: &Target,
        relation: LayoutRelation,
        tolerance_px: u32,
    ) -> Result<StepOutput, DriverError> {
        if tolerance_px > MAX_LAYOUT_TOLERANCE_PX {
            return Err(DriverError::new(
                "test.driver.web.expectation_invalid",
                format!("layout tolerance cannot exceed {MAX_LAYOUT_TOLERANCE_PX} pixels"),
            ));
        }
        let data = self
            .execute_command(layout_probe_args(target, relative_to)?)
            .await?;
        let result = browser_result(data);
        let (target_rect, relative_rect) = layout_rects(&result)?;
        if !relation.matches(target_rect, relative_rect, tolerance_px) {
            return Err(DriverError::new(
                "test.assert.layout",
                format!(
                    "expected {target_rect:?} to be {relation:?} relative to {relative_rect:?} within {tolerance_px}px"
                ),
            ));
        }
        Ok(StepOutput::new("layout relation matched").with_data(json!({
            "target": target,
            "relative_to": relative_to,
            "relation": relation,
            "tolerance_px": tolerance_px,
            "target_rect": rect_data(target_rect),
            "relative_rect": rect_data(relative_rect),
            "matched": true,
        })))
    }

    async fn assert_state(
        &self,
        target: &Target,
        state: ElementState,
        expected: bool,
    ) -> Result<StepOutput, DriverError> {
        let actual = if matches!(target, Target::Ref { .. }) {
            self.ref_state(target, state).await?
        } else {
            let value = self
                .probe_target(target, AssertionProbe::State(state))
                .await?;
            value.as_bool().ok_or_else(|| output_invalid("boolean"))?
        };
        let (name, mismatch_code) = state_expectation(state, expected);
        if actual != expected {
            return Err(DriverError::new(
                mismatch_code,
                format!("expected target to be {name}, observed {actual}"),
            ));
        }
        Ok(
            StepOutput::new(format!("target {name} state matched")).with_data(json!({
                "target": target,
                "state": element_state_name(state),
                "expected": expected,
                "actual": actual,
            })),
        )
    }

    async fn assert_value(
        &self,
        target: &Target,
        expected: &str,
    ) -> Result<StepOutput, DriverError> {
        let actual = if let Target::Ref { value } = target {
            let data = self
                .execute_command(vec!["get".into(), "value".into(), value.into()])
                .await?;
            scalar_string(&data)
                .map(ToOwned::to_owned)
                .ok_or_else(|| output_invalid("string"))?
        } else {
            self.probe_target(target, AssertionProbe::Value)
                .await?
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| output_invalid("string"))?
        };
        if actual != expected {
            return Err(DriverError::new(
                "test.assert.value",
                format!("expected target value {expected:?}, received {actual:?}"),
            ));
        }
        Ok(StepOutput::new("target value matched").with_data(json!({
            "target": target,
            "expected": expected,
            "actual": actual,
        })))
    }

    async fn assert_rendered_text(
        &self,
        target: &Target,
        expected: &str,
    ) -> Result<StepOutput, DriverError> {
        let actual = if let Target::Ref { value } = target {
            let data = self
                .execute_command(vec!["get".into(), "text".into(), value.into()])
                .await?;
            scalar_string(&data)
                .map(normalize_rendered_text)
                .ok_or_else(|| output_invalid("string"))?
        } else {
            self.probe_target(target, AssertionProbe::RenderedText)
                .await?
                .as_str()
                .map(normalize_rendered_text)
                .ok_or_else(|| output_invalid("string"))?
        };
        let expected = normalize_rendered_text(expected);
        if actual != expected {
            return Err(DriverError::new(
                "test.assert.rendered_text",
                format!("expected rendered text {expected:?}, received {actual:?}"),
            ));
        }
        Ok(
            StepOutput::new("target rendered text matched").with_data(json!({
                "target": target,
                "expected": expected,
                "actual": actual,
            })),
        )
    }

    async fn assert_visible_count(
        &self,
        target: &Target,
        expected: u32,
    ) -> Result<StepOutput, DriverError> {
        if matches!(target, Target::Ref { .. }) {
            return Err(DriverError::new(
                "test.driver.web.target_unsupported",
                "visible_count requires a stable CSS or semantic locator, not an observation-bound ref",
            ));
        }
        let value = self
            .probe_target(target, AssertionProbe::VisibleCount)
            .await?;
        let actual = value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| output_invalid("unsigned 32-bit integer"))?;
        if actual != expected {
            return Err(DriverError::new(
                "test.assert.visible_count",
                format!("expected {expected} visible matches, received {actual}"),
            ));
        }
        Ok(
            StepOutput::new("target visible count matched").with_data(json!({
                "target": target,
                "expected": expected,
                "actual": actual,
            })),
        )
    }

    async fn assert_rendered_texts(
        &self,
        target: &Target,
        expected: &[String],
    ) -> Result<StepOutput, DriverError> {
        if expected.len() > MAX_RENDERED_TEXT_ITEMS {
            return Err(DriverError::new(
                "test.driver.web.expectation_invalid",
                format!("rendered_texts cannot contain more than {MAX_RENDERED_TEXT_ITEMS} items"),
            ));
        }
        if matches!(target, Target::Ref { .. }) {
            return Err(DriverError::new(
                "test.driver.web.target_unsupported",
                "rendered_texts requires a stable CSS or semantic locator, not an observation-bound ref",
            ));
        }
        let expected = expected
            .iter()
            .map(|value| normalize_rendered_text(value))
            .collect::<Vec<_>>();
        let value = self
            .probe_target(target, AssertionProbe::RenderedTexts)
            .await?;
        let actual = rendered_text_values(&value)?;
        if actual != expected {
            return Err(DriverError::new(
                "test.assert.rendered_texts",
                format!("expected rendered texts {expected:?}, received {actual:?}"),
            ));
        }
        Ok(
            StepOutput::new("target rendered text sequence matched").with_data(json!({
                "target": target,
                "expected": expected,
                "actual": actual,
                "count": actual.len(),
            })),
        )
    }

    async fn assert_selected_values(
        &self,
        target: &Target,
        expected: &[String],
    ) -> Result<StepOutput, DriverError> {
        if matches!(target, Target::Ref { .. }) {
            return Err(DriverError::new(
                "test.driver.web.state_unsupported",
                "selected_values assertions require a stable CSS or semantic target with the current browser protocol",
            ));
        }
        let expected = canonical_values(expected, "expectation_invalid")?;
        let value = self
            .probe_target(target, AssertionProbe::SelectedValues)
            .await?;
        let values = value
            .as_array()
            .ok_or_else(|| output_invalid("string array"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| output_invalid("string array"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let actual = canonical_values(&values, "output_invalid")?;
        if actual != expected {
            return Err(DriverError::new(
                "test.assert.selected_values",
                format!("expected selected values {expected:?}, received {actual:?}"),
            ));
        }
        Ok(
            StepOutput::new("target selected values matched").with_data(json!({
                "target": target,
                "expected": expected,
                "actual": actual,
            })),
        )
    }

    async fn ref_state(&self, target: &Target, state: ElementState) -> Result<bool, DriverError> {
        let Target::Ref { value } = target else {
            return Err(DriverError::new(
                "test.driver.web.target_unsupported",
                "native state queries require a current browser ref",
            ));
        };
        match state {
            ElementState::Enabled => {
                let data = self
                    .execute_command(vec!["is".into(), "enabled".into(), value.into()])
                    .await?;
                scalar_bool(&data).ok_or_else(|| output_invalid("boolean"))
            }
            ElementState::Checked => {
                let native_type = self.ref_attribute(value, "type").await?;
                if native_type
                    .as_deref()
                    .is_some_and(|value| matches!(value, "checkbox" | "radio"))
                {
                    let data = self
                        .execute_command(vec!["is".into(), "checked".into(), value.into()])
                        .await?;
                    return scalar_bool(&data).ok_or_else(|| output_invalid("boolean"));
                }
                let aria_checked = self.ref_attribute(value, "aria-checked").await?;
                aria_boolean(aria_checked.as_deref())?.ok_or_else(|| state_unsupported("checked"))
            }
            ElementState::Selected => {
                let aria_selected = self.ref_attribute(value, "aria-selected").await?;
                aria_boolean(aria_selected.as_deref())?.ok_or_else(|| state_unsupported("selected"))
            }
            ElementState::Focused | ElementState::FocusWithin => Err(state_unsupported(
                "focus ownership through an observation-bound ref",
            )),
            ElementState::Expanded
            | ElementState::Pressed
            | ElementState::ReadOnly
            | ElementState::Required
            | ElementState::Invalid => Err(state_unsupported(
                "semantic state through an observation-bound ref",
            )),
        }
    }

    async fn ref_attribute(
        &self,
        reference: &str,
        name: &str,
    ) -> Result<Option<String>, DriverError> {
        let data = self
            .execute_command(vec![
                OsString::from("get"),
                OsString::from("attr"),
                OsString::from(reference),
                OsString::from(name),
            ])
            .await?;
        if let Some(value) = scalar_string(&data) {
            return Ok(Some(value.to_string()));
        }
        if data
            .pointer("/data/value")
            .or_else(|| data.get("value"))
            .is_some_and(Value::is_null)
        {
            return Ok(None);
        }
        Err(output_invalid("nullable attribute string"))
    }

    async fn probe_target(
        &self,
        target: &Target,
        probe: AssertionProbe,
    ) -> Result<Value, DriverError> {
        let data = self
            .execute_command(assertion_probe_args(target, probe)?)
            .await?;
        let result = browser_result(data);
        match result.get("status").and_then(Value::as_str) {
            Some("ok") => result
                .get("actual")
                .cloned()
                .ok_or_else(|| output_invalid("actual state")),
            Some("not_found") => Err(DriverError::new(
                "test.driver.web.target_not_found",
                "no browser element matched the assertion target",
            )),
            Some("ambiguous") => Err(DriverError::new(
                "test.driver.web.target_ambiguous",
                format!(
                    "{} browser elements matched the assertion target",
                    result.get("count").and_then(Value::as_u64).unwrap_or(0)
                ),
            )),
            Some("unsupported") => Err(DriverError::new(
                "test.driver.web.state_unsupported",
                "the matched browser element does not expose the requested assertion state",
            )),
            Some("invalid_target") => Err(DriverError::new(
                "test.driver.web.target_invalid",
                result
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("the browser rejected the assertion target"),
            )),
            Some("collection_limit") => Err(collection_limit(
                result.get("count").and_then(Value::as_u64),
            )),
            _ => Err(output_invalid("assertion probe envelope")),
        }
    }
}

fn viewport_coverage_expectation(
    comparison: ViewportCoverageComparison,
) -> (&'static str, &'static str) {
    match comparison {
        ViewportCoverageComparison::AtLeast => {
            ("at least", "test.assert.viewport_coverage_at_least")
        }
        ViewportCoverageComparison::AtMost => ("at most", "test.assert.viewport_coverage_at_most"),
    }
}

fn interactability_rects(value: &Value) -> Result<(LayoutRect, LayoutRect), DriverError> {
    match value.get("status").and_then(Value::as_str) {
        Some("ok") => Ok((
            layout_rect(value.get("target_rect"), "target")?,
            layout_rect(value.get("viewport_rect"), "visual viewport")?,
        )),
        Some("not_found") => Err(DriverError::new(
            "test.driver.web.target_not_found",
            "no browser element matched the interactability target",
        )),
        Some("ambiguous") => Err(DriverError::new(
            "test.driver.web.target_ambiguous",
            format!(
                "{} browser elements matched the interactability target",
                value.get("count").and_then(Value::as_u64).unwrap_or(0)
            ),
        )),
        Some("invalid_target") => Err(DriverError::new(
            "test.driver.web.target_invalid",
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the browser rejected the interactability target"),
        )),
        Some("invalid_geometry") => {
            let subject = match value.get("subject").and_then(Value::as_str) {
                Some("target") => "target",
                Some("viewport") => "visual viewport",
                _ => return Err(output_invalid("interactability geometry subject")),
            };
            Err(output_invalid(&format!(
                "{subject} interactability rectangle"
            )))
        }
        Some("unsupported") => Err(DriverError::new(
            "test.driver.web.interactability_unsupported",
            "the browser does not expose the required pointer hit-testing primitive",
        )),
        _ => Err(output_invalid("interactability probe envelope")),
    }
}

#[derive(Clone, Copy, Debug)]
struct PointerSample {
    x: f64,
    y: f64,
    reachable: bool,
}

fn pointer_samples(
    value: &Value,
    target_rect: LayoutRect,
    viewport_rect: LayoutRect,
) -> Result<Vec<PointerSample>, DriverError> {
    let values = value
        .get("samples")
        .and_then(Value::as_array)
        .ok_or_else(|| output_invalid("pointer sample array"))?;
    let ratio = target_rect
        .intersection_ratio(viewport_rect)
        .ok_or_else(|| output_invalid("pointer sample geometry"))?;
    if ratio == 0.0 {
        return if values.is_empty() {
            Ok(Vec::new())
        } else {
            Err(output_invalid("empty offscreen pointer sample array"))
        };
    }
    if values.len() != POINTER_SAMPLE_COUNT {
        return Err(output_invalid(&format!(
            "exactly {POINTER_SAMPLE_COUNT} pointer samples"
        )));
    }

    let left = target_rect.x.max(viewport_rect.x);
    let top = target_rect.y.max(viewport_rect.y);
    let right = (target_rect.x + target_rect.width).min(viewport_rect.x + viewport_rect.width);
    let bottom = (target_rect.y + target_rect.height).min(viewport_rect.y + viewport_rect.height);
    let fractions = [1.0 / 6.0, 0.5, 5.0 / 6.0];
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = value
                .get("x")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .ok_or_else(|| output_invalid("finite pointer sample x coordinate"))?;
            let y = value
                .get("y")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .ok_or_else(|| output_invalid("finite pointer sample y coordinate"))?;
            let reachable = value
                .get("reachable")
                .and_then(Value::as_bool)
                .ok_or_else(|| output_invalid("pointer sample reachability boolean"))?;
            let expected_x = left + (right - left) * fractions[index % 3];
            let expected_y = top + (bottom - top) * fractions[index / 3];
            let scale = expected_x.abs().max(expected_y.abs()).max(1.0);
            let tolerance = scale * f64::EPSILON * 16.0;
            if (x - expected_x).abs() > tolerance || (y - expected_y).abs() > tolerance {
                return Err(output_invalid("deterministic 3 by 3 pointer sample grid"));
            }
            Ok(PointerSample { x, y, reachable })
        })
        .collect()
}

fn layout_rects(value: &Value) -> Result<(LayoutRect, LayoutRect), DriverError> {
    match value.get("status").and_then(Value::as_str) {
        Some("ok") => Ok((
            layout_rect(value.get("target_rect"), "target")?,
            layout_rect(value.get("relative_rect"), "relative_to")?,
        )),
        Some("not_found") => {
            let subject = layout_subject(value)?;
            Err(DriverError::new(
                "test.driver.web.target_not_found",
                format!("no browser element matched the layout {subject}"),
            ))
        }
        Some("ambiguous") => {
            let subject = layout_subject(value)?;
            Err(DriverError::new(
                "test.driver.web.target_ambiguous",
                format!(
                    "{} browser elements matched the layout {subject}",
                    value.get("count").and_then(Value::as_u64).unwrap_or(0)
                ),
            ))
        }
        Some("invalid_target") => {
            let subject = layout_subject(value)?;
            Err(DriverError::new(
                "test.driver.web.target_invalid",
                format!(
                    "layout {subject}: {}",
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("the browser rejected the locator")
                ),
            ))
        }
        Some("invalid_geometry") => {
            let subject = layout_subject(value)?;
            Err(output_invalid(&format!("{subject} layout rectangle")))
        }
        _ => Err(output_invalid("layout probe envelope")),
    }
}

fn layout_subject(value: &Value) -> Result<&'static str, DriverError> {
    match value.get("subject").and_then(Value::as_str) {
        Some("target") => Ok("target"),
        Some("relative_to") => Ok("relative_to target"),
        _ => Err(output_invalid("layout probe subject")),
    }
}

fn layout_rect(value: Option<&Value>, subject: &str) -> Result<LayoutRect, DriverError> {
    let value = value.ok_or_else(|| output_invalid(&format!("{subject} layout rectangle")))?;
    let number = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_f64)
            .ok_or_else(|| output_invalid(&format!("{subject} layout rectangle '{name}'")))
    };
    let rect = LayoutRect {
        x: number("x")?,
        y: number("y")?,
        width: number("width")?,
        height: number("height")?,
    };
    if !rect.is_valid() {
        return Err(output_invalid(&format!("{subject} layout rectangle")));
    }
    Ok(rect)
}

fn rect_data(rect: LayoutRect) -> Value {
    json!({
        "x": rect.x,
        "y": rect.y,
        "width": rect.width,
        "height": rect.height,
    })
}

fn rendered_text_values(value: &Value) -> Result<Vec<String>, DriverError> {
    let values = value
        .as_array()
        .ok_or_else(|| output_invalid("rendered text array"))?;
    if values.len() > MAX_RENDERED_TEXT_ITEMS {
        return Err(collection_limit(u64::try_from(values.len()).ok()));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(normalize_rendered_text)
                .ok_or_else(|| output_invalid("rendered text array"))
        })
        .collect()
}

fn collection_limit(count: Option<u64>) -> DriverError {
    let observed = count.map_or_else(
        || "more than the supported limit".to_string(),
        |count| format!("{count} items"),
    );
    DriverError::new(
        "test.driver.web.collection_limit",
        format!("rendered_texts observed {observed}; the maximum is {MAX_RENDERED_TEXT_ITEMS}"),
    )
}

fn aria_boolean(value: Option<&str>) -> Result<Option<bool>, DriverError> {
    match value {
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        None => Ok(None),
        Some(_) => Err(DriverError::new(
            "test.driver.web.state_unsupported",
            "the ARIA state is not a supported boolean value",
        )),
    }
}

fn canonical_values(values: &[String], error_kind: &str) -> Result<Vec<String>, DriverError> {
    let selected = values.iter().cloned().collect::<BTreeSet<_>>();
    if selected.len() != values.len() {
        return Err(DriverError::new(
            format!("test.driver.web.{error_kind}"),
            "selected values cannot contain duplicates",
        ));
    }
    Ok(selected.into_iter().collect())
}

fn normalize_rendered_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn state_expectation(state: ElementState, expected: bool) -> (&'static str, &'static str) {
    match (state, expected) {
        (ElementState::Enabled, true) => ("enabled", "test.assert.enabled"),
        (ElementState::Enabled, false) => ("disabled", "test.assert.disabled"),
        (ElementState::Checked, true) => ("checked", "test.assert.checked"),
        (ElementState::Checked, false) => ("unchecked", "test.assert.unchecked"),
        (ElementState::Selected, true) => ("selected", "test.assert.selected"),
        (ElementState::Selected, false) => ("unselected", "test.assert.unselected"),
        (ElementState::Focused, true) => ("focused", "test.assert.focused"),
        (ElementState::Focused, false) => ("unfocused", "test.assert.unfocused"),
        (ElementState::FocusWithin, true) => ("focus_within", "test.assert.focus_within"),
        (ElementState::FocusWithin, false) => ("focus_outside", "test.assert.focus_outside"),
        (ElementState::Expanded, true) => ("expanded", "test.assert.expanded"),
        (ElementState::Expanded, false) => ("collapsed", "test.assert.collapsed"),
        (ElementState::Pressed, true) => ("pressed", "test.assert.pressed"),
        (ElementState::Pressed, false) => ("unpressed", "test.assert.unpressed"),
        (ElementState::ReadOnly, true) => ("readonly", "test.assert.readonly"),
        (ElementState::ReadOnly, false) => ("writable", "test.assert.writable"),
        (ElementState::Required, true) => ("required", "test.assert.required"),
        (ElementState::Required, false) => ("optional", "test.assert.optional"),
        (ElementState::Invalid, true) => ("invalid", "test.assert.invalid"),
        (ElementState::Invalid, false) => ("valid", "test.assert.valid"),
    }
}

fn element_state_name(state: ElementState) -> &'static str {
    match state {
        ElementState::Enabled => "enabled",
        ElementState::Checked => "checked",
        ElementState::Selected => "selected",
        ElementState::Focused => "focused",
        ElementState::FocusWithin => "focus_within",
        ElementState::Expanded => "expanded",
        ElementState::Pressed => "pressed",
        ElementState::ReadOnly => "readonly",
        ElementState::Required => "required",
        ElementState::Invalid => "invalid",
    }
}

fn state_unsupported(name: &str) -> DriverError {
    DriverError::new(
        "test.driver.web.state_unsupported",
        format!("the current browser ref does not expose a boolean {name} state"),
    )
}

fn output_invalid(expected: &str) -> DriverError {
    DriverError::new(
        "test.driver.web.output_invalid",
        format!("browser state response did not contain a valid {expected}"),
    )
}
