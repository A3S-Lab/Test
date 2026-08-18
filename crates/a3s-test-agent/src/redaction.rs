use std::collections::BTreeSet;
use std::fmt;

use a3s_test_core::{
    Action, CaptureOperation, DialogOperation, Evidence, Expectation, FrameTarget, NetworkRoute,
    StepOutput, SurfaceObservation, TabOperation, Target, VideoOperation, WaitCondition,
};
use serde_json::{Map, Value};

use crate::{AgentDecision, AgentError, AgentRunResult, AgentTurn};

pub const REDACTED_VALUE: &str = "[REDACTED]";

const MAX_EXACT_SECRETS: usize = 256;
const MAX_SECRET_BYTES: usize = 16 * 1_024;
const MAX_TOTAL_SECRET_BYTES: usize = 1024 * 1_024;

/// Redacts sensitive values from the serializable agent provenance trace.
///
/// Common credential-shaped JSON keys and secret-bearing action payloads are
/// always removed. Hosts should additionally register every runtime secret
/// that may appear in unstructured observations, summaries, paths, request
/// identifiers, or provider errors.
#[derive(Clone)]
pub struct ProvenanceRedactor {
    exact_secrets: Vec<String>,
    replacement: String,
}

impl ProvenanceRedactor {
    /// Builds a redactor that also removes the supplied exact secret values.
    ///
    /// Values are treated as case-sensitive substrings. They are kept private
    /// and omitted from `Debug` output.
    pub fn from_exact_secrets<I, S>(secrets: I) -> Result<Self, AgentError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut unique = BTreeSet::new();
        let mut total_bytes = 0usize;
        for secret in secrets {
            let secret = secret.into();
            if secret.is_empty() {
                return Err(redaction_config_error(
                    "exact provenance secrets must not be empty",
                ));
            }
            if secret.len() > MAX_SECRET_BYTES {
                return Err(redaction_config_error(format!(
                    "an exact provenance secret exceeds {MAX_SECRET_BYTES} bytes"
                )));
            }
            if unique.insert(secret.clone()) {
                total_bytes = total_bytes.saturating_add(secret.len());
                if unique.len() > MAX_EXACT_SECRETS {
                    return Err(redaction_config_error(format!(
                        "at most {MAX_EXACT_SECRETS} exact provenance secrets may be registered"
                    )));
                }
                if total_bytes > MAX_TOTAL_SECRET_BYTES {
                    return Err(redaction_config_error(format!(
                        "exact provenance secrets exceed the {MAX_TOTAL_SECRET_BYTES} byte aggregate limit"
                    )));
                }
            }
        }

