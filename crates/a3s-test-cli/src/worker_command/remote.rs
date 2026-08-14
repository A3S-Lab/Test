use std::{
    ffi::OsString, net::SocketAddr, path::PathBuf, process::ExitCode, sync::Arc, time::Duration,
};

use a3s_test_driver_tui::{TuiCapabilities, TuiCommand, TuiDriverConfig, TuiSize};
use a3s_test_driver_web::{AgentBrowserConfig, BrowserNetworkPolicy};
use a3s_test_worker::{
    remote_artifact_protocol_schema, remote_worker_protocol_schema, RemoteArtifactDescriptor,
    RemoteRetentionPolicy, RemoteWorkerDescriptor, RemoteWorkerIdentity, RemoteWorkerLimits,
    RemoteWorkerService, RemoteWorkerServiceConfig, WebExecutionMode, WorkerCapabilityInventory,
    WorkerSurfaceCapability, DEFAULT_MAX_INDEXED_JOBS, DEFAULT_MAX_INDEX_AGE_MS,
    DEFAULT_MAX_RETAINED_BYTES, DEFAULT_MAX_RETAINED_JOBS, DEFAULT_MAX_RETENTION_AGE_MS,
    MAX_REMOTE_CLEANUP_TIMEOUT_MS, MIN_REMOTE_CLEANUP_TIMEOUT_MS,
};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use super::print_json;
use crate::{browser_command, validate_timeout, BrowserDriverKind};

mod executor;
mod http;

const MAX_REMOTE_COMMAND_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
const MAX_REMOTE_IDLE_TIMEOUT_MS: u64 = 60 * 60 * 1_000;
const MAX_REMOTE_RETRY_BACKOFF_MS: u64 = 60 * 1_000;

#[derive(Debug, Args)]
pub(super) struct WorkerRemoteArgs {
    #[command(subcommand)]
    command: WorkerRemoteCommand,
}

#[derive(Debug, Subcommand)]
enum WorkerRemoteCommand {
    /// Print the strict remote worker request, response, and descriptor schemas.
    Schema(RemoteSchemaArgs),
}

#[derive(Debug, Args)]
struct RemoteSchemaArgs {
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    compact: bool,
}

#[derive(Debug, Args)]
pub(super) struct WorkerArtifactArgs {
    #[command(subcommand)]
    command: WorkerArtifactCommand,
}

#[derive(Debug, Subcommand)]
enum WorkerArtifactCommand {
    /// Print the strict report-index and artifact-transport schemas.
    Schema(RemoteSchemaArgs),
}

