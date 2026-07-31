mod events;
mod policy;
mod runtime;
mod store;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use a3s_test_core::{Action, DriverError, Surface, Target};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, AgentBrowserSession,
    BrowserCommand,
};
use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::json;
use url::Url;

use self::events::{append_success_event, append_terminal_event, record_failure};
use self::policy::validate_action;
use self::runtime::{
    create_runtime_directory, remove_runtime_directory, session_namespace,
    validate_runtime_directory,
};
use self::store::{
    AgentSessionError, AgentSessionReport, AgentSessionState, AgentSessionStatus,
    AgentSessionStore, StoredBrowserConfig, StoredBrowserDriver, SESSION_SCHEMA_VERSION,
};
use super::{validate_timeout, BrowserDriverKind};

const AGENT_PROTOCOL_REVISION: u32 = 1;

#[derive(Debug, Args)]
pub(crate) struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    /// Start a persistent Web test session for an external coding agent.
    #[command(alias = "open")]
    Start(StartArgs),
    /// Capture the next semantic observation from an active session.
    #[command(alias = "snapshot")]
    Observe(ObserveArgs),
    /// Execute one schema-validated action in an active session.
    Act(ActArgs),
    /// Click a ref or CSS target in an active session.
    Click(ClickArgs),
    /// Replace the value of a ref or CSS target in an active session.
    Fill(FillArgs),
    /// Send one key or key chord to the active session.
    Press(PressArgs),
    /// Capture a screenshot inside the session artifact directory.
    Screenshot(ScreenshotArgs),
    /// Finish the session, close its surface, and write a report.
    Finish(FinishArgs),
    /// Abort an active session and close only its owned surface.
    Abort(SessionArgs),
    /// Show the persisted state for one session.
    Show(SessionArgs),
    /// List sessions in the current workspace.
    List(ListArgs),
    /// Print the external-planner protocol and typed action schema.
    Schema(SchemaArgs),
}

