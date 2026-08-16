mod config;
mod session;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use a3s_test_agent::{
    AgentLoop, AgentRunResult, AgentStatus, HttpLlmProvider, HttpProviderConfig, ProvenanceRedactor,
};
use a3s_test_core::{
    Action, DriverError, DriverSession, ScenarioContext, StepOutput, Surface, SurfaceDriver,
    TestStep,
};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserDriver, BrowserNetworkPolicy, CommandExecutor,
};
use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use self::config::{parse_config, AgentRunConfig};
use self::session::{AgentHostSession, OriginObservationPolicy};
use super::{browser_command, install_interrupt_handler, BrowserDriverKind, BrowserMicrophoneArg};

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_CLEANUP_TIMEOUT_MS: u64 = 10_000;
static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Args)]
pub(crate) struct AgentRunArgs {
    /// ACL agent-run configuration.
    config: PathBuf,
    /// Browser driver integration.
    #[arg(long, value_enum, default_value_t = BrowserDriverKind::A3s)]
    browser_driver: BrowserDriverKind,
    /// Override the browser driver executable.
    #[arg(long)]
    browser_executable: Option<PathBuf>,
    /// Synthetic grants a deterministic local microphone without using a real device.
    #[arg(long, value_enum, default_value_t = BrowserMicrophoneArg::Disabled)]
    browser_microphone: BrowserMicrophoneArg,
    /// Show the browser window; omitted runs enforce headless execution.
    #[arg(long)]
    headed: bool,
    /// Per-command browser deadline.
    #[arg(long, default_value_t = 30_000)]
    command_timeout_ms: u64,
    /// Browser daemon inactivity deadline.
    #[arg(long, default_value_t = 300_000)]
    idle_timeout_ms: u64,
    /// Surface cleanup deadline.
    #[arg(long, default_value_t = DEFAULT_CLEANUP_TIMEOUT_MS)]
    cleanup_timeout_ms: u64,
    /// Write the redacted result to this file instead of the default run path.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Replace an existing regular report file.
    #[arg(long)]
    force: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AgentHostReport {
    protocol: String,
    run_id: String,
    config: PathBuf,
    surface: Surface,
    initial_url: String,
    allowed_origins: Vec<String>,
    allowed_domains: Vec<String>,
    started_at_ms: u64,
    finished_at_ms: u64,
    result: AgentRunResult,
    verification: Vec<VerificationStepResult>,
    cleanup_error: Option<HostError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct VerificationStepResult {
    id: String,
    output: Option<StepOutput>,
    error: Option<HostError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct HostError {
    code: String,
    message: String,
    retryable: bool,
}

pub(crate) async fn execute(args: AgentRunArgs) -> Result<ExitCode> {
    execute_with_executor(args, None, None).await
}

async fn execute_with_executor(
    args: AgentRunArgs,
    executor: Option<Arc<dyn CommandExecutor>>,
    workspace_override: Option<PathBuf>,
) -> Result<ExitCode> {
    super::validate_timeout(args.command_timeout_ms, "command timeout")?;
    super::validate_timeout(args.idle_timeout_ms, "idle timeout")?;
    super::validate_timeout(args.cleanup_timeout_ms, "cleanup timeout")?;
    let (config_path, config) = read_config(&args.config).await?;
    let workspace = match workspace_override {
        Some(path) => path,
        None => canonical_workspace().await?,
    };
    let run_id = new_run_id();
    let artifacts_dir = workspace
        .join(".a3s-test")
        .join("agent-runs")
        .join(&run_id)
        .join("artifacts");
    let report_path = args.report.unwrap_or_else(|| {
        workspace
            .join(".a3s-test")
            .join("agent-runs")
            .join(&run_id)
            .join("report.json")
    });
    ensure_report_target(&report_path, args.force)?;
    ensure_report_outside_artifacts(&report_path, &artifacts_dir)?;

    let authorization = read_authorization(&config)?;
    let redactor = match authorization.as_deref() {
        Some(value) => ProvenanceRedactor::from_exact_secrets(authorization_secrets(value))?,
        None => ProvenanceRedactor::default(),
    };
    let mut provider_config = HttpProviderConfig::new(config.endpoint.clone())
        .with_timeout(Duration::from_millis(args.command_timeout_ms))?
        .with_body_limits(config.options.max_context_bytes, MAX_REPORT_BYTES)?;
    if let Some(value) = authorization {
        provider_config = provider_config.with_authorization(value)?;
    }
    let provider = Arc::new(HttpLlmProvider::new(
        config.provider.clone(),
        provider_config,
    )?);
    let policy = Arc::new(OriginObservationPolicy::new(
        config.allowed_actions.clone(),
        config.allowed_origins.clone(),
    ));
    let mut options = config.options.clone();
    options.provenance_redactor = redactor.clone();
    let agent = AgentLoop::new(provider, policy, options)?;
    let network_policy = BrowserNetworkPolicy::restricted(
        config
            .allowed_origins
            .iter()
            .map(|origin| origin.origin().ascii_serialization()),
        config.allowed_domains.iter().cloned(),
    )
    .map_err(anyhow::Error::new)?;
    let browser_config = AgentBrowserConfig {
        command: browser_command(args.browser_driver, args.browser_executable),
        namespace: String::new(),
        headed: args.headed,
        command_timeout: Duration::from_millis(args.command_timeout_ms),
        idle_timeout: Duration::from_millis(args.idle_timeout_ms),
        microphone: args.browser_microphone.into(),
        network_policy,
    };
    let browser: Arc<dyn SurfaceDriver> = match executor {
        Some(executor) => Arc::new(AgentBrowserDriver::with_executor(browser_config, executor)),
        None => Arc::new(AgentBrowserDriver::new(browser_config)),
    };
    let context = ScenarioContext {
        run_id: run_id.clone(),
        scenario_id: config.id.clone(),
        artifacts_dir,
    };
    let cancellation = CancellationToken::new();
    let signal_task = install_interrupt_handler(cancellation.clone());
    let workflow_deadline = tokio::time::Instant::now() + config.options.timeout;
    let started_at_ms = unix_ms();

    let opened = tokio::select! {
        biased;
        () = cancellation.cancelled() => Err(DriverError::new(
            "test.agent.cancelled",
            "agent run was cancelled while opening the Web surface",
        )),
        result = tokio::time::timeout_at(workflow_deadline, browser.open(&context)) => match result {
            Ok(result) => result,
            Err(_) => Err(DriverError::new(
                "test.agent.timeout",
                "agent run deadline expired while opening the Web surface",
            )),
        },
    };
    let mut session = match opened {
        Ok(session) => AgentHostSession::new(session, config.allowed_origins.clone()),
        Err(error) => {
            signal_task.abort();
            let _ = signal_task.await;
            let mut report = AgentHostReport {
                protocol: "a3s.test.agent-run/1".to_string(),
                run_id,
                config: config_path,
                surface: Surface::Web,
                initial_url: config.initial_url.as_str().to_string(),
                allowed_origins: config
                    .allowed_origins
                    .iter()
                    .map(|url| url.origin().ascii_serialization())
                    .collect(),
                allowed_domains: config.allowed_domains.clone(),
                started_at_ms,
                finished_at_ms: unix_ms(),
                result: failed_agent_result(&config, error),
                verification: Vec::new(),
                cleanup_error: None,
            };
            redact_report(&redactor, &mut report)?;
            write_report(&report_path, &report, args.force).await?;
            emit_report(args.json, &report, &report_path)?;
            return Ok(agent_status_exit_code(report.result.status));
        }
    };

    let initial_step = TestStep {
        id: "agent-host-initial-navigation".to_string(),
        action: Action::Navigate {
            url: config.initial_url.as_str().to_string(),
        },
    };
    let initial_error = tokio::select! {
        biased;
        () = cancellation.cancelled() => Some(DriverError::new(
            "test.agent.cancelled",
            "agent run was cancelled during initial navigation",
        )),
        result = tokio::time::timeout_at(workflow_deadline, session.execute(&initial_step)) => {
            match result {
                Ok(Ok(_)) => None,
                Ok(Err(error)) => Some(error),
                Err(_) => Some(DriverError::new(
                    "test.agent.timeout",
                    "agent run deadline expired during initial navigation",
                )),
            }
        },
    };
    let (mut result, verification) = if let Some(error) = initial_error {
        (
            failed_agent_result(&config, error),
            Vec::<VerificationStepResult>::new(),
        )
    } else {
        let result = match tokio::time::timeout_at(
            workflow_deadline,
            agent.run(
                &config.goal,
                Surface::Web,
                &mut session,
                cancellation.clone(),
            ),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => failed_agent_result(
                &config,
                DriverError::new("test.agent.timeout", "agent run deadline expired"),
            ),
        };
        let verification = if result.status == AgentStatus::Succeeded {
            verify(&config, &mut session, &cancellation, workflow_deadline).await
        } else {
            Vec::new()
        };
        (result, verification)
    };
    apply_verification_outcome(&mut result, &verification);

    let cleanup_error = match tokio::time::timeout(
        Duration::from_millis(args.cleanup_timeout_ms),
        session.close(),
    )
    .await
    {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(HostError::from_driver(error)),
        Err(_) => Some(HostError {
            code: "test.agent.cleanup_timeout".to_string(),
            message: "Web surface cleanup exceeded its deadline".to_string(),
            retryable: false,
        }),
    };
    signal_task.abort();
    let _ = signal_task.await;
    if cleanup_error.is_some() && result.status == AgentStatus::Succeeded {
        result.status = AgentStatus::Failed;
        result.summary = None;
        result.error = Some(a3s_test_agent::AgentError::new(
            "test.agent.cleanup_failed",
            "the agent workflow passed, but exact Web surface cleanup failed",
        ));
    }

    let mut report = AgentHostReport {
        protocol: "a3s.test.agent-run/1".to_string(),
        run_id,
        config: config_path,
        surface: Surface::Web,
        initial_url: config.initial_url.as_str().to_string(),
        allowed_origins: config
            .allowed_origins
            .iter()
            .map(|url| url.origin().ascii_serialization())
            .collect(),
        allowed_domains: config.allowed_domains,
        started_at_ms,
        finished_at_ms: unix_ms(),
        result,
        verification,
        cleanup_error,
    };
    redact_report(&redactor, &mut report)?;
    write_report(&report_path, &report, args.force).await?;
    emit_report(args.json, &report, &report_path)?;
    Ok(agent_status_exit_code(report.result.status))
}

async fn verify(
    config: &AgentRunConfig,
    session: &mut dyn DriverSession,
    cancellation: &CancellationToken,
    deadline: tokio::time::Instant,
) -> Vec<VerificationStepResult> {
    let steps = &config.verification.scenarios[0].steps;
    let mut results = Vec::with_capacity(steps.len());
    for step in steps {
        let execution = tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(DriverError::new(
                "test.agent.cancelled",
                "agent run was cancelled during deterministic verification",
            )),
            result = tokio::time::timeout_at(deadline, session.execute(step)) => match result {
                Ok(result) => result,
                Err(_) => Err(DriverError::new(
                    "test.agent.verification_timeout",
                    "deterministic verification exceeded the agent deadline",
                )),
            },
        };
        match execution {
            Ok(output) => results.push(VerificationStepResult {
                id: step.id.clone(),
                output: Some(output),
                error: None,
            }),
            Err(error) => {
                results.push(VerificationStepResult {
                    id: step.id.clone(),
                    output: None,
                    error: Some(HostError::from_driver(error)),
                });
                break;
            }
        }
    }
    results
}

fn apply_verification_outcome(
    result: &mut AgentRunResult,
    verification: &[VerificationStepResult],
) {
    if result.status != AgentStatus::Succeeded {
        return;
    }
    let Some(error) = verification.iter().find_map(|step| step.error.as_ref()) else {
        return;
    };

    result.summary = None;
    match error.code.as_str() {
        "test.agent.cancelled" => {
            result.status = AgentStatus::Cancelled;
            result.error = Some(
                a3s_test_agent::AgentError::new(&error.code, &error.message)
                    .with_retryable(error.retryable),
            );
        }
        "test.agent.timeout" | "test.agent.verification_timeout" => {
            result.status = AgentStatus::TimedOut;
            result.error = Some(
                a3s_test_agent::AgentError::new(&error.code, &error.message)
                    .with_retryable(error.retryable),
            );
        }
        _ => {
            result.status = AgentStatus::Failed;
            result.error = Some(a3s_test_agent::AgentError::new(
                "test.agent.verification_failed",
                "the model finished, but deterministic local verification failed",
            ));
        }
    }
}

fn failed_agent_result(config: &AgentRunConfig, error: DriverError) -> AgentRunResult {
    let status = match error.code() {
        "test.agent.cancelled" => AgentStatus::Cancelled,
        "test.agent.timeout" => AgentStatus::TimedOut,
        _ => AgentStatus::Failed,
    };
    let retryable = error.retryable();
    AgentRunResult {
        provider: config.provider.clone(),
        prompt_version: a3s_test_agent::AGENT_PROMPT_VERSION.to_string(),
        status,
        summary: None,
        usage: Default::default(),
        turns: Vec::new(),
        error: Some(
            a3s_test_agent::AgentError::new(error.code(), error.message())
                .with_retryable(retryable),
        ),
    }
}

async fn read_config(path: &Path) -> Result<(PathBuf, AgentRunConfig)> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect agent run config {}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        anyhow::bail!("agent run config must be a regular non-symbolic-link file");
    }
    if metadata.len() == 0 || metadata.len() > MAX_CONFIG_BYTES {
        anyhow::bail!("agent run config must contain 1 to {MAX_CONFIG_BYTES} bytes");
    }
    let canonical = tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("failed to resolve agent run config {}", path.display()))?;
    let source = tokio::fs::read_to_string(&canonical)
        .await
        .with_context(|| format!("failed to read agent run config {}", canonical.display()))?;
    let config = parse_config(&source)?;
    Ok((canonical, config))
}