        let mut exact_secrets = unique.into_iter().collect::<Vec<_>>();
        exact_secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
        let replacement = safe_replacement(&exact_secrets);
        Ok(Self {
            exact_secrets,
            replacement,
        })
    }

    /// Applies the same credential-key and exact-secret policy to a host-owned
    /// JSON report that contains an [`AgentRunResult`] or adjacent metadata.
    pub fn redact_json(&self, value: &mut Value) {
        self.redact_value(value);
    }

    /// Redact configured secrets and sensitive URL components in host-owned
    /// unstructured diagnostics before they are persisted or emitted.
    #[must_use]
    pub fn redacted_text(&self, value: impl Into<String>) -> String {
        let mut value = value.into();
        self.redact_text(&mut value);
        value
    }

    pub(crate) fn redact_result(&self, mut result: AgentRunResult) -> AgentRunResult {
        self.redact_text(&mut result.provider.provider);
        self.redact_text(&mut result.provider.model);
        self.redact_text(&mut result.prompt_version);
        if let Some(summary) = result.summary.as_mut() {
            self.redact_text(summary);
        }
        for turn in &mut result.turns {
            self.redact_turn(turn);
        }
        if let Some(error) = result.error.as_mut() {
            self.redact_error(error);
        }
        result
    }

    fn redact_turn(&self, turn: &mut AgentTurn) {
        self.redact_observation(&mut turn.observation);
        if let Some(decision) = turn.decision.as_mut() {
            self.redact_decision(decision);
        }
        if let Some(request_id) = turn.request_id.as_mut() {
            self.redact_text(request_id);
        }
        if let Some(output) = turn.output.as_mut() {
            self.redact_output(output);
        }
        if let Some(error) = turn.error.as_mut() {
            self.redact_error(error);
        }
    }

    fn redact_decision(&self, decision: &mut AgentDecision) {
        match decision {
            AgentDecision::Act { action } => self.redact_action(action),
            AgentDecision::Finish { summary } => self.redact_text(summary),
            AgentDecision::Fail { reason } => self.redact_text(reason),
        }
    }

    fn redact_action(&self, action: &mut Action) {
        match action {
            Action::Navigate { url } => self.redact_url(url),
            Action::Snapshot { .. } | Action::Viewport { .. } | Action::TerminalResize { .. } => {}
            Action::VerifyContract {
                contract,
                variant,
                state,
            } => {
                self.redact_text(contract);
                self.redact_text(variant);
                self.redact_text(state);
            }
            Action::Click { target }
            | Action::Hover { target }
            | Action::Focus { target }
            | Action::DoubleClick { target }
            | Action::ContextClick { target }
            | Action::Check { target }
            | Action::Uncheck { target } => self.redact_target(target),
            Action::Fill { target, value } | Action::Type { target, value } => {
                self.redact_target(target);
                value.clone_from(&self.replacement);
            }
            Action::InsertText { value } => value.clone_from(&self.replacement),
            Action::Select { target, values } => {
                self.redact_target(target);
                for value in values {
                    value.clone_from(&self.replacement);
                }
            }
            Action::Drag { source, target } => {
                self.redact_target(source);
                self.redact_target(target);
            }
            Action::Press { key } => self.redact_text(key),
            Action::TerminalPaste { text } => text.clone_from(&self.replacement),
            Action::TerminalRecording { path } => self.redact_text(path),
            Action::Wheel { target, .. } => {
                if let Some(target) = target {
                    self.redact_target(target);
                }
            }
            Action::Wait { condition } => self.redact_wait(condition),
            Action::Assert { expectation } => self.redact_expectation(expectation),
            Action::Screenshot { path }
            | Action::Accessibility { path, .. }
            | Action::Console { path, .. }
            | Action::PageErrors { path, .. } => self.redact_text(path),
            Action::Tab { operation } => self.redact_tab(operation),
            Action::Frame { target } => self.redact_frame(target),
            Action::Dialog { operation } => self.redact_dialog(operation),
            Action::Upload { target, paths } => {
                self.redact_target(target);
                for path in paths {
                    self.redact_text(path);
                }
            }
            Action::Download { target, path } => {
                self.redact_target(target);
                self.redact_text(path);
            }
            Action::NetworkRoute { pattern, route } => {
                self.redact_text(pattern);
                if let NetworkRoute::Body(body) = route {
                    body.clone_from(&self.replacement);
                }
            }
            Action::NetworkUnroute { pattern } => {
                if let Some(pattern) = pattern {
                    self.redact_text(pattern);
                }
            }
            Action::Har { operation } | Action::Trace { operation } => {
                self.redact_capture(operation);
            }
            Action::Video { operation } => self.redact_video(operation),
        }
    }

    fn redact_target(&self, target: &mut Target) {
        match target {
            Target::Ref { value }
            | Target::Text { value, .. }
            | Target::AutomationId { value }
            | Target::TestId { value }
            | Target::Label { value }
            | Target::Placeholder { value } => self.redact_text(value),
            Target::Css { selector } => self.redact_text(selector),
            Target::Role { role, name } => {
                self.redact_text(role);
                self.redact_text(name);
            }
            Target::VisualPoint { snapshot, .. } => self.redact_text(snapshot),
        }
    }

    fn redact_wait(&self, condition: &mut WaitCondition) {
        match condition {
            WaitCondition::Load(_) => {}
            WaitCondition::Text(text) => self.redact_text(text),
            WaitCondition::Regex(regex) => self.redact_text(regex),
            WaitCondition::Url(url) => self.redact_url(url),
            WaitCondition::Visible(target) => self.redact_target(target),
        }
    }

    fn redact_expectation(&self, expectation: &mut Expectation) {
        match expectation {
            Expectation::TextVisible(text) => self.redact_text(text),
            Expectation::Url(url) => self.redact_url(url),
            Expectation::Visible(target)
            | Expectation::InViewport(target)
            | Expectation::ViewportCoverage { target, .. }
            | Expectation::PointerReachable(target) => self.redact_target(target),
            Expectation::RenderedText { target, value } => {
                self.redact_target(target);
                self.redact_text(value);
            }
            Expectation::RenderedTexts { target, values } => {
                self.redact_target(target);
                for value in values {
                    self.redact_text(value);
                }
            }
            Expectation::VisibleCount { target, .. } => self.redact_target(target),
            Expectation::State { target, .. } => self.redact_target(target),
            Expectation::Value { target, value } => {
                self.redact_target(target);
                self.redact_text(value);
            }
            Expectation::SelectedValues { target, values } => {
                self.redact_target(target);
                for value in values {
                    self.redact_text(value);
                }
            }
            Expectation::Layout {
                target,
                relative_to,
                ..
            } => {
                self.redact_target(target);
                self.redact_target(relative_to);
            }
        }
    }

    fn redact_tab(&self, operation: &mut TabOperation) {
        match operation {
            TabOperation::List => {}
            TabOperation::New { url, label } => {
                if let Some(url) = url {
                    self.redact_url(url);
                }
                if let Some(label) = label {
                    self.redact_text(label);
                }
            }
            TabOperation::Switch { tab } => self.redact_text(tab),
            TabOperation::Close { tab } => {
                if let Some(tab) = tab {
                    self.redact_text(tab);
                }
            }
        }
    }

    fn redact_frame(&self, target: &mut FrameTarget) {
        if let FrameTarget::Selector(selector) = target {
            self.redact_text(selector);
        }
    }

    fn redact_dialog(&self, operation: &mut DialogOperation) {
        if let DialogOperation::Accept { text: Some(text) } = operation {
            text.clone_from(&self.replacement);
        }
    }

    fn redact_capture(&self, operation: &mut CaptureOperation) {
        if let CaptureOperation::Stop { path } = operation {
            self.redact_text(path);
        }
    }

    fn redact_video(&self, operation: &mut VideoOperation) {
        if let VideoOperation::Start { path, url } = operation {
            self.redact_text(path);
            if let Some(url) = url {
                self.redact_url(url);
            }
        }
    }

    fn redact_observation(&self, observation: &mut SurfaceObservation) {
        self.redact_text(&mut observation.summary);
        self.redact_value(&mut observation.data);
        self.redact_evidence(&mut observation.evidence);
        if let Some(page_context) = observation.page_context.as_mut() {
            self.redact_page_context(page_context);
        }
    }

    fn redact_output(&self, output: &mut StepOutput) {
        self.redact_text(&mut output.summary);
        self.redact_value(&mut output.data);
        self.redact_evidence(&mut output.evidence);
        if let Some(page_context) = output.page_context.as_mut() {
            self.redact_page_context(page_context);
        }
    }

    fn redact_page_context(&self, page_context: &mut a3s_test_core::PageContextObservation) {
        let Ok(mut value) = serde_json::to_value(&*page_context) else {
            return;
        };
        self.redact_value(&mut value);
        if let Ok(redacted) = serde_json::from_value(value) {
            *page_context = redacted;
        }
    }

    fn redact_evidence(&self, evidence: &mut [Evidence]) {
        for item in evidence {
            self.redact_text(&mut item.name);
            self.redact_text(&mut item.path);
            self.redact_text(&mut item.media_type);
        }
    }

    fn redact_error(&self, error: &mut AgentError) {
        self.redact_text(&mut error.code);
        self.redact_text(&mut error.message);
    }

    fn redact_url(&self, url: &mut String) {
        if let Ok(mut parsed) = url::Url::parse(url) {
            let has_sensitive_components = !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some();
            if has_sensitive_components {
                let _ = parsed.set_username("");
                let _ = parsed.set_password(None);
                parsed.set_query(None);
                parsed.set_fragment(None);
                *url = parsed.into();
            }
        }
        self.redact_text(url);
    }

    fn redact_value(&self, value: &mut Value) {
        match value {
            Value::String(text) => self.redact_text(text),
            Value::Array(values) => {
                for value in values {
                    self.redact_value(value);
                }
            }
            Value::Object(object) => self.redact_object(object),
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    fn redact_object(&self, object: &mut Map<String, Value>) {
        let has_sensitive_label = object.iter().any(|(key, value)| {
            matches!(
                normalized_key(key).as_str(),
                "name" | "key" | "header" | "type" | "label" | "autocomplete"
            ) && value.as_str().is_some_and(sensitive_key)
        });
        let values = std::mem::take(object);
        for (mut key, mut value) in values {
            let normalized = normalized_key(&key);
            let sensitive = sensitive_key(&key)
                || (has_sensitive_label
                    && matches!(normalized.as_str(), "value" | "text" | "content"));
            self.redact_text(&mut key);
            if sensitive {
                value = Value::String(self.replacement.clone());
            } else if normalized.ends_with("url") {
                if let Value::String(url) = &mut value {
                    self.redact_url(url);
                } else {
                    self.redact_value(&mut value);
                }
            } else {
                self.redact_value(&mut value);
            }
            object.insert(key, value);
        }
    }

    fn redact_text(&self, text: &mut String) {
        for secret in &self.exact_secrets {
            if text.contains(secret) {
                *text = text.replace(secret, &self.replacement);
            }
        }

        // A replacement can create a registered value across a marker boundary.
        // Removing any such residual match is finite because each pass shrinks
        // the string and the selected marker never contains a registered secret.
        while self
            .exact_secrets
            .iter()
            .any(|secret| text.contains(secret))
        {
            for secret in &self.exact_secrets {
                *text = text.replace(secret, "");
            }
        }
    }
}

impl Default for ProvenanceRedactor {
    fn default() -> Self {
        Self {
            exact_secrets: Vec::new(),
            replacement: REDACTED_VALUE.to_string(),
        }
    }
}

impl fmt::Debug for ProvenanceRedactor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvenanceRedactor")
            .field("exact_secret_count", &self.exact_secrets.len())
            .finish_non_exhaustive()
    }
}

