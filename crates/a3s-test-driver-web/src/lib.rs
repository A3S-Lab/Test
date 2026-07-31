//! A3S Browser adapter for A3S Test.

mod config;
mod executor;
mod process;
mod protocol;
mod session;

pub use config::{AgentBrowserConfig, BrowserCommand};
pub use executor::{CommandExecutor, CommandInvocation, CommandOutput, TokioCommandExecutor};
pub use process::terminate_active_commands;
pub use session::AgentBrowserDriver;
