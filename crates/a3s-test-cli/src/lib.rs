use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::TestSuite;
use a3s_test_driver_web::{
    terminate_active_commands, AgentBrowserConfig, AgentBrowserDriver, BrowserCapabilities,
    BrowserCommand,
};
use a3s_test_runner::{RetryPolicy, RunResult, RunStatus, Runner, RunnerOptions};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
#[command(
    name = "a3s-test",
    version,
    about = "Agent-ready end-to-end testing across Web, GUI, and TUI surfaces"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Validate and inspect an ACL test suite without launching a surface.
    Check(CheckArgs),
    /// Discover and verify the installed Web driver protocol.
    Capabilities(CapabilitiesArgs),
    /// Run a test suite.
    Run(RunArgs),
}

#[derive(Debug, Args)]
struct CheckArgs {
    /// ACL test suite.
    manifest: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BrowserDriverKind {
    /// Run the browser capability through `a3s use browser`.
    A3s,
    /// Run a standalone `agent-browser` compatible executable.
    Standalone,
}

#[derive(Debug, Args)]
struct CapabilitiesArgs {
    /// Browser driver integration.
    #[arg(long, value_enum, default_value_t = BrowserDriverKind::A3s)]
    browser_driver: BrowserDriverKind,
    /// Override the browser driver executable.
    #[arg(long)]
    browser_executable: Option<PathBuf>,
    /// Browser capability probe deadline.
    #[arg(long, default_value_t = 30_000)]
    command_timeout_ms: u64,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RunArgs {
    /// ACL test suite.
    manifest: PathBuf,
    /// Browser driver integration.
    #[arg(long, value_enum, default_value_t = BrowserDriverKind::A3s)]
    browser_driver: BrowserDriverKind,
    /// Override the browser driver executable.
    #[arg(long)]
    browser_executable: Option<PathBuf>,
    /// Show the browser window.
    #[arg(long)]
    headed: bool,
    /// Per-command browser deadline.
    #[arg(long, default_value_t = 30_000)]
    command_timeout_ms: u64,
    /// Browser daemon inactivity deadline.
    #[arg(long, default_value_t = 30_000)]
    idle_timeout_ms: u64,
    /// Surface cleanup deadline.
    #[arg(long, default_value_t = 10_000)]
    cleanup_timeout_ms: u64,
    /// Retries allowed only for explicitly retryable infrastructure failures.
    #[arg(long, default_value_t = 1)]
    infrastructure_retries: u32,
    /// Delay between infrastructure retries.
    #[arg(long, default_value_t = 100)]
    retry_backoff_ms: u64,
    /// Maximum scenarios that may own browser sessions concurrently.
    #[arg(long, default_value_t = 1)]
    max_parallel_scenarios: usize,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

pub async fn execute(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Commands::Check(args) => check(args).await,
        Commands::Capabilities(args) => capabilities(args).await,
        Commands::Run(args) => run(args).await,
    }
}

async fn check(args: CheckArgs) -> Result<ExitCode> {
    let suite = read_suite(&args.manifest).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&suite)?);
    } else {
        println!(
            "Valid: {} ({} scenario{})",
            suite.name,
            suite.scenarios.len(),
            plural(suite.scenarios.len())
        );
    }
    Ok(ExitCode::SUCCESS)
}

async fn capabilities(args: CapabilitiesArgs) -> Result<ExitCode> {
    validate_timeout(args.command_timeout_ms, "command timeout")?;
    let browser = AgentBrowserDriver::new(AgentBrowserConfig {
        command: browser_command(args.browser_driver, args.browser_executable),
        namespace: String::new(),
        headed: false,
        command_timeout: Duration::from_millis(args.command_timeout_ms),
        idle_timeout: Duration::from_secs(30),
    });
    let capabilities = browser.capabilities().await.map_err(anyhow::Error::new)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&capabilities)?);
    } else {
        print_capabilities(&capabilities);
    }
    Ok(ExitCode::SUCCESS)
}