fn sensitive_key(key: &str) -> bool {
    let normalized = normalized_key(key);
    matches!(
        normalized.as_str(),
        "password"
            | "currentpassword"
            | "newpassword"
            | "passwd"
            | "passphrase"
            | "credential"
            | "credentials"
            | "secret"
            | "secretkey"
            | "clientsecret"
            | "apikey"
            | "xapikey"
            | "auth"
            | "authentication"
            | "authorization"
            | "proxyauthorization"
            | "xauthtoken"
            | "cookie"
            | "setcookie"
            | "accesstoken"
            | "accesskey"
            | "accesskeyid"
            | "refreshtoken"
            | "idtoken"
            | "sessiontoken"
            | "sessionid"
            | "csrftoken"
            | "xcsrftoken"
            | "connectionstring"
            | "databaseurl"
            | "privatekey"
            | "secretaccesskey"
    )
}

fn normalized_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn redaction_config_error(message: impl Into<String>) -> AgentError {
    AgentError::new("test.agent.provenance_redaction_invalid", message)
}

fn safe_replacement(exact_secrets: &[String]) -> String {
    [REDACTED_VALUE, "[FILTERED]", "<hidden>", "***"]
        .into_iter()
        .find(|candidate| {
            exact_secrets
                .iter()
                .all(|secret| !candidate.contains(secret))
        })
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
#[path = "redaction_tests.rs"]
mod tests;
