//! A3S Browser adapter for A3S Test.

mod actions;
mod artifact;
mod capabilities;
mod config;
mod executor;
mod path_security;
mod process;
mod process_tree;
mod protocol;
mod repair_reference;
mod runtime;
mod session;
mod testkit;

pub use capabilities::{BrowserCapabilities, BrowserIntegration, WebCapability};
pub use config::{AgentBrowserConfig, BrowserCommand, BrowserMicrophone, BrowserNetworkPolicy};
pub use executor::{
    CommandError, CommandErrorKind, CommandExecutor, CommandInvocation, CommandOutput,
    TokioCommandExecutor,
};
pub use process::terminate_active_commands;
pub use session::{AgentBrowserConnectionConfig, AgentBrowserDriver, AgentBrowserSession};
pub use testkit::{
    TestKitHandshake, TESTKIT_HANDSHAKE_PROTOCOL, TESTKIT_PACKAGE_NAME, TESTKIT_SDK_COMPATIBILITY,
};