fn read_authorization(config: &AgentRunConfig) -> Result<Option<String>> {
    config
        .authorization_env
        .as_ref()
        .map(|name| {
            std::env::var(name).with_context(|| {
                format!("provider authorization environment variable {name} is not set")
            })
        })
        .transpose()
}

fn authorization_secrets(value: &str) -> Vec<&str> {
    let mut secrets = vec![value];
    if let Some((_, credential)) = value.split_once(' ') {
        if credential.len() >= 8 {
            secrets.push(credential);
        }
    }
    secrets
}

fn observed_web_url(value: &serde_json::Value) -> Option<&str> {
    [
        "/data/origin",
        "/data/url",
        "/data/value/origin",
        "/data/value/url",
        "/origin",
        "/url",
    ]
    .into_iter()
    .find_map(|pointer| value.pointer(pointer).and_then(serde_json::Value::as_str))
}

fn redact_report(redactor: &ProvenanceRedactor, report: &mut AgentHostReport) -> Result<()> {
    let mut value = serde_json::to_value(&*report)?;
    redactor.redact_json(&mut value);
    *report = serde_json::from_value(value)?;
    Ok(())
}

async fn write_report(path: &Path, report: &AgentHostReport, _force: bool) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    if bytes.len() > MAX_REPORT_BYTES {
        anyhow::bail!("agent run report exceeds {MAX_REPORT_BYTES} bytes");
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("failed to create report directory {}", parent.display()))?;
    let metadata = tokio::fs::symlink_metadata(parent)
        .await
        .with_context(|| format!("failed to inspect report directory {}", parent.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        anyhow::bail!("agent run report directory must be a non-symbolic-link directory");
    }
    ensure_report_target(path, _force)?;
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .context("agent run report path must have a file name")?
            .to_string_lossy(),
        std::process::id(),
        sequence,
    ));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .context("failed to create temporary agent run report")?;
    use tokio::io::AsyncWriteExt;
    if let Err(error) = file.write_all(&bytes).await {
        drop(file);
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error).context("failed to write temporary agent run report");
    }
    drop(file);
    ensure_report_target(path, _force)?;
    #[cfg(windows)]
    if _force && path.exists() {
        tokio::fs::remove_file(path)
            .await
            .with_context(|| format!("failed to replace agent run report {}", path.display()))?;
    }
    if let Err(error) = tokio::fs::rename(&temporary, path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error)
            .with_context(|| format!("failed to publish agent run report {}", path.display()));
    }
    Ok(())
}

