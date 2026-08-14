use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use a3s_test_driver_tui::TuiCapabilities;
use a3s_test_driver_web::{AgentBrowserConfig, AgentBrowserDriver};
use a3s_test_worker::{
    worker_capability_protocol_schema, WebExecutionMode, WorkerCapabilityInventory,
    WorkerSurfaceCapability,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;

use super::{browser_command, validate_timeout, BrowserDriverKind};

#[derive(Debug, Args)]
pub(crate) struct WorkerArgs {
    #[command(subcommand)]
    command: WorkerCommand,
}

#[derive(Debug, Subcommand)]
enum WorkerCommand {
    /// Probe this runtime and print its typed scheduling inventory.
    Inventory(WorkerInventoryArgs),
    /// Print the versioned worker capability protocol and JSON Schema.
    Schema(WorkerSchemaArgs),
}

#[derive(Debug, Args)]
struct WorkerInventoryArgs {
    /// Explicitly add a probed Web integration to the inventory.
    #[arg(long, value_enum)]
    browser_driver: Option<BrowserDriverKind>,
    /// Override the explicitly selected browser driver executable.
    #[arg(long, requires = "browser_driver")]
    browser_executable: Option<PathBuf>,
    /// Browser capability probe deadline.
    #[arg(long, default_value_t = 30_000)]
    command_timeout_ms: u64,
    /// Maximum scenario concurrency advertised to a scheduler.
    #[arg(long, default_value_t = 1)]
    max_parallel_scenarios: u16,
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    compact: bool,
}

#[derive(Debug, Args)]
struct WorkerSchemaArgs {
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    compact: bool,
}

pub(crate) async fn execute(args: WorkerArgs) -> Result<ExitCode> {
    match args.command {
        WorkerCommand::Inventory(args) => inventory(args).await,
        WorkerCommand::Schema(args) => schema(args),
    }
}

async fn inventory(args: WorkerInventoryArgs) -> Result<ExitCode> {
    validate_timeout(args.command_timeout_ms, "command timeout")?;
    let mut surfaces = vec![WorkerSurfaceCapability::Tui {
        terminal: TuiCapabilities::compiled().map_err(anyhow::Error::new)?,
    }];
    if let Some(kind) = args.browser_driver {
        let browser = AgentBrowserDriver::new(AgentBrowserConfig {
            command: browser_command(kind, args.browser_executable),
            namespace: String::new(),
            headed: false,
            command_timeout: Duration::from_millis(args.command_timeout_ms),
            idle_timeout: Duration::from_secs(30),
            network_policy: Default::default(),
        });
        let capabilities = browser.capabilities().await.map_err(anyhow::Error::new)?;
        surfaces.push(WorkerSurfaceCapability::Web {
            execution: WebExecutionMode::Headless,
            browser: capabilities,
        });
    }
    let inventory = WorkerCapabilityInventory::local(args.max_parallel_scenarios, surfaces)
        .map_err(anyhow::Error::new)?;
    print_json(&inventory, args.compact)?;
    Ok(ExitCode::SUCCESS)
}

fn schema(args: WorkerSchemaArgs) -> Result<ExitCode> {
    print_json(&worker_capability_protocol_schema(), args.compact)?;
    Ok(ExitCode::SUCCESS)
}

fn print_json(value: &impl Serialize, compact: bool) -> Result<()> {
    if compact {
        println!("{}", serde_json::to_string(value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}
