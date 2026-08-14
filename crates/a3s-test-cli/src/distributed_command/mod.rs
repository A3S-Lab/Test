mod config;
mod history;
mod http;
mod input;
mod report;
mod runtime;
mod shard;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_test_worker::{distributed_run_protocol_schema, DistributedRunStatus};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Args)]
pub(crate) struct DistributedArgs {
    #[command(subcommand)]
    command: DistributedCommand,
}

#[derive(Debug, Subcommand)]
enum DistributedCommand {
    /// Inspect workers and emit the immutable deterministic shard plan.
    Plan(DistributedPlanArgs),
    /// Execute, verify, analyze, and retain one distributed run.
    Run(DistributedRunArgs),
    /// Print the distributed plan and analysis protocol schemas.
    Schema(DistributedSchemaArgs),
}

#[derive(Debug, Args)]
struct DistributedPlanArgs {
    /// ACL distributed run configuration.
    config: PathBuf,
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    compact: bool,
}

#[derive(Debug, Args)]
struct DistributedRunArgs {
    /// ACL distributed run configuration.
    config: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DistributedSchemaArgs {
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    compact: bool,
}

pub(crate) async fn execute(args: DistributedArgs) -> Result<ExitCode> {
    match args.command {
        DistributedCommand::Plan(args) => {
            let plan = runtime::create_plan(&args.config).await?;
            print_json(&plan, args.compact)?;
            Ok(ExitCode::SUCCESS)
        }
        DistributedCommand::Run(args) => run(args).await,
        DistributedCommand::Schema(args) => {
            print_json(&distributed_run_protocol_schema(), args.compact)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

async fn run(args: DistributedRunArgs) -> Result<ExitCode> {
    let cancellation = CancellationToken::new();
    let signal_task = crate::install_interrupt_handler(cancellation.clone());
    let result = runtime::run(&args.config, cancellation.clone()).await;
    signal_task.abort();
    let _ = signal_task.await;
    let (analysis, report_path) = match result {
        Ok(result) => result,
        Err(_) if cancellation.is_cancelled() => return Ok(ExitCode::from(130)),
        Err(error) => return Err(error),
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&analysis)?);
    } else {
        println!(
            "{}: {} ({} passed, {} failed, {} quarantined, {} infrastructure, {} timed out, {} cancelled)",
            status_label(analysis.status),
            analysis.suite,
            analysis.counts.passed,
            analysis.counts.failed,
            analysis.counts.quarantined_failed,
            analysis.counts.infrastructure_failed,
            analysis.counts.timed_out,
            analysis.counts.cancelled,
        );
        println!("Report: {}", report_path.display());
    }
    Ok(status_exit_code(analysis.status))
}

fn print_json(value: &impl Serialize, compact: bool) -> Result<()> {
    if compact {
        println!("{}", serde_json::to_string(value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}

fn status_label(status: DistributedRunStatus) -> &'static str {
    match status {
        DistributedRunStatus::Passed => "PASS",
        DistributedRunStatus::Failed => "FAIL",
        DistributedRunStatus::InfrastructureFailed => "INFRASTRUCTURE",
        DistributedRunStatus::TimedOut => "TIMEOUT",
        DistributedRunStatus::Cancelled => "CANCELLED",
    }
}

fn status_exit_code(status: DistributedRunStatus) -> ExitCode {
    match status {
        DistributedRunStatus::Passed => ExitCode::SUCCESS,
        DistributedRunStatus::Failed => ExitCode::from(1),
        DistributedRunStatus::InfrastructureFailed => ExitCode::from(2),
        DistributedRunStatus::TimedOut => ExitCode::from(124),
        DistributedRunStatus::Cancelled => ExitCode::from(130),
    }
}

pub(super) fn unix_ms() -> Result<u64> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock precedes the Unix epoch")?
        .as_millis();
    u64::try_from(milliseconds).context("system clock does not fit distributed timestamps")
}

pub(super) fn operation_id(kind: &str, now_ms: u64) -> String {
    let sequence = NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
    format!("{kind}-{now_ms:x}-{:x}-{sequence:x}", std::process::id())
}

pub(super) fn request_id(kind: &str) -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    operation_id(kind, now_ms)
}

#[cfg(test)]
mod tests {
    use super::{operation_id, status_exit_code};
    use a3s_test_worker::DistributedRunStatus;
    use std::process::ExitCode;

    #[test]
    fn operation_ids_are_portable_and_exit_codes_are_stable() {
        let id = operation_id("run", 1_800_000_000_000);
        assert!(id.len() < 128);
        assert!(id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
        assert_eq!(
            status_exit_code(DistributedRunStatus::Passed),
            ExitCode::SUCCESS
        );
        assert_eq!(
            status_exit_code(DistributedRunStatus::Failed),
            ExitCode::from(1)
        );
        assert_eq!(
            status_exit_code(DistributedRunStatus::InfrastructureFailed),
            ExitCode::from(2)
        );
    }
}
