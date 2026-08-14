use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use a3s_test_core::DriverError;

pub const MAX_TUI_COLUMNS: u16 = 1_000;
pub const MAX_TUI_ROWS: u16 = 500;
const MAX_SCROLLBACK_ROWS: usize = 10_000;
const MAX_TERMINAL_CELLS: usize = 2_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TuiSize {
    pub rows: u16,
    pub columns: u16,
}

impl TuiSize {
    pub fn new(columns: u16, rows: u16) -> Result<Self, DriverError> {
        if rows == 0 || columns == 0 || rows > MAX_TUI_ROWS || columns > MAX_TUI_COLUMNS {
            return Err(config_error(format!(
                "terminal dimensions exceed the supported {MAX_TUI_COLUMNS}x{MAX_TUI_ROWS} cell bound"
            )));
        }
        Ok(Self { rows, columns })
    }
}

impl Default for TuiSize {
    fn default() -> Self {
        Self {
            rows: 24,
            columns: 80,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TuiCommand {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub working_directory: Option<PathBuf>,
    pub environment: BTreeMap<OsString, OsString>,
}

impl TuiCommand {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            working_directory: None,
            environment: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TuiDriverConfig {
    pub command: TuiCommand,
    pub initial_size: TuiSize,
    pub command_timeout: Duration,
    pub cleanup_timeout: Duration,
    pub scrollback_rows: usize,
    pub max_output_bytes: usize,
}

impl TuiDriverConfig {
    pub fn validate(&self) -> Result<(), DriverError> {
        if self.command.executable.as_os_str().is_empty() {
            return Err(config_error("TUI executable must not be empty"));
        }
        if self.command.arguments.len() > 256
            || self
                .command
                .arguments
                .iter()
                .any(|argument| argument.len() > 64 * 1024)
        {
            return Err(config_error(
                "TUI arguments must contain at most 256 entries of at most 65536 bytes",
            ));
        }
        if self.command.environment.len() > 128
            || self.command.environment.iter().any(|(name, value)| {
                name.is_empty() || name.len() > 4_096 || value.len() > 64 * 1024
            })
        {
            return Err(config_error(
                "TUI environment must contain at most 128 bounded non-empty names and values",
            ));
        }
        if let Some(directory) = &self.command.working_directory {
            if !directory.is_absolute() {
                return Err(config_error("TUI working directory must be absolute"));
            }
        }
        if self.command_timeout.is_zero() {
            return Err(config_error(
                "TUI command timeout must be greater than zero",
            ));
        }
        if self.cleanup_timeout.is_zero() {
            return Err(config_error(
                "TUI cleanup timeout must be greater than zero",
            ));
        }
        if !(1..=MAX_SCROLLBACK_ROWS).contains(&self.scrollback_rows) {
            return Err(config_error(
                "TUI scrollback rows must be between 1 and 10000",
            ));
        }
        if !(1_024..=16 * 1024 * 1024).contains(&self.max_output_bytes) {
            return Err(config_error(
                "TUI output budget must be between 1024 and 16777216 bytes",
            ));
        }
        let size = TuiSize::new(self.initial_size.columns, self.initial_size.rows)?;
        validate_terminal_budget(size, self.scrollback_rows)?;
        Ok(())
    }
}

pub(crate) fn validate_terminal_budget(
    size: TuiSize,
    scrollback_rows: usize,
) -> Result<(), DriverError> {
    let retained_rows = scrollback_rows.saturating_add(usize::from(size.rows));
    let cells = retained_rows.saturating_mul(usize::from(size.columns));
    if cells > MAX_TERMINAL_CELLS {
        return Err(config_error(format!(
            "terminal viewport and scrollback exceed the {MAX_TERMINAL_CELLS} cell state budget"
        )));
    }
    Ok(())
}

impl Default for TuiDriverConfig {
    fn default() -> Self {
        Self {
            command: TuiCommand::new("sh"),
            initial_size: TuiSize::default(),
            command_timeout: Duration::from_secs(30),
            cleanup_timeout: Duration::from_secs(5),
            scrollback_rows: 2_000,
            max_output_bytes: 4 * 1024 * 1024,
        }
    }
}

fn config_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.tui.config_invalid", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rejects_unbounded_terminal_state() {
        let config = TuiDriverConfig {
            scrollback_rows: 0,
            ..TuiDriverConfig::default()
        };
        assert_eq!(
            config.validate().expect_err("zero scrollback").code(),
            "test.driver.tui.config_invalid"
        );

        let config = TuiDriverConfig {
            scrollback_rows: 1,
            max_output_bytes: usize::MAX,
            ..TuiDriverConfig::default()
        };
        assert_eq!(
            config.validate().expect_err("unbounded output").code(),
            "test.driver.tui.config_invalid"
        );
    }
}
