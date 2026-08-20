mod config;
mod dev;
mod discovery;
mod doctor;
mod init;
mod process;
mod repair_bridge;
mod verification;

pub(crate) use verification::{run_configured_checks, VerificationRun};

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, ValueEnum};

pub(crate) const DEFAULT_CONFIG_PATH: &str = ".a3s-test/project.acl";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum TestKitRequirementArg {
    #[default]
    Required,
    Optional,
}

#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    /// Project root containing package.json.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Project profile path relative to the project root.
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    /// Development script from package.json. Defaults to dev, then start.
    #[arg(long)]
    script: Option<String>,
    /// Initial HTTP(S) product URL. Defaults from the detected framework.
    #[arg(long)]
    url: Option<String>,
    /// Whether the Vibe Loop requires the embedded Test Kit.
    #[arg(long, value_enum, default_value_t = TestKitRequirementArg::Required)]
    testkit: TestKitRequirementArg,
    /// Replace an existing regular project profile.
    #[arg(long)]
    force: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DoctorArgs {
    /// Project root containing the A3S Test profile.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Project profile path relative to the project root.
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    /// Probe the configured URL in addition to static project checks.
    #[arg(long)]
    connect: bool,
    /// Treat warnings as a failed diagnosis.
    #[arg(long)]
    strict: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DevArgs {
    /// Project root containing the A3S Test profile.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Project profile path relative to the project root.
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    /// Emit lifecycle events as JSON lines.
    #[arg(long)]
    json: bool,
}

pub(crate) async fn init(args: InitArgs) -> Result<ExitCode> {
    init::execute(args).await
}

pub(crate) async fn doctor(args: DoctorArgs) -> Result<ExitCode> {
    doctor::execute(args).await
}

pub(crate) async fn dev(args: DevArgs) -> Result<ExitCode> {
    dev::execute(args).await
}

pub(crate) fn terminate_active_dev_servers() {
    process::terminate_active_dev_servers();
}
