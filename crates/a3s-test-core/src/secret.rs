//! Admission for an environment-backed fill.
//!
//! The secret value is not an argument. Callers resolve it only after this
//! function accepts the sink.

use crate::SecretPurpose;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretSink {
    pub enabled: bool,
    pub readonly: bool,
    pub content_editable: bool,
    pub input_type: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretSinkError {
    Disabled,
    ReadOnly,
    NotEditable,
    PurposeMismatch,
}

const EDITABLE_TYPES: &[&str] = &[
    "text", "search", "tel", "url", "email", "password", "number",
];

pub fn admit_secret_sink(purpose: SecretPurpose, sink: &SecretSink) -> Result<(), SecretSinkError> {
    if !sink.enabled {
        return Err(SecretSinkError::Disabled);
    }
    if sink.readonly {
        return Err(SecretSinkError::ReadOnly);
    }
    let type_editable = sink
        .input_type
        .as_deref()
        .is_some_and(|value| EDITABLE_TYPES.contains(&value));
    if !sink.content_editable && !type_editable {
        return Err(SecretSinkError::NotEditable);
    }
    if purpose == SecretPurpose::Password && sink.input_type.as_deref() != Some("password") {
        return Err(SecretSinkError::PurposeMismatch);
    }
    Ok(())
}

/// Remove a resolved secret from a browser error. The name stays.
#[must_use]
pub fn redact_secret_text(message: &str, secret: &str) -> String {
    if secret.is_empty() {
        return message.to_string();
    }
    message.replace(secret, "[redacted]")
}