#[derive(Debug, Args)]
pub(super) struct WorkerServeArgs {
    /// Loopback socket used by the HTTP reference host.
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: SocketAddr,
    /// Private persistent state root; relative paths resolve from the current directory.
    #[arg(long, default_value = ".a3s-test/remote-worker")]
    state_root: PathBuf,
    /// Stable identity for this exact worker process or deployment instance.
    #[arg(long)]
    instance_id: String,
    /// Externally supplied digest of the deployed worker image.
    #[arg(long)]
    image_digest: String,
    /// Environment variable containing the exact required Authorization header value.
    #[arg(long)]
    authorization_env: String,
    /// Browser integration enabled by this deployment; omitted disables Web jobs.
    #[arg(long, value_enum)]
    browser_driver: Option<BrowserDriverKind>,
    /// Deployment-owned browser driver executable.
    #[arg(long, requires = "browser_driver")]
    browser_executable: Option<PathBuf>,
    /// Exact Web origin admitted by this deployment. Repeat for multiple origins.
    #[arg(long = "web-allow-origin", requires = "browser_driver")]
    web_allowed_origins: Vec<String>,
    /// Additional hostname admitted by the deployment Web policy.
    #[arg(long = "web-allow-domain", requires = "browser_driver")]
    web_allowed_domains: Vec<String>,
    /// Deployment-owned executable used by every remote TUI scenario; a shell grants shell authority.
    #[arg(long)]
    tui_executable: Option<PathBuf>,
    /// Deployment-owned TUI argument. Repeat to pass multiple arguments.
    #[arg(
        long = "tui-arg",
        requires = "tui_executable",
        allow_hyphen_values = true
    )]
    tui_arguments: Vec<OsString>,
    /// Deployment-owned absolute TUI working directory; input root is used when omitted.
    #[arg(long, requires = "tui_executable")]
    tui_working_directory: Option<PathBuf>,
    /// Initial remote TUI terminal column count.
    #[arg(long, default_value_t = 80, requires = "tui_executable")]
    tui_columns: u16,
    /// Initial remote TUI terminal row count.
    #[arg(long, default_value_t = 24, requires = "tui_executable")]
    tui_rows: u16,
    /// Retained semantic TUI scrollback rows.
    #[arg(long, default_value_t = 2_000, requires = "tui_executable")]
    tui_scrollback_rows: usize,
    /// Maximum retained raw TUI output bytes.
    #[arg(long, default_value_t = 4 * 1024 * 1024, requires = "tui_executable")]
    tui_max_output_bytes: usize,
    /// Per-command Web or TUI deadline.
    #[arg(long, default_value_t = 30_000)]
    command_timeout_ms: u64,
    /// Browser daemon inactivity deadline.
    #[arg(long, default_value_t = 30_000)]
    idle_timeout_ms: u64,
    /// Surface and server cleanup deadline.
    #[arg(long, default_value_t = 30_000)]
    cleanup_timeout_ms: u64,
    /// Retries allowed only for explicitly retryable infrastructure failures.
    #[arg(long, default_value_t = 1)]
    infrastructure_retries: u32,
    /// Delay between infrastructure retries.
    #[arg(long, default_value_t = 100)]
    retry_backoff_ms: u64,
    /// Maximum scenario concurrency accepted in a dispatched job.
    #[arg(long, default_value_t = 1)]
    max_parallel_scenarios: u16,
    /// Maximum jobs waiting behind the single active remote job.
    #[arg(long, default_value_t = 16)]
    max_queued_jobs: u16,
    /// Maximum terminal jobs whose complete input, report, and evidence remain retained.
    #[arg(long, default_value_t = DEFAULT_MAX_RETAINED_JOBS)]
    retention_max_jobs: u32,
    /// Aggregate byte budget for retained terminal-job inputs, reports, and evidence.
    #[arg(long, default_value_t = DEFAULT_MAX_RETAINED_BYTES)]
    retention_max_bytes: u64,
    /// Maximum age of complete terminal-job payloads.
    #[arg(long, default_value_t = DEFAULT_MAX_RETENTION_AGE_MS)]
    retention_max_age_ms: u64,
    /// Maximum compact terminal-job records retained in the report index.
    #[arg(long, default_value_t = DEFAULT_MAX_INDEXED_JOBS)]
    report_index_max_jobs: u32,
    /// Maximum age of compact terminal-job records in the report index.
    #[arg(long, default_value_t = DEFAULT_MAX_INDEX_AGE_MS)]
    report_index_max_age_ms: u64,
    /// Emit a compact JSON readiness record.
    #[arg(long)]
    compact: bool,
}

#[derive(Serialize)]
struct ReadyRecord<'a> {
    listen: String,
    worker: &'a RemoteWorkerDescriptor,
    artifacts: &'a RemoteArtifactDescriptor,
}

