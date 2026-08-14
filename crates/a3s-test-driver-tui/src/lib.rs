//! Owned pseudo-terminal adapter for TUI testing.

mod artifact;
mod config;
mod driver;
mod input;
mod process;
mod terminal;

pub use config::{TuiCommand, TuiDriverConfig, TuiSize};
pub use driver::{TuiDriver, TuiSession};
pub use process::terminate_active_tui_processes;