async fn run(args: RunArgs) -> Result<ExitCode> {
    validate_timeout(args.command_timeout_ms, "command timeout")?;
    validate_timeout(args.idle_timeout_ms, "idle timeout")?;
    validate_timeout(args.cleanup_timeout_ms, "cleanup timeout")?;
    if args.infrastructure_retries > 10 {
        anyhow::bail!("infrastructure retries cannot exceed 10");
    }
    if !(1..=64).contains(&args.max_parallel_scenarios) {
        anyhow::bail!("parallel scenario limit must be between 1 and 64");
    }

    let suite = read_suite(&args.manifest).await?;
    let command = browser_command(args.browser_driver, args.browser_executable);
    let browser = AgentBrowserDriver::new(AgentBrowserConfig {
        command,
        namespace: String::new(),
        headed: args.headed,
        command_timeout: Duration::from_millis(args.command_timeout_ms),
        idle_timeout: Duration::from_millis(args.idle_timeout_ms),
    });
    let runner = Runner::new(
        vec![Arc::new(browser)],
        RunnerOptions {
            cleanup_timeout: Duration::from_millis(args.cleanup_timeout_ms),
            retry_policy: RetryPolicy {
                max_retries: args.infrastructure_retries,
                backoff: Duration::from_millis(args.retry_backoff_ms),
            },
            max_parallel_scenarios: args.max_parallel_scenarios,
        },
    )
    .map_err(anyhow::Error::msg)?;

    let cancellation = CancellationToken::new();
    let signal_task = install_interrupt_handler(cancellation.clone());
    let result = runner.run(&suite, cancellation).await;
    signal_task.abort();
    let _ = signal_task.await;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_human_result(&result);
    }
    Ok(status_exit_code(result.status))
}

async fn read_suite(path: &Path) -> Result<TestSuite> {
    let source = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    TestSuite::from_acl(&source).with_context(|| format!("invalid test suite {}", path.display()))
}

fn browser_command(kind: BrowserDriverKind, executable: Option<PathBuf>) -> BrowserCommand {
    match kind {
        BrowserDriverKind::A3s => BrowserCommand::A3s {
            executable: executable.unwrap_or_else(|| PathBuf::from("a3s")),
        },
        BrowserDriverKind::Standalone => BrowserCommand::Standalone {
            executable: executable.unwrap_or_else(|| PathBuf::from("agent-browser")),
        },
    }
}

fn install_interrupt_handler(cancellation: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        cancellation.cancel();

        if tokio::signal::ctrl_c().await.is_ok() {
            terminate_active_commands();
            std::process::exit(130);
        }
    })
}

fn print_capabilities(capabilities: &BrowserCapabilities) {
    println!(
        "{:?} browser {} (protocol {})",
        capabilities.integration, capabilities.version, capabilities.protocol_revision
    );
    for feature in &capabilities.features {
        println!("  {feature:?}");
    }
}

fn print_human_result(result: &RunResult) {
    println!("{}: {}", status_label(result.status), result.suite);
    for scenario in &result.scenarios {
        println!(
            "  {} {} ({} ms)",
            status_label(scenario.status),
            scenario.name,
            scenario.duration_ms
        );
        if let Some(error) = &scenario.error {
            println!("    {}: {}", error.code, error.message);
        }
        if let Some(error) = &scenario.cleanup_error {
            println!("    {}: {}", error.code, error.message);
        }
    }
}

fn status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Passed => "PASS",
        RunStatus::Failed => "FAIL",
        RunStatus::TimedOut => "TIMEOUT",
        RunStatus::Cancelled => "CANCELLED",
    }
}

fn status_exit_code(status: RunStatus) -> ExitCode {
    match status {
        RunStatus::Passed => ExitCode::SUCCESS,
        RunStatus::Failed => ExitCode::from(1),
        RunStatus::TimedOut => ExitCode::from(124),
        RunStatus::Cancelled => ExitCode::from(130),
    }
}

fn validate_timeout(value: u64, name: &str) -> Result<()> {
    if value == 0 {
        anyhow::bail!("{name} must be greater than zero");
    }
    Ok(())
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_are_stable_for_automation() {
        assert_eq!(status_exit_code(RunStatus::Passed), ExitCode::SUCCESS);
        assert_eq!(status_exit_code(RunStatus::Failed), ExitCode::from(1));
        assert_eq!(status_exit_code(RunStatus::TimedOut), ExitCode::from(124));
        assert_eq!(status_exit_code(RunStatus::Cancelled), ExitCode::from(130));
    }

    #[test]
    fn browser_command_is_a_typed_choice() {
        assert_eq!(
            browser_command(BrowserDriverKind::A3s, None),
            BrowserCommand::A3s {
                executable: PathBuf::from("a3s")
            }
        );
        assert_eq!(
            browser_command(BrowserDriverKind::Standalone, None),
            BrowserCommand::Standalone {
                executable: PathBuf::from("agent-browser")
            }
        );
    }
}