pub(super) fn execute(args: WorkerRemoteArgs) -> Result<ExitCode> {
    match args.command {
        WorkerRemoteCommand::Schema(args) => {
            print_json(&remote_worker_protocol_schema(), args.compact)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

pub(super) fn execute_artifacts(args: WorkerArtifactArgs) -> Result<ExitCode> {
    match args.command {
        WorkerArtifactCommand::Schema(args) => {
            print_json(&remote_artifact_protocol_schema(), args.compact)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

pub(super) async fn serve(args: WorkerServeArgs) -> Result<ExitCode> {
    validate_serve_args(&args)?;
    let retention_policy = retention_policy(&args)?;
    let authorization = read_authorization(&args.authorization_env)?;
    let state_root = absolute_path(args.state_root.clone())?;
    let profiles = build_profiles(&args).await?;
    let limits = RemoteWorkerLimits {
        max_queued_jobs: args.max_queued_jobs,
        cleanup_timeout_ms: args.cleanup_timeout_ms,
        ..RemoteWorkerLimits::default()
    };
    let inventory =
        WorkerCapabilityInventory::local(args.max_parallel_scenarios, profiles.capabilities)
            .map_err(anyhow::Error::new)?;
    let descriptor = RemoteWorkerDescriptor::new(
        RemoteWorkerIdentity {
            instance_id: args.instance_id,
            image_digest: args.image_digest,
        },
        inventory,
        limits,
    )
    .map_err(anyhow::Error::new)?;
    let listener = TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("failed to bind remote worker listener {}", args.listen))?;
    let listen = listener
        .local_addr()
        .context("failed to inspect remote worker listener")?;
    let executor = Arc::new(executor::CliRemoteExecutor::new(profiles.executor));
    let service = RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(state_root, descriptor)
            .with_retention_policy(retention_policy),
        executor,
    )
    .await
    .map_err(anyhow::Error::new)?;
    print_json(
        &ReadyRecord {
            listen: listen.to_string(),
            worker: service.descriptor(),
            artifacts: service.artifact_descriptor(),
        },
        args.compact,
    )?;

    let shutdown = CancellationToken::new();
    let signal_task = install_shutdown_signal(shutdown.clone());
    let server_result = http::serve(
        listener,
        service.clone(),
        authorization,
        shutdown,
        Duration::from_millis(args.command_timeout_ms),
        Duration::from_millis(args.cleanup_timeout_ms),
    )
    .await;
    signal_task.abort();
    let _ = signal_task.await;
    let service_result = service.shutdown().await.map_err(anyhow::Error::new);
    server_result?;
    service_result?;
    Ok(ExitCode::SUCCESS)
}

struct BuiltProfiles {
    capabilities: Vec<WorkerSurfaceCapability>,
    executor: executor::ExecutorProfiles,
}

async fn build_profiles(args: &WorkerServeArgs) -> Result<BuiltProfiles> {
    let mut capabilities = Vec::new();
    let authorization_environment = OsString::from(&args.authorization_env);
    let surface_cleanup_timeout = Duration::from_millis((args.cleanup_timeout_ms / 3).max(1));
    let runner_cleanup_timeout = Duration::from_millis((args.cleanup_timeout_ms / 2).max(1));
    let web = if let Some(kind) = args.browser_driver {
        if args.web_allowed_origins.is_empty() {
            anyhow::bail!("remote Web execution requires at least one --web-allow-origin");
        }
        let network_policy = BrowserNetworkPolicy::restricted(
            args.web_allowed_origins.iter().cloned(),
            args.web_allowed_domains.iter().cloned(),
        )
        .map_err(anyhow::Error::new)?;
        let config = AgentBrowserConfig {
            command: browser_command(kind, args.browser_executable.clone()),
            namespace: String::new(),
            headed: false,
            command_timeout: Duration::from_millis(args.command_timeout_ms),
            idle_timeout: Duration::from_millis(args.idle_timeout_ms),
            network_policy,
        };
        let browser =
            executor::remote_web_driver(config.clone(), authorization_environment.clone());
        let browser_capabilities = browser.capabilities().await.map_err(anyhow::Error::new)?;
        capabilities.push(WorkerSurfaceCapability::Web {
            execution: WebExecutionMode::Headless,
            browser: browser_capabilities,
        });
        Some(config)
    } else {
        None
    };

    let tui = args
        .tui_executable
        .as_ref()
        .map(|executable| {
            let mut command = TuiCommand::new(executable);
            command.arguments.clone_from(&args.tui_arguments);
            command.working_directory = args.tui_working_directory.clone();
            command
                .removed_environment
                .insert(authorization_environment.clone());
            let config = TuiDriverConfig {
                command,
                initial_size: TuiSize::new(args.tui_columns, args.tui_rows)
                    .map_err(anyhow::Error::new)?,
                command_timeout: Duration::from_millis(args.command_timeout_ms),
                cleanup_timeout: surface_cleanup_timeout,
                scrollback_rows: args.tui_scrollback_rows,
                max_output_bytes: args.tui_max_output_bytes,
            };
            config.validate().map_err(anyhow::Error::new)?;
            Ok::<_, anyhow::Error>(config)
        })
        .transpose()?;
    if tui.is_some() {
        capabilities.push(WorkerSurfaceCapability::Tui {
            terminal: TuiCapabilities::compiled().map_err(anyhow::Error::new)?,
        });
    }
    if capabilities.is_empty() {
        anyhow::bail!("remote worker requires a deployment-owned Web or TUI profile");
    }
    Ok(BuiltProfiles {
        capabilities,
        executor: executor::ExecutorProfiles {
            web,
            tui,
            authorization_environment,
            cleanup_timeout: runner_cleanup_timeout,
            infrastructure_retries: args.infrastructure_retries,
            retry_backoff: Duration::from_millis(args.retry_backoff_ms),
        },
    })
}

fn validate_serve_args(args: &WorkerServeArgs) -> Result<()> {
    if !args.listen.ip().is_loopback() {
        anyhow::bail!("remote worker listener must bind a loopback address");
    }
    validate_timeout(args.command_timeout_ms, "command timeout")?;
    validate_timeout(args.idle_timeout_ms, "idle timeout")?;
    validate_timeout(args.cleanup_timeout_ms, "cleanup timeout")?;
    if args.command_timeout_ms > MAX_REMOTE_COMMAND_TIMEOUT_MS {
        anyhow::bail!("command timeout cannot exceed {MAX_REMOTE_COMMAND_TIMEOUT_MS} ms");
    }
    if args.idle_timeout_ms > MAX_REMOTE_IDLE_TIMEOUT_MS {
        anyhow::bail!("idle timeout cannot exceed {MAX_REMOTE_IDLE_TIMEOUT_MS} ms");
    }
    if !(MIN_REMOTE_CLEANUP_TIMEOUT_MS..=MAX_REMOTE_CLEANUP_TIMEOUT_MS)
        .contains(&args.cleanup_timeout_ms)
    {
        anyhow::bail!(
            "cleanup timeout must be between {MIN_REMOTE_CLEANUP_TIMEOUT_MS} and {MAX_REMOTE_CLEANUP_TIMEOUT_MS} ms"
        );
    }
    if args.infrastructure_retries > 10 {
        anyhow::bail!("infrastructure retries cannot exceed 10");
    }
    if args.retry_backoff_ms > MAX_REMOTE_RETRY_BACKOFF_MS {
        anyhow::bail!("retry backoff cannot exceed {MAX_REMOTE_RETRY_BACKOFF_MS} ms");
    }
    if !(1..=64).contains(&args.max_parallel_scenarios) {
        anyhow::bail!("parallel scenario limit must be between 1 and 64");
    }
    if !(1..=1_024).contains(&args.max_queued_jobs) {
        anyhow::bail!("queued job limit must be between 1 and 1024");
    }
    if let Some(directory) = &args.tui_working_directory {
        if !directory.is_absolute() {
            anyhow::bail!("--tui-working-directory must be absolute");
        }
    }
    Ok(())
}

fn retention_policy(args: &WorkerServeArgs) -> Result<RemoteRetentionPolicy> {
    let policy = RemoteRetentionPolicy {
        max_retained_jobs: args.retention_max_jobs,
        max_retained_bytes: args.retention_max_bytes,
        max_retention_age_ms: args.retention_max_age_ms,
        max_indexed_jobs: args.report_index_max_jobs,
        max_index_age_ms: args.report_index_max_age_ms,
    };
    policy.validate().map_err(anyhow::Error::new)?;
    Ok(policy)
}

fn read_authorization(environment_name: &str) -> Result<Arc<[u8]>> {
    let valid_name = !environment_name.is_empty()
        && environment_name.len() <= 128
        && environment_name.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_uppercase() || (index > 0 && byte.is_ascii_digit())
        });
    if !valid_name {
        anyhow::bail!("authorization environment variable name is invalid");
    }
    let value = std::env::var_os(environment_name)
        .with_context(|| format!("authorization environment variable {environment_name} is unset"))?
        .into_string()
        .map_err(|_| anyhow::anyhow!("authorization environment variable is not valid UTF-8"))?;
    if value.is_empty()
        || value.trim() != value
        || value.len() > 4_096
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        anyhow::bail!("authorization environment variable contains an invalid HTTP header value");
    }
    Ok(Arc::from(value.into_bytes()))
}

fn absolute_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(std::env::current_dir()
        .context("failed to resolve current directory")?
        .join(path))
}

fn install_shutdown_signal(shutdown: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            shutdown.cancel();
        }
    })
}
