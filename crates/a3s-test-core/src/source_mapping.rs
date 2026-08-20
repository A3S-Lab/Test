use std::collections::HashSet;
use std::fmt::{Display, Formatter};

use crate::{PageContextSource, PageContextSourceMapping, SOURCE_MAPPING_PROTOCOL};

const MAX_CANDIDATES: usize = 8;
const MAX_FILE_BYTES: usize = 2_048;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_POSITION: u32 = 10_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMappingValidationError {
    message: String,
}

impl SourceMappingValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for SourceMappingValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SourceMappingValidationError {}

impl PageContextSourceMapping {
    pub fn validate(&self) -> Result<(), SourceMappingValidationError> {
        if self.protocol != SOURCE_MAPPING_PROTOCOL {
            return Err(SourceMappingValidationError::new(
                "source mapping protocol is unsupported",
            ));
        }
        if self.candidates.is_empty() || self.candidates.len() > MAX_CANDIDATES {
            return Err(SourceMappingValidationError::new(format!(
                "source mapping must contain between 1 and {MAX_CANDIDATES} candidates"
            )));
        }
        if self.truncated && self.candidates.len() != MAX_CANDIDATES {
            return Err(SourceMappingValidationError::new(format!(
                "truncated source mapping must contain exactly {MAX_CANDIDATES} candidates"
            )));
        }

        let mut previous_confidence = f64::INFINITY;
        let mut spans = HashSet::new();
        for candidate in &self.candidates {
            validate_span(&candidate.span, "source span")?;
            if let Some(generated) = &candidate.generated_span {
                validate_span(generated, "generated source span")?;
            }
            if !candidate.confidence.is_finite() || !(0.0..=1.0).contains(&candidate.confidence) {
                return Err(SourceMappingValidationError::new(
                    "source mapping confidence must be finite and between 0 and 1",
                ));
            }
            if candidate.confidence > previous_confidence {
                return Err(SourceMappingValidationError::new(
                    "source mapping candidates must be ranked by descending confidence",
                ));
            }
            previous_confidence = candidate.confidence;
            if !bounded_label(&candidate.registration_id, MAX_IDENTIFIER_BYTES) {
                return Err(SourceMappingValidationError::new(
                    "source mapping registration ID is invalid",
                ));
            }
            if candidate
                .component_id
                .as_deref()
                .is_some_and(|value| !bounded_label(value, MAX_IDENTIFIER_BYTES))
            {
                return Err(SourceMappingValidationError::new(
                    "source mapping component ID is invalid",
                ));
            }
            if candidate
                .framework
                .as_deref()
                .is_some_and(|value| !bounded_label(value, 64))
            {
                return Err(SourceMappingValidationError::new(
                    "source mapping framework is invalid",
                ));
            }
            let key = (
                candidate.span.file.as_str(),
                candidate.span.line,
                candidate.span.column,
                candidate.span.end_line,
                candidate.span.end_column,
            );
            if !spans.insert(key) {
                return Err(SourceMappingValidationError::new(
                    "source mapping candidates contain a duplicate span",
                ));
            }
        }
        Ok(())
    }
}

fn validate_span(
    span: &PageContextSource,
    label: &str,
) -> Result<(), SourceMappingValidationError> {
    if span.file.is_empty()
        || span.file.len() > MAX_FILE_BYTES
        || span.file.chars().any(char::is_control)
    {
        return Err(SourceMappingValidationError::new(format!(
            "{label} file is invalid"
        )));
    }
    for (field, value) in [
        ("line", span.line),
        ("column", span.column),
        ("end line", span.end_line),
        ("end column", span.end_column),
    ] {
        if value.is_some_and(|value| value == 0 || value > MAX_POSITION) {
            return Err(SourceMappingValidationError::new(format!(
                "{label} {field} is invalid"
            )));
        }
    }
    if span.column.is_some() && span.line.is_none() {
        return Err(SourceMappingValidationError::new(format!(
            "{label} column requires a line"
        )));
    }
    if span.end_line.is_some() && span.line.is_none() {
        return Err(SourceMappingValidationError::new(format!(
            "{label} end line requires a line"
        )));
    }
    if span.end_column.is_some() && span.end_line.is_none() {
        return Err(SourceMappingValidationError::new(format!(
            "{label} end column requires an end line"
        )));
    }
    if let (Some(line), Some(end_line)) = (span.line, span.end_line) {
        let starts_after_end = end_line < line
            || (end_line == line && span.end_column.unwrap_or(1) < span.column.unwrap_or(1));
        if starts_after_end {
            return Err(SourceMappingValidationError::new(format!(
                "{label} end precedes its start"
            )));
        }
    }
    Ok(())
}

fn bounded_label(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}
