//! Environment-backed fill. The value is resolved only after the name is
//! present, and only a passing sink receives it.

use std::ffi::OsString;

use a3s_test_core::{
    admit_secret_sink, redact_secret_text, DriverError, SecretPurpose, SecretSink, SecretSinkError,
    StepOutput, Target,
};
use serde_json::{json, Value};

use super::browser_result;
use crate::protocol::{direct_selector, scalar_bool, scalar_string, target_action};

use super::AgentBrowserSession;

impl AgentBrowserSession {
    pub(super) async fn fill_secret(
        &mut self,
        target: &Target,
        name: &str,
        purpose: SecretPurpose,
        type_secret: bool,
    ) -> Result<StepOutput, DriverError> {
        let secret = resolve_secret(name)?;
        let selector = direct_selector(target)?;
        let sink = self.probe_secret_sink(selector).await?;
        if let Err(error) = admit_secret_sink(purpose, &sink) {
            return Err(sink_error(error));
        }
        let verb = if type_secret { "type" } else { "fill" };
        let args = target_action(target, verb, Some(&secret))?;
        self.secret_exposed = true;
        match self.execute_command(args).await {
            Ok(_) => Ok(StepOutput::new(if type_secret {
                "secret typed into target"
            } else {
                "secret filled"
            })
            .with_data(json!({ "env": name, "filled": true }))),
            Err(error) => Err(redact_driver_error(error, &secret)),
        }
    }

    async fn probe_secret_sink(&self, selector: &str) -> Result<SecretSink, DriverError> {
        let enabled = self
            .browser_flag(vec![
                OsString::from("is"),
                OsString::from("enabled"),
                OsString::from(selector),
            ])
            .await?;
        let readonly = self.browser_attr(selector, "readonly", true).await?;
        let content_editable = self
            .browser_attr(selector, "contenteditable", false)
            .await?;
        let input_type = self.browser_attr(selector, "type", false).await?;
        Ok(SecretSink {
            enabled,
            readonly: readonly_attribute(readonly.as_deref()),
            content_editable: content_editable.as_deref() == Some("true"),
            input_type,
        })
    }

    async fn browser_flag(&self, args: Vec<OsString>) -> Result<bool, DriverError> {
        let data = self.execute_command(args).await?;
        probed_bool(&data)
    }

    async fn browser_attr(
        &self,
        selector: &str,
        name: &str,
        keep_empty: bool,
    ) -> Result<Option<String>, DriverError> {
        let data = self
            .execute_command(vec![
                OsString::from("get"),
                OsString::from("attr"),
                OsString::from(selector),
                OsString::from(name),
            ])
            .await?;
        Ok(probed_string(&data).filter(|value| keep_empty || !value.is_empty()))
    }
}

/// `<input readonly>` is a boolean attribute. Any present value is read-only,
/// including `""`, `"readonly"`, `"true"`, and `"false"`.
/// An absent attribute is null, not an empty string.
fn readonly_attribute(value: Option<&str>) -> bool {
    value.is_some()
}

fn resolve_secret(name: &str) -> Result<String, DriverError> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(DriverError::new(
            "test.driver.web.secret_unavailable",
            format!("environment variable '{name}' is unset or empty"),
        )),
    }
}

fn sink_error(error: SecretSinkError) -> DriverError {
    let (code, message) = match error {
        SecretSinkError::Disabled => (
            "test.driver.web.secret_sink_disabled",
            "secret target is disabled",
        ),
        SecretSinkError::ReadOnly => (
            "test.driver.web.secret_sink_readonly",
            "secret target is read-only",
        ),
        SecretSinkError::NotEditable => (
            "test.driver.web.secret_sink_not_editable",
            "secret target is not an editable text control",
        ),
        SecretSinkError::PurposeMismatch => (
            "test.driver.web.secret_purpose_mismatch",
            "password purpose requires a password control",
        ),
    };
    DriverError::new(code, message)
}

fn redact_driver_error(error: DriverError, secret: &str) -> DriverError {
    DriverError::new(error.code(), redact_secret_text(error.message(), secret))
        .with_retryable(error.retryable())
}

fn probed_bool(data: &Value) -> Result<bool, DriverError> {
    let unwrapped = browser_result(data.clone());
    scalar_bool(&unwrapped)
        .or_else(|| scalar_bool(data))
        .ok_or_else(|| {
            DriverError::new(
                "test.driver.web.output_invalid",
                "browser boolean response did not contain a boolean",
            )
        })
}

fn probed_string(data: &Value) -> Option<String> {
    let unwrapped = browser_result(data.clone());
    scalar_string(&unwrapped)
        .or_else(|| scalar_string(data))
        .map(str::to_string)
}
