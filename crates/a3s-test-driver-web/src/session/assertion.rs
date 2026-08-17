use std::collections::BTreeSet;
use std::ffi::OsString;

use a3s_test_core::{DriverError, ElementState, Expectation, StepOutput, Target};
use serde_json::{json, Value};

use crate::protocol::{
    assertion_probe_args, scalar_bool, scalar_string, visibility_args, AssertionProbe,
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
            Expectation::State {
                target,
                state,
                expected,
            } => self.assert_state(target, *state, *expected).await,
            Expectation::Value { target, value } => self.assert_value(target, value).await,
            Expectation::SelectedValues { target, values } => {
                self.assert_selected_values(target, values).await
            }
        }
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
                "no browser element matched the state assertion target",
            )),
            Some("ambiguous") => Err(DriverError::new(
                "test.driver.web.target_ambiguous",
                format!(
                    "{} browser elements matched the state assertion target",
                    result.get("count").and_then(Value::as_u64).unwrap_or(0)
                ),
            )),
            Some("unsupported") => Err(DriverError::new(
                "test.driver.web.state_unsupported",
                "the matched browser element does not expose the requested state",
            )),
            Some("invalid_target") => Err(DriverError::new(
                "test.driver.web.target_invalid",
                result
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("the browser rejected the state assertion target"),
            )),
            _ => Err(output_invalid("state probe envelope")),
        }
    }
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

fn state_expectation(state: ElementState, expected: bool) -> (&'static str, &'static str) {
    match (state, expected) {
        (ElementState::Enabled, true) => ("enabled", "test.assert.enabled"),
        (ElementState::Enabled, false) => ("disabled", "test.assert.disabled"),
        (ElementState::Checked, true) => ("checked", "test.assert.checked"),
        (ElementState::Checked, false) => ("unchecked", "test.assert.unchecked"),
        (ElementState::Selected, true) => ("selected", "test.assert.selected"),
        (ElementState::Selected, false) => ("unselected", "test.assert.unselected"),
    }
}

fn element_state_name(state: ElementState) -> &'static str {
    match state {
        ElementState::Enabled => "enabled",
        ElementState::Checked => "checked",
        ElementState::Selected => "selected",
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