#[derive(Debug, Args)]
struct StartArgs {
    /// Initial Web URL.
    url: String,
    /// Stable workspace-local session identifier.
    #[arg(long)]
    session: String,
    /// Concrete test goal for the coding agent.
    #[arg(long)]
    goal: String,
    /// Observable success criterion. Repeat for multiple criteria.
    #[arg(long = "success", required = true)]
    success_criteria: Vec<String>,
    /// Additional navigation origin allowed during this session.
    #[arg(long = "allow-origin")]
    allowed_origins: Vec<String>,
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
    /// Browser daemon inactivity deadline between agent turns.
    #[arg(long, default_value_t = 300_000)]
    idle_timeout_ms: u64,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ObserveArgs {
    /// Active session identifier.
    #[arg(long)]
    session: String,
    /// Return only interactive elements when supported.
    #[arg(long)]
    interactive: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ActArgs {
    /// Active session identifier.
    #[arg(long)]
    session: String,
    /// One JSON object matching the Action schema.
    #[arg(long = "action-json")]
    action_json: String,
    /// Observation identifier that supplied any ref target used by the action.
    #[arg(long)]
    observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ClickArgs {
    /// Ref such as @e3, or an explicit CSS selector.
    target: String,
    /// Active session identifier.
    #[arg(long)]
    session: String,
    /// Observation identifier that supplied a ref target.
    #[arg(long)]
    observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct FillArgs {
    /// Ref such as @e3, or an explicit CSS selector.
    target: String,
    /// Replacement value.
    value: String,
    /// Active session identifier.
    #[arg(long)]
    session: String,
    /// Observation identifier that supplied a ref target.
    #[arg(long)]
    observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PressArgs {
    /// Key or key chord, for example Enter or Meta+z.
    key: String,
    /// Active session identifier.
    #[arg(long)]
    session: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ScreenshotArgs {
    /// Relative path below the session artifact directory.
    path: String,
    /// Active session identifier.
    #[arg(long)]
    session: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum FinishStatus {
    Passed,
    Failed,
}

#[derive(Debug, Args)]
struct FinishArgs {
    /// Active session identifier.
    #[arg(long)]
    session: String,
    /// Final test status decided from explicit success criteria and evidence.
    #[arg(long, value_enum)]
    status: FinishStatus,
    /// Concise result summary.
    #[arg(long)]
    summary: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SessionArgs {
    /// Session identifier.
    #[arg(long)]
    session: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ListArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SchemaArgs {
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    compact: bool,
}

pub(crate) async fn execute(args: AgentArgs) -> Result<ExitCode> {
    match args.command {
        AgentCommand::Start(args) => start(args).await,
        AgentCommand::Observe(args) => observe(args).await,
        AgentCommand::Act(args) => act(args).await,
        AgentCommand::Click(args) => {
            perform_action(
                args.session,
                selector_action_target(&args.target, ActionTargetUse::Click, None)?,
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Fill(args) => {
            perform_action(
                args.session,
                selector_action_target(&args.target, ActionTargetUse::Fill, Some(args.value))?,
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Press(args) => {
            perform_action(
                args.session,
                Action::Press { key: args.key },
                None,
                args.json,
            )
            .await
        }
        AgentCommand::Screenshot(args) => {
            perform_action(
                args.session,
                Action::Screenshot { path: args.path },
                None,
                args.json,
            )
            .await
        }
        AgentCommand::Finish(args) => finish(args).await,
        AgentCommand::Abort(args) => abort(args).await,
        AgentCommand::Show(args) => show(args).await,
        AgentCommand::List(args) => list(args).await,
        AgentCommand::Schema(args) => schema(args),
    }
}

async fn start(args: StartArgs) -> Result<ExitCode> {
    validate_session_id(&args.session)?;
    validate_timeout(args.command_timeout_ms, "command timeout")?;
    validate_timeout(args.idle_timeout_ms, "idle timeout")?;
    if args.goal.trim().is_empty() {
        anyhow::bail!("agent test goal must not be empty");
    }
    if args
        .success_criteria
        .iter()
        .any(|criterion| criterion.trim().is_empty())
    {
        anyhow::bail!("success criteria must not be empty");
    }

    let workspace = canonical_workspace().await?;
    let store = AgentSessionStore::for_workspace(&workspace, &args.session);
    if store.exists() {
        let existing = load_session_state(&store, &workspace, &args.session).await?;
        anyhow::bail!(
            "agent session '{}' already exists with status {:?}; use a new id or inspect it with `a3s-test agent show`",
            args.session,
            existing.status
        );
    }

    let initial_url = Url::parse(&args.url).context("initial Web URL is invalid")?;
    let mut allowed_origins = BTreeSet::from([web_origin(&initial_url)?]);
    for origin in &args.allowed_origins {
        let parsed =
            Url::parse(origin).with_context(|| format!("allowed origin '{origin}' is invalid"))?;
        allowed_origins.insert(web_origin(&parsed)?);
    }

    store.create_directories().await?;
    let runtime_dir = create_runtime_directory(&workspace, &args.session).await?;
    let now = unix_ms();
    let executable = args.browser_executable.clone().unwrap_or_else(|| {
        PathBuf::from(match args.browser_driver {
            BrowserDriverKind::A3s => "a3s",
            BrowserDriverKind::Standalone => "agent-browser",
        })
    });
    let mut state = AgentSessionState {
        schema_version: SESSION_SCHEMA_VERSION,
        session: args.session.clone(),
        workspace: workspace.clone(),
        surface: Surface::Web,
        status: AgentSessionStatus::Active,
        goal: args.goal,
        success_criteria: args.success_criteria,
        allowed_origins: allowed_origins.into_iter().collect(),
        browser: StoredBrowserConfig {
            driver: match args.browser_driver {
                BrowserDriverKind::A3s => StoredBrowserDriver::A3s,
                BrowserDriverKind::Standalone => StoredBrowserDriver::Standalone,
            },
            executable,
            headed: args.headed,
            command_timeout_ms: args.command_timeout_ms,
            idle_timeout_ms: args.idle_timeout_ms,
        },
        namespace: session_namespace(&workspace, &args.session),
        driver_session: format!("agent-{}", args.session),
        runtime_dir,
        artifacts_dir: store.artifacts_dir().to_path_buf(),
        active_video_path: None,
        next_sequence: 1,
        next_observation_id: 1,
        latest_observation: None,
        started_at_ms: now,
        updated_at_ms: now,
        summary: None,
    };

    let mut browser = match connect(&state).await {
        Ok(browser) => browser,
        Err(error) => {
            cleanup_failed_start(&store, &state).await;
            return Err(error);
        }
    };
    let action = Action::Navigate {
        url: args.url.clone(),
    };
    let output = match browser.execute_action("agent-start", action.clone()).await {
        Ok(output) => output,
        Err(error) => {
            let _ = browser.close_surface().await;
            cleanup_failed_start(&store, &state).await;
            return Err(anyhow::Error::new(error));
        }
    };
    state.active_video_path = browser.active_video_path().map(str::to_string);
    let persistence = async {
        append_success_event(&store, &mut state, "start", None, action, output.clone()).await?;
        store.save(&state).await
    }
    .await;
    if let Err(error) = persistence {
        let _ = browser.close_surface().await;
        cleanup_failed_start(&store, &state).await;
        return Err(error);
    }

    emit(
        args.json,
        json!({
            "protocol_revision": AGENT_PROTOCOL_REVISION,
            "session": state.session,
            "status": state.status,
            "goal": state.goal,
            "success_criteria": state.success_criteria,
            "allowed_origins": state.allowed_origins,
            "output": output,
            "artifacts_dir": state.artifacts_dir,
            "next": format!("a3s-test agent observe --session {} --interactive --json", state.session),
        }),
        format!(
            "Started agent test session '{}' for {}",
            state.session, args.url
        ),
    )?;
    Ok(ExitCode::SUCCESS)
}

async fn observe(args: ObserveArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_active(&store, &workspace, &args.session).await?;
    let mut browser = connect(&state).await?;
    let action = Action::Snapshot {
        interactive: args.interactive,
    };
    match browser
        .execute_action(
            format!("agent-observe-{}", state.next_sequence),
            action.clone(),
        )
        .await
    {
        Ok(output) => {
            let observation_id = state.next_observation_id;
            state.next_observation_id = state
                .next_observation_id
                .checked_add(1)
                .context("agent observation sequence exhausted")?;
            state.latest_observation = Some(observation_id);
            state.active_video_path = browser.active_video_path().map(str::to_string);
            append_success_event(
                &store,
                &mut state,
                "observe",
                Some(observation_id),
                action,
                output.clone(),
            )
            .await?;
            store.save(&state).await?;
            emit(
                args.json,
                json!({
                    "protocol_revision": AGENT_PROTOCOL_REVISION,
                    "session": state.session,
                    "status": state.status,
                    "observation_id": observation_id,
                    "output": output,
                    "next": format!(
                        "a3s-test agent act --session {} --observation {} --action-json '<Action JSON>' --json",
                        state.session, observation_id
                    ),
                }),
                format!(
                    "Observation {observation_id} captured for '{}'",
                    state.session
                ),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            state.active_video_path = browser.active_video_path().map(str::to_string);
            record_failure(&store, &mut state, "observe", Some(action), &error).await?;
            emit_driver_error(args.json, &state, error)
        }
    }
}

async fn act(args: ActArgs) -> Result<ExitCode> {
    let action: Action = serde_json::from_str(&args.action_json)
        .context("action JSON does not match the typed Action schema")?;
    if matches!(action, Action::Snapshot { .. }) {
        anyhow::bail!("use `a3s-test agent observe` for snapshots");
    }
    perform_action(args.session, action, args.observation, args.json).await
}

async fn perform_action(
    session: String,
    action: Action,
    observation: Option<u64>,
    json_output: bool,
) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &session)?;
    let mut state = load_active(&store, &workspace, &session).await?;
    validate_action(&state, &action, observation)?;

    let mut browser = connect(&state).await?;
    match browser
        .execute_action(format!("agent-act-{}", state.next_sequence), action.clone())
        .await
    {
        Ok(output) => {
            state.latest_observation = None;
            state.active_video_path = browser.active_video_path().map(str::to_string);
            append_success_event(&store, &mut state, "act", None, action, output.clone()).await?;
            store.save(&state).await?;
            emit(
                json_output,
                json!({
                    "protocol_revision": AGENT_PROTOCOL_REVISION,
                    "session": state.session,
                    "status": state.status,
                    "output": output,
                    "next": format!(
                        "a3s-test agent observe --session {} --interactive --json",
                        state.session
                    ),
                }),
                format!("Action completed in '{}'", state.session),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            state.latest_observation = None;
            state.active_video_path = browser.active_video_path().map(str::to_string);
            record_failure(&store, &mut state, "act", Some(action), &error).await?;
            emit_driver_error(json_output, &state, error)
        }
    }
}

enum ActionTargetUse {
    Click,
    Fill,
}

fn selector_action_target(
    raw_target: &str,
    use_kind: ActionTargetUse,
    value: Option<String>,
) -> Result<Action> {
    if raw_target.trim().is_empty() {
        anyhow::bail!("target must not be empty");
    }
    let target = if raw_target.starts_with("@e")
        && raw_target.strip_prefix("@e").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
        }) {
        Target::Ref {
            value: raw_target.to_string(),
        }
    } else {
        Target::Css {
            selector: raw_target.to_string(),
        }
    };
    match use_kind {
        ActionTargetUse::Click => Ok(Action::Click { target }),
        ActionTargetUse::Fill => Ok(Action::Fill {
            target,
            value: value.ok_or_else(|| anyhow::anyhow!("fill value is required"))?,
        }),
    }
}

async fn finish(args: FinishArgs) -> Result<ExitCode> {
    if args.summary.trim().is_empty() {
        anyhow::bail!("agent test summary must not be empty");
    }
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_active(&store, &workspace, &args.session).await?;
    let mut browser = connect(&state).await?;
    let cleanup_error = close_and_remove_runtime(&mut browser, &state).await;
    state.active_video_path = None;
    state.status = match args.status {
        FinishStatus::Passed => AgentSessionStatus::Passed,
        FinishStatus::Failed => AgentSessionStatus::Failed,
    };
    state.summary = Some(args.summary.clone());
    state.latest_observation = None;
    state.updated_at_ms = unix_ms();

    if let Some(error) = &cleanup_error {
        state.status = AgentSessionStatus::Failed;
        record_failure(&store, &mut state, "finish", None, error).await?;
    } else {
        append_terminal_event(&store, &mut state, "finish").await?;
    }
    store.save(&state).await?;
    let report = AgentSessionReport {
        schema_version: SESSION_SCHEMA_VERSION,
        session: state.session.clone(),
        surface: state.surface,
        status: state.status,
        goal: state.goal.clone(),
        success_criteria: state.success_criteria.clone(),
        allowed_origins: state.allowed_origins.clone(),
        event_count: state.next_sequence.saturating_sub(1),
        artifacts_dir: state.artifacts_dir.clone(),
        events_path: store.events_path().to_path_buf(),
        started_at_ms: state.started_at_ms,
        finished_at_ms: state.updated_at_ms,
        summary: args.summary,
    };
    store.write_report(&report).await?;

    emit(
        args.json,
        json!({
            "protocol_revision": AGENT_PROTOCOL_REVISION,
            "session": state.session,
            "status": state.status,
            "report": report,
            "report_path": store.report_path(),
            "cleanup_error": cleanup_error.map(|error| AgentSessionError {
                code: error.code().to_string(),
                message: error.message().to_string(),
            }),
        }),
        format!(
            "Finished agent test session '{}' with status {:?}",
            state.session, state.status
        ),
    )?;
    Ok(match state.status {
        AgentSessionStatus::Passed => ExitCode::SUCCESS,
        AgentSessionStatus::Failed => ExitCode::from(1),
        AgentSessionStatus::Active | AgentSessionStatus::Aborted => ExitCode::from(2),
    })
}

async fn abort(args: SessionArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_session_state(&store, &workspace, &args.session).await?;
    let cleanup_error = if state.runtime_dir.exists() {
        let mut browser = connect(&state).await?;
        close_and_remove_runtime(&mut browser, &state).await
    } else {
        None
    };
    if cleanup_error.is_none() {
        if state.status == AgentSessionStatus::Active {
            state.status = AgentSessionStatus::Aborted;
            state.summary = Some("Agent session aborted by the caller".to_string());
            append_terminal_event(&store, &mut state, "abort").await?;
        }
        state.active_video_path = None;
        state.latest_observation = None;
        store.save(&state).await?;
    } else if let Some(error) = &cleanup_error {
        state.summary = Some("Agent session abort cleanup failed".to_string());
        record_failure(&store, &mut state, "abort", None, error).await?;
    }
    emit(
        args.json,
        json!({
            "protocol_revision": AGENT_PROTOCOL_REVISION,
            "session": state.session,
            "status": state.status,
            "cleanup_error": cleanup_error.as_ref().map(|error| AgentSessionError {
                code: error.code().to_string(),
                message: error.message().to_string(),
            }),
        }),
        format!(
            "Agent test session '{}' is {:?}",
            state.session, state.status
        ),
    )?;
    Ok(if cleanup_error.is_some() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

async fn show(args: SessionArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let state = load_session_state(&store, &workspace, &args.session).await?;
    emit(
        args.json,
        serde_json::to_value(&state)?,
        format!("{}: {:?} — {}", state.session, state.status, state.goal),
    )?;
    Ok(ExitCode::SUCCESS)
}

async fn list(args: ListArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let root = AgentSessionStore::sessions_root(&workspace);
    let mut sessions = Vec::new();
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            emit(args.json, json!([]), "No agent test sessions".to_string())?;
            return Ok(ExitCode::SUCCESS);
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to list {}", root.display()));
        }
    };
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let Some(session) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let store = AgentSessionStore::for_workspace(&workspace, &session);
        if let Ok(state) = load_session_state(&store, &workspace, &session).await {
            sessions.push(state);
        }
    }
    sessions.sort_by(|left, right| left.session.cmp(&right.session));
    let human = if sessions.is_empty() {
        "No agent test sessions".to_string()
    } else {
        sessions
            .iter()
            .map(|state| format!("{}: {:?}", state.session, state.status))
            .collect::<Vec<_>>()
            .join("\n")
    };
    emit(args.json, serde_json::to_value(&sessions)?, human)?;
    Ok(ExitCode::SUCCESS)
}

fn schema(args: SchemaArgs) -> Result<ExitCode> {
    let schema = json!({
        "protocol_revision": AGENT_PROTOCOL_REVISION,
        "planner": "external_coding_agent",
        "turns": [
            "start",
            "observe",
            "act",
            "observe",
            "finish"
        ],
        "invariants": {
            "typed_actions": true,
            "ref_targets_require_latest_observation": true,
            "explicit_navigation_is_origin_scoped": true,
            "sessions_are_workspace_local": true,
            "evidence_is_session_scoped": true
        },
        "action_schema": schemars::schema_for!(Action),
    });
    if args.compact {
        println!("{}", serde_json::to_string(&schema)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&schema)?);
    }
    Ok(ExitCode::SUCCESS)
}

async fn connect(state: &AgentSessionState) -> Result<AgentBrowserSession> {
    validate_timeout(state.browser.command_timeout_ms, "command timeout")?;
    validate_timeout(state.browser.idle_timeout_ms, "idle timeout")?;
    let command = match state.browser.driver {
        StoredBrowserDriver::A3s => BrowserCommand::A3s {
            executable: state.browser.executable.clone(),
        },
        StoredBrowserDriver::Standalone => BrowserCommand::Standalone {
            executable: state.browser.executable.clone(),
        },
    };
    let driver = AgentBrowserDriver::new(AgentBrowserConfig {
        command,
        namespace: state.namespace.clone(),
        headed: state.browser.headed,
        command_timeout: Duration::from_millis(state.browser.command_timeout_ms),
        idle_timeout: Duration::from_millis(state.browser.idle_timeout_ms),
    });
    driver
        .connect(AgentBrowserConnectionConfig {
            namespace: state.namespace.clone(),
            session: state.driver_session.clone(),
            runtime_dir: state.runtime_dir.clone(),
            artifacts_dir: state.artifacts_dir.clone(),
            active_video_path: state.active_video_path.clone(),
        })
        .await
        .map_err(anyhow::Error::new)
}

async fn close_and_remove_runtime(
    browser: &mut AgentBrowserSession,
    state: &AgentSessionState,
) -> Option<DriverError> {
    if let Err(error) = browser.close_surface().await {
        return Some(error);
    }
    remove_runtime_directory(&state.runtime_dir, &state.workspace, &state.session)
        .await
        .err()
        .map(|error| {
            DriverError::new(
                "test.session.runtime_cleanup_failed",
                format!("browser closed but runtime cleanup failed: {error:#}"),
            )
        })
}

fn emit_driver_error(
    json_output: bool,
    state: &AgentSessionState,
    error: a3s_test_core::DriverError,
) -> Result<ExitCode> {
    emit(
        json_output,
        json!({
            "protocol_revision": AGENT_PROTOCOL_REVISION,
            "session": state.session,
            "status": state.status,
            "error": AgentSessionError {
                code: error.code().to_string(),
                message: error.message().to_string(),
            },
            "next": format!(
                "a3s-test agent observe --session {} --interactive --json",
                state.session
            ),
        }),
        format!("{}: {}", error.code(), error.message()),
    )?;
    Ok(ExitCode::from(1))
}

fn load_store(workspace: &Path, session: &str) -> Result<AgentSessionStore> {
    validate_session_id(session)?;
    let store = AgentSessionStore::for_workspace(workspace, session);
    if !store.exists() {
        anyhow::bail!("agent session '{session}' does not exist in this workspace");
    }
    Ok(store)
}

async fn load_session_state(
    store: &AgentSessionStore,
    workspace: &Path,
    session: &str,
) -> Result<AgentSessionState> {
    let state = store.load().await?;
    if state.workspace != workspace
        || state.session != session
        || state.surface != Surface::Web
        || state.namespace != session_namespace(workspace, session)
        || state.driver_session != format!("agent-{session}")
        || state.artifacts_dir != store.artifacts_dir()
    {
        anyhow::bail!(
            "agent session '{}' metadata does not match the current workspace",
            session
        );
    }
    validate_runtime_directory(
        &state.runtime_dir,
        workspace,
        session,
        state.status == AgentSessionStatus::Active,
    )
    .await?;
    Ok(state)
}

async fn load_active(
    store: &AgentSessionStore,
    workspace: &Path,
    session: &str,
) -> Result<AgentSessionState> {
    let state = load_session_state(store, workspace, session).await?;
    if state.status != AgentSessionStatus::Active {
        anyhow::bail!(
            "agent session '{}' is {:?}, not active",
            state.session,
            state.status
        );
    }
    Ok(state)
}

fn validate_session_id(session: &str) -> Result<()> {
    if session.is_empty()
        || session.len() > 48
        || !session
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("session id must be 1-48 ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}

fn web_origin(url: &Url) -> Result<String> {
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("agent Web sessions allow only http and https URLs");
    }
    Ok(url.origin().ascii_serialization())
}

async fn canonical_workspace() -> Result<PathBuf> {
    let current = std::env::current_dir().context("failed to resolve current workspace")?;
    tokio::fs::canonicalize(&current)
        .await
        .with_context(|| format!("failed to canonicalize workspace {}", current.display()))
}

async fn cleanup_failed_start(store: &AgentSessionStore, state: &AgentSessionState) {
    let _ = remove_runtime_directory(&state.runtime_dir, &state.workspace, &state.session).await;
    let _ = tokio::fs::remove_dir_all(store.root()).await;
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

fn emit<T: Serialize>(json_output: bool, value: T, human: String) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{human}");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