fn ensure_report_target(path: &Path, force: bool) -> Result<()> {
    if path.file_name().is_none() {
        anyhow::bail!("agent run report path must have a file name");
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("agent run report output must not be a symbolic link");
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            anyhow::bail!("agent run report output must be a regular file");
        }
        Ok(_) if !force => {
            anyhow::bail!("agent run report already exists; pass --force to replace it");
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("failed to inspect agent run report output"),
    }
    Ok(())
}

fn ensure_report_outside_artifacts(report: &Path, artifacts: &Path) -> Result<()> {
    let current = std::env::current_dir().context("failed to resolve current directory")?;
    let report = lexical_absolute(report, &current);
    let artifacts = lexical_absolute(artifacts, &current);
    if report.starts_with(&artifacts) {
        anyhow::bail!("agent run report must be outside its browser artifact directory");
    }
    Ok(())
}

fn lexical_absolute(path: &Path, current: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        current.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn emit_report(json: bool, report: &AgentHostReport, report_path: &Path) -> Result<()> {
    if json {
        let value = serde_json::json!({
            "protocol": report.protocol,
            "run_id": report.run_id,
            "status": report.result.status,
            "report": report,
            "report_path": report_path,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "{}: {} ({})",
            if report.result.status == AgentStatus::Succeeded {
                "PASS"
            } else {
                "FAIL"
            },
            report.run_id,
            report_path.display()
        );
    }
    Ok(())
}

fn agent_status_exit_code(status: AgentStatus) -> ExitCode {
    match status {
        AgentStatus::Succeeded => ExitCode::SUCCESS,
        AgentStatus::TimedOut => ExitCode::from(124),
        AgentStatus::Cancelled => ExitCode::from(130),
        AgentStatus::Failed | AgentStatus::PolicyDenied | AgentStatus::BudgetExceeded => {
            ExitCode::from(1)
        }
    }
}

fn new_run_id() -> String {
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("agent-{}-{sequence}", std::process::id())
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

async fn canonical_workspace() -> Result<PathBuf> {
    let current = std::env::current_dir().context("failed to resolve current workspace")?;
    tokio::fs::canonicalize(&current)
        .await
        .with_context(|| format!("failed to canonicalize workspace {}", current.display()))
}

impl HostError {
    fn from_driver(error: DriverError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.message().to_string(),
            retryable: error.retryable(),
        }
    }
}

#[cfg(test)]
mod tests;
