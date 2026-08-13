use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{Action, Surface, SurfaceContractDraft, SurfaceDriver, TestSuite};
use a3s_test_driver_gui::{
    terminate_active_cua_processes, ApplicationIdentity, AttachSpec, CuaEndpoint, GuiAppTarget,
    GuiCaptureScope, GuiDriver, GuiDriverConfig, GuiProfile, LaunchSpec, WindowSelector,
};
use a3s_test_driver_web::{
    terminate_active_commands, AgentBrowserConfig, AgentBrowserDriver, BrowserCapabilities,
    BrowserCommand,
};
use a3s_test_runner::{ContractRegistry, RetryPolicy, RunResult, RunStatus, Runner, RunnerOptions};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

mod action_schema;
mod agent_session;
mod gui_certification;
mod mcp;
mod mcp_web;

#[derive(Debug, Parser)]
#[command(
    name = "a3s-test",
    version,
    about = "AI-native end-to-end testing across Web, GUI, and TUI surfaces"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Drive a persistent test session from A3S Code, Codex, or another agent.
    Agent(agent_session::AgentArgs),
    /// Validate and inspect an ACL test suite without launching a surface.
    Check(CheckArgs),
    /// Discover and verify the installed Web driver protocol.
    Capabilities(CapabilitiesArgs),
    /// Print the locked GUI platform and endpoint certification matrix.
    GuiCertification(gui_certification::GuiCertificationArgs),
    /// Exercise one real GUI profile and verify observation plus owned cleanup.
    GuiCertify(gui_certification::GuiCertifyArgs),
    /// Serve surface-neutral agent sessions over MCP stdio.
    Mcp(McpArgs),
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
    /// Show the browser window; omitted runs enforce headless execution.
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
    #[command(flatten)]
    gui: GuiRunArgs,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct McpArgs {
    #[command(flatten)]
    gui: GuiRunArgs,
    /// Initial Web URL fixed by the MCP host. Omit to disable Web sessions.
    #[arg(long)]
    web_url: Option<String>,
    /// Browser driver integration used by Web MCP sessions.
    #[arg(long, value_enum, default_value_t = BrowserDriverKind::A3s)]
    browser_driver: BrowserDriverKind,
    /// Override the browser driver executable used by Web MCP sessions.
    #[arg(long)]
    browser_executable: Option<PathBuf>,
    /// Show Web MCP browser windows.
    #[arg(long)]
    headed: bool,
    /// Additional hostname admitted by the Web MCP browser network filter.
    #[arg(long = "web-allow-domain")]
    web_allowed_domains: Vec<String>,
    /// Browser daemon inactivity deadline between MCP turns.
    #[arg(long, default_value_t = 300_000)]
    idle_timeout_ms: u64,
    /// Per-command Web or CUA deadline.
    #[arg(long, default_value_t = 30_000)]
    command_timeout_ms: u64,
    /// Bounded cleanup deadline for each MCP session.
    #[arg(long, default_value_t = 10_000)]
    cleanup_timeout_ms: u64,
    /// Maximum surface sessions held by this MCP server.
    #[arg(long, default_value_t = 4)]
    max_sessions: usize,
    /// Artifact root for MCP sessions.
    #[arg(long, default_value = ".a3s-test/mcp-sessions")]
    artifacts_root: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GuiTargetMode {
    /// Launch a new application instance and own its cleanup.
    Launch,
    /// Attach to an already-running application and never terminate it.
    Attach,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GuiProfileArg {
    /// Accessibility semantics without an automatic screenshot on observation.
    Semantic,
    /// Accessibility semantics plus a SHA-256-bound window screenshot.
    WindowVision,
}

#[derive(Debug, Args)]
struct GuiRunArgs {
    /// Absolute or workspace-relative CUA policy file required by GUI scenarios.
    #[arg(long)]
    gui_policy_file: Option<PathBuf>,
    /// CUA MCP proxy executable.
    #[arg(long)]
    cua_proxy_executable: Option<PathBuf>,
    /// Connect the proxy to this embedded CUA socket instead of the installed daemon.
    #[arg(long)]
    cua_embedded_socket: Option<PathBuf>,
    /// macOS bundle identifier for the GUI application.
    #[arg(long)]
    gui_macos_bundle_id: Option<String>,
    /// Launch a new app or attach to a running app.
    #[arg(long, value_enum, default_value_t = GuiTargetMode::Launch)]
    gui_target_mode: GuiTargetMode,
    /// GUI perception profile.
    #[arg(long, value_enum, default_value_t = GuiProfileArg::Semantic)]
    gui_profile: GuiProfileArg,
    /// Existing process ID used in attach mode; omit only when the bundle has one instance.
    #[arg(long)]
    gui_attach_pid: Option<NonZeroU32>,
    /// Application argument used in launch mode. Repeat to pass multiple arguments.
    #[arg(long = "gui-arg")]
    gui_arguments: Vec<std::ffi::OsString>,
    /// Select a top-level window by exact title instead of the primary window.
    #[arg(long, conflicts_with = "gui_window_automation_id")]
    gui_window_title: Option<String>,
    /// Select a top-level window by exact automation ID instead of the primary window.
    #[arg(long, conflicts_with = "gui_window_title")]
    gui_window_automation_id: Option<String>,
}

pub async fn execute(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Commands::Agent(args) => agent_session::execute(args).await,
        Commands::Check(args) => check(args).await,
        Commands::Capabilities(args) => capabilities(args).await,
        Commands::GuiCertification(args) => gui_certification::print_matrix(args),
        Commands::GuiCertify(args) => gui_certification::certify(args).await,
        Commands::Mcp(args) => serve_mcp(args).await,
        Commands::Run(args) => run(args).await,
    }
}

async fn check(args: CheckArgs) -> Result<ExitCode> {
    let admitted = read_suite(&args.manifest).await?;
    let suite = admitted.suite;
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
        network_policy: Default::default(),
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

    let admitted = read_suite(&args.manifest).await?;
    let suite = admitted.suite;
    let has_gui = suite
        .scenarios
        .iter()
        .any(|scenario| scenario.surface == Surface::Gui);
    let command = browser_command(args.browser_driver, args.browser_executable);
    let browser = AgentBrowserDriver::new(AgentBrowserConfig {
        command,
        namespace: String::new(),
        headed: args.headed,
        command_timeout: Duration::from_millis(args.command_timeout_ms),
        idle_timeout: Duration::from_millis(args.idle_timeout_ms),
        network_policy: Default::default(),
    });
    let mut drivers: Vec<Arc<dyn SurfaceDriver>> = vec![Arc::new(browser)];
    if has_gui {
        drivers.push(Arc::new(
            gui_driver(&args.gui, Duration::from_millis(args.command_timeout_ms)).await?,
        ));
    } else if args.gui.requested() {
        anyhow::bail!("GUI options were provided but the suite has no GUI scenarios");
    }
    let runner = Runner::new(
        drivers,
        RunnerOptions {
            cleanup_timeout: Duration::from_millis(args.cleanup_timeout_ms),
            quality_projection_timeout: RunnerOptions::default().quality_projection_timeout,
            retry_policy: RetryPolicy {
                max_retries: args.infrastructure_retries,
                backoff: Duration::from_millis(args.retry_backoff_ms),
            },
            max_parallel_scenarios: args.max_parallel_scenarios,
        },
    )
    .map_err(anyhow::Error::msg)?
    .with_contracts(admitted.contracts);

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

async fn serve_mcp(args: McpArgs) -> Result<ExitCode> {
    validate_timeout(args.command_timeout_ms, "command timeout")?;
    validate_timeout(args.idle_timeout_ms, "idle timeout")?;
    validate_timeout(args.cleanup_timeout_ms, "cleanup timeout")?;
    if !(1..=64).contains(&args.max_sessions) {
        anyhow::bail!("maximum MCP sessions must be between 1 and 64");
    }
    let mut drivers: Vec<Arc<dyn SurfaceDriver>> = Vec::new();
    if let Some(initial_url) = args.web_url.as_ref() {
        let parsed = url::Url::parse(initial_url).context("MCP Web URL is invalid")?;
        if !matches!(parsed.scheme(), "http" | "https") {
            anyhow::bail!("MCP Web URL must use http or https");
        }
        let host = parsed
            .host_str()
            .context("MCP Web URL must contain a hostname")?;
        let mut domains = vec![host.to_string()];
        domains.extend(args.web_allowed_domains.iter().cloned());
        let network_policy =
            a3s_test_driver_web::BrowserNetworkPolicy::restricted_to_domains(domains)
                .map_err(anyhow::Error::new)?;
        let browser = AgentBrowserDriver::new(AgentBrowserConfig {
            command: browser_command(args.browser_driver, args.browser_executable.clone()),
            namespace: String::new(),
            headed: args.headed,
            command_timeout: Duration::from_millis(args.command_timeout_ms),
            idle_timeout: Duration::from_millis(args.idle_timeout_ms),
            network_policy,
        });
        drivers.push(Arc::new(mcp_web::McpWebDriver::new(
            browser,
            initial_url.clone(),
        )));
    } else if args.browser_executable.is_some()
        || args.headed
        || !args.web_allowed_domains.is_empty()
    {
        anyhow::bail!("Web MCP browser options require --web-url");
    }
    if args.gui.requested() {
        drivers.push(Arc::new(
            gui_driver(&args.gui, Duration::from_millis(args.command_timeout_ms)).await?,
        ));
    }
    if drivers.is_empty() {
        anyhow::bail!("MCP requires --web-url, reviewed GUI host options, or both");
    }
    let artifacts_root = if args.artifacts_root.is_absolute() {
        args.artifacts_root
    } else {
        std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(args.artifacts_root)
    };
    let manager = a3s_test_session::AgentSessionManager::new(
        drivers,
        a3s_test_session::SessionManagerOptions {
            artifacts_root,
            cleanup_timeout: Duration::from_millis(args.cleanup_timeout_ms),
            max_sessions: args.max_sessions,
        },
    )
    .map_err(anyhow::Error::new)?;
    mcp::serve(Arc::new(manager)).await?;
    Ok(ExitCode::SUCCESS)
}

impl GuiRunArgs {
    fn requested(&self) -> bool {
        self.gui_policy_file.is_some()
            || self.cua_proxy_executable.is_some()
            || self.cua_embedded_socket.is_some()
            || self.gui_macos_bundle_id.is_some()
            || self.gui_target_mode != GuiTargetMode::Launch
            || self.gui_profile != GuiProfileArg::Semantic
            || self.gui_attach_pid.is_some()
            || !self.gui_arguments.is_empty()
            || self.gui_window_title.is_some()
            || self.gui_window_automation_id.is_some()
    }
}

async fn gui_driver(args: &GuiRunArgs, command_timeout: Duration) -> Result<GuiDriver> {
    let policy = args
        .gui_policy_file
        .as_ref()
        .context("GUI scenarios require --gui-policy-file")?;
    let policy_file = tokio::fs::canonicalize(policy)
        .await
        .with_context(|| format!("failed to resolve GUI policy file {}", policy.display()))?;
    let proxy_executable = args
        .cua_proxy_executable
        .clone()
        .unwrap_or_else(|| PathBuf::from("cua-driver"));
    let endpoint = match &args.cua_embedded_socket {
        Some(socket) => CuaEndpoint::EmbeddedSocket {
            proxy_executable,
            socket: socket.clone(),
        },
        None => CuaEndpoint::InstalledDaemon { proxy_executable },
    };
    let bundle_id = args
        .gui_macos_bundle_id
        .as_ref()
        .context("GUI scenarios require --gui-macos-bundle-id")?
        .clone();
    let application = ApplicationIdentity::MacOsBundle { bundle_id };
    let target = match args.gui_target_mode {
        GuiTargetMode::Launch => {
            if args.gui_attach_pid.is_some() {
                anyhow::bail!("--gui-attach-pid is only valid with --gui-target-mode attach");
            }
            GuiAppTarget::Launch(LaunchSpec {
                application,
                arguments: args.gui_arguments.clone(),
                working_directory: None,
            })
        }
        GuiTargetMode::Attach => {
            if !args.gui_arguments.is_empty() {
                anyhow::bail!("--gui-arg is only valid with --gui-target-mode launch");
            }
            GuiAppTarget::Attach(AttachSpec {
                application,
                process_id: args.gui_attach_pid,
            })
        }
    };
    let window = if let Some(title) = &args.gui_window_title {
        WindowSelector::ExactTitle(title.clone())
    } else if let Some(automation_id) = &args.gui_window_automation_id {
        WindowSelector::AutomationId(automation_id.clone())
    } else {
        WindowSelector::Primary
    };
    let config = GuiDriverConfig {
        endpoint,
        policy_file,
        target,
        window,
        capture_scope: GuiCaptureScope::Window,
        profile: match args.gui_profile {
            GuiProfileArg::Semantic => GuiProfile::Semantic,
            GuiProfileArg::WindowVision => GuiProfile::WindowVision,
        },
        command_timeout,
    };
    config.validate().map_err(anyhow::Error::new)?;
    Ok(GuiDriver::new(config))
}

struct AdmittedSuite {
    suite: TestSuite,
    contracts: ContractRegistry,
}

async fn read_suite(path: &Path) -> Result<AdmittedSuite> {
    let canonical_manifest = tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    let metadata = tokio::fs::metadata(&canonical_manifest)
        .await
        .with_context(|| format!("failed to inspect {}", canonical_manifest.display()))?;
    if !metadata.is_file() {
        anyhow::bail!(
            "test suite must be a regular file: {}",
            canonical_manifest.display()
        );
    }
    let manifest_dir = canonical_manifest
        .parent()
        .context("test suite path does not have a parent directory")?;
    let source = tokio::fs::read_to_string(&canonical_manifest)
        .await
        .with_context(|| format!("failed to read {}", canonical_manifest.display()))?;
    let suite = TestSuite::from_acl(&source)
        .with_context(|| format!("invalid test suite {}", canonical_manifest.display()))?;
    let contracts = read_contracts(&suite, manifest_dir).await?;
    Ok(AdmittedSuite { suite, contracts })
}

async fn read_contracts(suite: &TestSuite, manifest_dir: &Path) -> Result<ContractRegistry> {
    let references = suite
        .scenarios
        .iter()
        .flat_map(|scenario| &scenario.steps)
        .filter_map(|step| match &step.action {
            Action::VerifyContract { contract, .. } => Some(contract.as_str()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut admitted = Vec::with_capacity(references.len());
    for reference in references {
        let relative = admit_contract_reference(reference)?;
        let requested = manifest_dir.join(relative);
        let canonical = tokio::fs::canonicalize(&requested).await.with_context(|| {
            format!(
                "failed to resolve contract reference '{reference}' from {}",
                manifest_dir.display()
            )
        })?;
        if !canonical.starts_with(manifest_dir) {
            anyhow::bail!(
                "contract reference must stay inside the test suite directory: '{reference}'"
            );
        }
        let metadata = tokio::fs::metadata(&canonical)
            .await
            .with_context(|| format!("failed to inspect contract {}", canonical.display()))?;
        if !metadata.is_file() {
            anyhow::bail!("contract reference must resolve to a regular file: '{reference}'");
        }
        let source = tokio::fs::read_to_string(&canonical)
            .await
            .with_context(|| format!("failed to read contract {}", canonical.display()))?;
        let draft = SurfaceContractDraft::from_acl(&source)
            .with_context(|| format!("invalid surface contract {}", canonical.display()))?;
        verify_contract_provenance(&draft, &canonical, manifest_dir).await?;
        let contract = draft.admit().with_context(|| {
            format!("surface contract was not admitted: {}", canonical.display())
        })?;
        admitted.push((reference.to_string(), contract));
    }
    ContractRegistry::new(admitted).map_err(anyhow::Error::msg)
}

async fn verify_contract_provenance(
    draft: &SurfaceContractDraft,
    contract_path: &Path,
    trust_root: &Path,
) -> Result<()> {
    let contract_dir = contract_path
        .parent()
        .context("surface contract path does not have a parent directory")?;
    for entry in draft.provenance() {
        let uri = Path::new(&entry.uri);
        if uri.is_absolute()
            || uri
                .components()
                .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
        {
            anyhow::bail!(
                "contract provenance must stay inside the test suite directory: '{}'",
                entry.uri
            );
        }
        let requested = contract_dir.join(uri);
        let canonical = tokio::fs::canonicalize(&requested).await.with_context(|| {
            format!(
                "failed to resolve provenance '{}' for contract {}",
                entry.uri,
                contract_path.display()
            )
        })?;
        if !canonical.starts_with(trust_root) {
            anyhow::bail!(
                "contract provenance must stay inside the test suite directory: '{}'",
                entry.uri
            );
        }
        let metadata = tokio::fs::metadata(&canonical)
            .await
            .with_context(|| format!("failed to inspect provenance {}", canonical.display()))?;
        if !metadata.is_file() {
            anyhow::bail!(
                "contract provenance must resolve to a regular file: '{}'",
                entry.uri
            );
        }
        let bytes = tokio::fs::read(&canonical)
            .await
            .with_context(|| format!("failed to read provenance {}", canonical.display()))?;
        let actual = format!("sha256:{:x}", Sha256::digest(bytes));
        if entry.digest != actual {
            anyhow::bail!(
                "test.contract.provenance_digest_mismatch: provenance '{}' does not match its declared SHA-256 digest",
                entry.id
            );
        }
    }
    Ok(())
}

fn admit_contract_reference(reference: &str) -> Result<&Path> {
    if reference.trim().is_empty() || reference.len() > 1_024 {
        anyhow::bail!("contract reference must be bounded and non-empty");
    }
    let path = Path::new(reference);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        anyhow::bail!(
            "contract reference must stay inside the test suite directory: '{reference}'"
        );
    }
    Ok(path)
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
            terminate_active_cua_processes();
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

    #[test]
    fn exposes_the_locked_gui_certification_matrix_without_starting_cua() {
        let cli = Cli::try_parse_from(["a3s-test", "gui-certification", "--json"])
            .expect("GUI certification command");
        let Commands::GuiCertification(args) = cli.command else {
            panic!("expected GUI certification command");
        };
        assert!(args.json);

        let matrix =
            a3s_test_driver_gui::GuiCertificationMatrix::locked().expect("certification matrix");
        assert_eq!(matrix.profiles().len(), 6);
    }

    #[tokio::test]
    async fn builds_a_typed_gui_driver_for_run_and_mcp_hosts() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let policy = temp.path().join("policy.yaml");
        tokio::fs::write(&policy, "rules: []")
            .await
            .expect("policy fixture");
        let driver = gui_driver(
            &GuiRunArgs {
                gui_policy_file: Some(policy),
                cua_proxy_executable: Some(PathBuf::from("cua-driver")),
                cua_embedded_socket: None,
                gui_macos_bundle_id: Some("com.example.Editor".to_string()),
                gui_target_mode: GuiTargetMode::Launch,
                gui_profile: GuiProfileArg::WindowVision,
                gui_attach_pid: None,
                gui_arguments: vec![std::ffi::OsString::from("--safe-mode")],
                gui_window_title: Some("Document".to_string()),
                gui_window_automation_id: None,
            },
            Duration::from_secs(2),
        )
        .await
        .expect("GUI driver");

        assert_eq!(driver.surface(), Surface::Gui);
    }
}
