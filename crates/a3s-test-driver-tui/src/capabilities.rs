use std::collections::BTreeSet;

use a3s_test_core::DriverError;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::config::{
    MAX_TUI_COLUMNS, MAX_TUI_OUTPUT_BYTES, MAX_TUI_ROWS, MAX_TUI_SCROLLBACK_ROWS,
    MAX_TUI_TERMINAL_CELLS,
};

pub const TUI_CAPABILITY_PROTOCOL: &str = "a3s.test.driver-tui/1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TuiBackend {
    UnixPty,
    WindowsConPty,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TuiFeature {
    AlternateScreen,
    KeyChords,
    OwnedProcessTree,
    Paste,
    RegexWaits,
    Resize,
    SemanticViewport,
    TerminalRecording,
    TextWaits,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuiCapabilityLimits {
    pub max_columns: u16,
    pub max_rows: u16,
    pub max_scrollback_rows: u64,
    pub max_output_bytes: u64,
    pub max_terminal_cells: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuiCapabilities {
    #[schemars(schema_with = "tui_capability_protocol_schema")]
    pub protocol: String,
    pub backend: TuiBackend,
    pub features: BTreeSet<TuiFeature>,
    pub limits: TuiCapabilityLimits,
}

fn tui_capability_protocol_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": TUI_CAPABILITY_PROTOCOL
    })
}

impl TuiCapabilities {
    pub fn compiled() -> Result<Self, DriverError> {
        let capabilities = Self {
            protocol: TUI_CAPABILITY_PROTOCOL.to_string(),
            backend: compiled_backend()?,
            features: expected_features(),
            limits: expected_limits(),
        };
        capabilities.validate()?;
        Ok(capabilities)
    }

    pub fn validate(&self) -> Result<(), DriverError> {
        if self.protocol != TUI_CAPABILITY_PROTOCOL {
            return Err(capability_error(format!(
                "unsupported TUI capability protocol {:?}",
                self.protocol
            )));
        }
        if self.features != expected_features() {
            return Err(capability_error(
                "TUI feature projection does not match the reviewed protocol",
            ));
        }
        if self.limits != expected_limits() {
            return Err(capability_error(
                "TUI limits do not match the reviewed protocol",
            ));
        }
        Ok(())
    }
}

fn expected_features() -> BTreeSet<TuiFeature> {
    [
        TuiFeature::AlternateScreen,
        TuiFeature::KeyChords,
        TuiFeature::OwnedProcessTree,
        TuiFeature::Paste,
        TuiFeature::RegexWaits,
        TuiFeature::Resize,
        TuiFeature::SemanticViewport,
        TuiFeature::TerminalRecording,
        TuiFeature::TextWaits,
    ]
    .into_iter()
    .collect()
}

fn expected_limits() -> TuiCapabilityLimits {
    TuiCapabilityLimits {
        max_columns: MAX_TUI_COLUMNS,
        max_rows: MAX_TUI_ROWS,
        max_scrollback_rows: MAX_TUI_SCROLLBACK_ROWS as u64,
        max_output_bytes: MAX_TUI_OUTPUT_BYTES as u64,
        max_terminal_cells: MAX_TUI_TERMINAL_CELLS as u64,
    }
}

#[cfg(unix)]
fn compiled_backend() -> Result<TuiBackend, DriverError> {
    Ok(TuiBackend::UnixPty)
}

#[cfg(windows)]
fn compiled_backend() -> Result<TuiBackend, DriverError> {
    Ok(TuiBackend::WindowsConPty)
}

#[cfg(not(any(unix, windows)))]
fn compiled_backend() -> Result<TuiBackend, DriverError> {
    Err(capability_error(
        "this platform has no reviewed PTY backend",
    ))
}

fn capability_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.tui.capability_unavailable", message)
}
