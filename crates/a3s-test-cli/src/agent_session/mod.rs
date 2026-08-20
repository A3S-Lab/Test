mod args;
mod browser;
mod design_audit;
mod dev;
mod events;
mod grounding;
mod inspect;
mod policy;
mod repair;
mod repair_watch;
mod runtime;
mod schema;
mod store;
mod validation;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_test_core::{
    Action, DriverError, RepairStatus, StepOutput, Surface, ACTION_PROTOCOL_REVISION,
};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;
use url::Url;

pub(crate) use self::args::AgentArgs;
use self::args::{
    ActArgs, AgentCommand, FinishArgs, FinishStatus, ListArgs, ObserveArgs, SessionArgs, StartArgs,
};
#[cfg(test)]
use self::browser::stored_browser_network_policy;
use self::browser::{
    close_and_remove_runtime, connect, containment_for_driver,
    validate_turn_browser_network_policy, BrowserConnectionPurpose,
};
pub(crate) use self::dev::{abort_dev_session, start_dev_session, DevSession, DevSessionRequest};
use self::events::{append_success_event, append_terminal_event, record_failure};
use self::policy::{
    browser_network_policy, validate_action, validate_observation_origin, web_origin,
};
pub(crate) use self::repair_watch::{
    watch_session, RepairPickup, RepairWatchRequest, RepairWatchResult,
};
use self::runtime::{
    create_runtime_directory, driver_session_id, remove_runtime_directory, session_namespace,
    validate_runtime_directory,
};
use self::store::{
    AgentSessionError, AgentSessionReport, AgentSessionState, AgentSessionStatus,
    AgentSessionStore, StoredBrowserConfig, StoredBrowserDriver, StoredBrowserMicrophone,
    SESSION_SCHEMA_VERSION,
};
use self::validation::{compact_target, validate_session_id};
use super::{validate_timeout, BrowserDriverKind};
use a3s_test_session::{
    action_uses_page_context_ref, bind_page_context_refs, refresh_page_context_bindings,
    resolve_page_context_refs,
};

pub(crate) async fn execute(args: AgentArgs) -> Result<ExitCode> {
    match args.command {
        AgentCommand::Run(args) => super::agent_host::execute(args).await,
        AgentCommand::Start(args) => start(args).await,
        AgentCommand::Observe(args) => observe(args).await,
        AgentCommand::Inspect(args) => inspect::execute(args).await,
        AgentCommand::Ground(args) => grounding::execute(args).await,
        AgentCommand::Audit(args) => design_audit::execute(args).await,
        AgentCommand::Act(args) => act(args).await,
        AgentCommand::Click(args) => {
            perform_action(
                args.session,
                Action::Click {
                    target: compact_target(&args.target)?,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Hover(args) => {
            perform_action(
                args.session,
                Action::Hover {
                    target: compact_target(&args.target)?,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Focus(args) => {
            perform_action(
                args.session,
                Action::Focus {
                    target: compact_target(&args.target)?,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::DoubleClick(args) => {
            perform_action(
                args.session,
                Action::DoubleClick {
                    target: compact_target(&args.target)?,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::ContextClick(args) => {
            perform_action(
                args.session,
                Action::ContextClick {
                    target: compact_target(&args.target)?,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Fill(args) => {
            perform_action(
                args.session,
                Action::Fill {
                    target: compact_target(&args.target)?,
                    value: args.value,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Type(args) => {
            perform_action(
                args.session,
                Action::Type {
                    target: compact_target(&args.target)?,
                    value: args.value,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::InsertText(args) => {
            perform_action(
                args.session,
                Action::InsertText { value: args.value },
                None,
                args.json,
            )
            .await
        }
        AgentCommand::Check(args) => {
            perform_action(
                args.session,
                Action::Check {
                    target: compact_target(&args.target)?,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Uncheck(args) => {
            perform_action(
                args.session,
                Action::Uncheck {
                    target: compact_target(&args.target)?,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Select(args) => {
            perform_action(
                args.session,
                Action::Select {
                    target: compact_target(&args.target)?,
                    values: args.values,
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Drag(args) => {
            perform_action(
                args.session,
                Action::Drag {
                    source: compact_target(&args.source)?,
                    target: compact_target(&args.target)?,
                },
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
        AgentCommand::Wheel(args) => {
            let target = args.target.as_deref().map(compact_target).transpose()?;
            perform_action(
                args.session,
                Action::Wheel {
                    target,
                    delta_x: args.delta_x,
                    delta_y: args.delta_y,
                    modifiers: args.modifiers.into_iter().map(Into::into).collect(),
                },
                args.observation,
                args.json,
            )
            .await
        }
        AgentCommand::Viewport(args) => {
            perform_action(
                args.session,
                Action::Viewport {
                    width: args.width,
                    height: args.height,
                    scale: args.scale,
                },
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
        AgentCommand::RepairWatch(args) => repair_watch::watch(args).await,
        AgentCommand::RepairClaim(args) => repair::transition(args, RepairStatus::Claimed).await,
        AgentCommand::RepairProgress(args) => {
            repair::transition(args, RepairStatus::Repairing).await
        }
        AgentCommand::RepairReply(args) => repair::transition(args, RepairStatus::NeedsInput).await,
        AgentCommand::RepairComplete(args) => {
            repair::transition(args, RepairStatus::Verifying).await
        }
        AgentCommand::RepairVerify(args) => repair::verify(args).await,
        AgentCommand::RepairFail(args) => repair::transition(args, RepairStatus::Failed).await,
        AgentCommand::RepairCancel(args) => repair::transition(args, RepairStatus::Cancelled).await,
        AgentCommand::Finish(args) => finish(args).await,
        AgentCommand::Abort(args) => abort(args).await,
        AgentCommand::Show(args) => show(args).await,
        AgentCommand::List(args) => list(args).await,
        AgentCommand::Schema(args) => schema::execute(args),
    }
}

struct StartSessionOutput {
    state: AgentSessionState,
    output: StepOutput,
    url: String,
}

async fn start(args: StartArgs) -> Result<ExitCode> {
    let json_output = args.json;
    let result = start_session(args, None).await?;
    emit_start(&result, json_output)?;
    Ok(ExitCode::SUCCESS)
}

async fn start_session(
    args: StartArgs,
    workspace_override: Option<&Path>,
) -> Result<StartSessionOutput> {
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

    let workspace = match workspace_override {
        Some(workspace) => workspace.to_path_buf(),
        None => canonical_workspace().await?,
    };
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
    let allowed_origins = allowed_origins.into_iter().collect::<Vec<_>>();
    let browser_network_policy = browser_network_policy(&allowed_origins, &args.allowed_domains)?;
    let browser_driver = match args.browser_driver {
        BrowserDriverKind::A3s => StoredBrowserDriver::A3s,
        BrowserDriverKind::Standalone => StoredBrowserDriver::Standalone,
    };
    let browser_containment = containment_for_driver(browser_driver);

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
        auto_resolve_repairs: args.auto_resolve_repairs,
        allowed_origins,
        browser_containment: Some(browser_containment),
        browser_allowed_origins: Some(browser_network_policy.allowed_origins().to_vec()),
        browser_allowed_domains: Some(browser_network_policy.allowed_domains().to_vec()),
        browser: StoredBrowserConfig {
            driver: browser_driver,
            executable,
            headed: args.headed,
            command_timeout_ms: args.command_timeout_ms,
            idle_timeout_ms: args.idle_timeout_ms,
            microphone: match args.browser_microphone {
                super::BrowserMicrophoneArg::Disabled => StoredBrowserMicrophone::Disabled,
                super::BrowserMicrophoneArg::Synthetic => StoredBrowserMicrophone::Synthetic,
            },
        },
        namespace: session_namespace(&workspace, &args.session),
        driver_session: driver_session_id(&args.session),
        runtime_dir,
        artifacts_dir: store.artifacts_dir().to_path_buf(),
        active_video_path: None,
        next_sequence: 1,
        next_observation_id: 1,
        latest_observation: None,
        page_context_bindings: None,
        started_at_ms: now,
        updated_at_ms: now,
        summary: None,
    };

    if let Err(error) = store.save(&state).await {
        cleanup_failed_start(&store, &state).await;
        return Err(error.context("failed to publish agent session recovery metadata"));
    }

    let mut browser = match connect(&state, BrowserConnectionPurpose::Turn).await {
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
            if let Err(cleanup_error) = browser.close_surface().await {
                let preservation =
                    preserve_failed_start(&store, &mut state, Some(action), &cleanup_error).await;
                let recovery = match preservation {
                    Ok(()) => format!(
                        "cleanup evidence was preserved; retry with `a3s-test agent abort --session {} --json`",
                        state.session
                    ),
                    Err(preservation_error) => format!(
                        "the owned runtime was retained, but recovery metadata could not be saved: {preservation_error:#}"
                    ),
                };
                anyhow::bail!(
                    "browser start failed: {}; cleanup also failed: {}; {recovery}",
                    error.message(),
                    cleanup_error.message()
                );
            }
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
        if let Err(cleanup_error) = browser.close_surface().await {
            let preservation =
                preserve_failed_start(&store, &mut state, None, &cleanup_error).await;
            let recovery = match preservation {
                Ok(()) => format!(
                    "cleanup evidence was preserved; retry with `a3s-test agent abort --session {} --json`",
                    state.session
                ),
                Err(preservation_error) => format!(
                    "the owned runtime was retained, but recovery metadata could not be saved: {preservation_error:#}"
                ),
            };
            return Err(error.context(format!(
                "browser cleanup also failed: {}; {recovery}",
                cleanup_error.message()
            )));
        }
        cleanup_failed_start(&store, &state).await;
        return Err(error);
    }

    Ok(StartSessionOutput {
        state,
        output,
        url: args.url,
    })
}

fn emit_start(result: &StartSessionOutput, json_output: bool) -> Result<()> {
    emit(
        json_output,
        json!({
            "protocol_revision": ACTION_PROTOCOL_REVISION,
            "session": result.state.session,
            "status": result.state.status,
            "goal": result.state.goal,
            "success_criteria": result.state.success_criteria,
            "auto_resolve_repairs": result.state.auto_resolve_repairs,
            "allowed_origins": result.state.allowed_origins,
            "browser_containment": result.state.browser_containment,
            "browser_allowed_origins": result.state.browser_allowed_origins,
            "browser_allowed_domains": result.state.browser_allowed_domains,
            "output": result.output,
            "artifacts_dir": result.state.artifacts_dir,
            "next": format!("a3s-test agent observe --session {} --interactive --json", result.state.session),
        }),
        format!(
            "Started agent test session '{}' for {}",
            result.state.session, result.url
        ),
    )
}

async fn observe(args: ObserveArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_active(&store, &workspace, &args.session).await?;
    let action = Action::Snapshot {
        interactive: args.interactive,
    };
    if let Err(error) = validate_turn_browser_network_policy(&state) {
        state.latest_observation = None;
        state.page_context_bindings = None;
        record_failure(&store, &mut state, "observe", Some(action), &error).await?;
        return emit_driver_error_with_next(args.json, &state, error, abort_next_command(&state));
    }
    state.latest_observation = None;
    state.page_context_bindings = None;
    store.save(&state).await?;
    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    match browser.observe_surface_interactive(args.interactive).await {
        Ok(mut observation) => {
            let mut output =
                StepOutput::new("page snapshot captured").with_data(observation.data.clone());
            output.evidence.clone_from(&observation.evidence);
            if let Err(error) = validate_observation_origin(&state, &output) {
                state.latest_observation = None;
                state.page_context_bindings = None;
                state.active_video_path = browser.active_video_path().map(str::to_string);
                record_failure(&store, &mut state, "observe", Some(action), &error).await?;
                return emit_driver_error(args.json, &state, error);
            }
            let observation_id = state.next_observation_id;
            state.next_observation_id = state
                .next_observation_id
                .checked_add(1)
                .context("agent observation sequence exhausted")?;
            state.latest_observation = Some(observation_id);
            let bindings = bind_page_context_refs(&mut observation);
            state.page_context_bindings =
                (bindings.revision.is_some() || !bindings.is_empty()).then_some(bindings);
            output.page_context = observation.page_context;
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
                    "protocol_revision": ACTION_PROTOCOL_REVISION,
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
            state.latest_observation = None;
            state.page_context_bindings = None;
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
    if let Err(error) = validate_turn_browser_network_policy(&state) {
        state.latest_observation = None;
        state.page_context_bindings = None;
        record_failure(&store, &mut state, "act", Some(action), &error).await?;
        return emit_driver_error_with_next(json_output, &state, error, abort_next_command(&state));
    }
    validate_action(&state, &action, observation)?;

    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    let expected_revision = if action_uses_page_context_ref(&action) {
        Some(
            state
                .page_context_bindings
                .as_ref()
                .and_then(|bindings| bindings.revision)
                .context("page context ref is missing its latest observation revision")?,
        )
    } else {
        None
    };
    state.latest_observation = None;
    let mut bindings = state.page_context_bindings.take().unwrap_or_default();
    store.save(&state).await?;
    if let Some(revision) = expected_revision {
        match browser.context_delta(revision).await {
            Ok(Some(context)) => {
                if let Err(refresh_error) = refresh_page_context_bindings(&mut bindings, &context) {
                    let error = DriverError::new(
                        "test.driver.web.page_context_diff_invalid",
                        refresh_error.to_string(),
                    );
                    record_failure(&store, &mut state, "act", Some(action), &error).await?;
                    return emit_driver_error(json_output, &state, error);
                }
            }
            Ok(None) => {}
            Err(error) => {
                record_failure(&store, &mut state, "act", Some(action), &error).await?;
                return emit_driver_error(json_output, &state, error);
            }
        }
    }
    let action = match resolve_page_context_refs(action.clone(), &bindings) {
        Ok(action) => action,
        Err(resolve_error) => {
            let error = DriverError::new(
                "test.driver.web.page_context_stale",
                resolve_error.to_string(),
            );
            record_failure(&store, &mut state, "act", Some(action), &error).await?;
            return emit_driver_error(json_output, &state, error);
        }
    };
    match browser
        .execute_action(format!("agent-act-{}", state.next_sequence), action.clone())
        .await
    {
        Ok(output) => {
            state.latest_observation = None;
            state.page_context_bindings = None;
            state.active_video_path = browser.active_video_path().map(str::to_string);
            append_success_event(&store, &mut state, "act", None, action, output.clone()).await?;
            store.save(&state).await?;
            emit(
                json_output,
                json!({
                    "protocol_revision": ACTION_PROTOCOL_REVISION,
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
            state.page_context_bindings = None;
            state.active_video_path = browser.active_video_path().map(str::to_string);
            record_failure(&store, &mut state, "act", Some(action), &error).await?;
            emit_driver_error(json_output, &state, error)
        }
    }
}

async fn finish(args: FinishArgs) -> Result<ExitCode> {
    if args.summary.trim().is_empty() {
        anyhow::bail!("agent test summary must not be empty");
    }
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_active(&store, &workspace, &args.session).await?;
    repair::interrupt_for_session_close(&store, &state).await?;
    let mut browser = connect(&state, BrowserConnectionPurpose::Cleanup).await?;
    let cleanup_error = close_and_remove_runtime(&mut browser, &state).await;
    state.active_video_path = None;
    state.status = match args.status {
        FinishStatus::Passed => AgentSessionStatus::Passed,
        FinishStatus::Failed => AgentSessionStatus::Failed,
    };
    state.summary = Some(args.summary.clone());
    state.latest_observation = None;
    state.page_context_bindings = None;
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
        auto_resolve_repairs: state.auto_resolve_repairs,
        allowed_origins: state.allowed_origins.clone(),
        browser_containment: state.browser_containment,
        browser_allowed_origins: state.browser_allowed_origins.clone().unwrap_or_default(),
        browser_allowed_domains: state.browser_allowed_domains.clone().unwrap_or_default(),
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
            "protocol_revision": ACTION_PROTOCOL_REVISION,
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

struct AbortSessionOutput {
    state: AgentSessionState,
    cleanup_error: Option<DriverError>,
}

async fn abort(args: SessionArgs) -> Result<ExitCode> {
    let result = abort_session(&args.session, None).await?;
    emit_abort(&result, args.json)?;
    Ok(if result.cleanup_error.is_some() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

async fn abort_session(
    session: &str,
    workspace_override: Option<&Path>,
) -> Result<AbortSessionOutput> {
    let workspace = match workspace_override {
        Some(workspace) => workspace.to_path_buf(),
        None => canonical_workspace().await?,
    };
    let store = load_store(&workspace, session)?;
    let mut state = load_session_state(&store, &workspace, session).await?;
    repair::interrupt_for_session_close(&store, &state).await?;
    let cleanup_error = if state.runtime_dir.exists() {
        let mut browser = connect(&state, BrowserConnectionPurpose::Cleanup).await?;
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
        state.page_context_bindings = None;
        store.save(&state).await?;
    } else if let Some(error) = &cleanup_error {
        state.summary = Some("Agent session abort cleanup failed".to_string());
        record_failure(&store, &mut state, "abort", None, error).await?;
    }
    Ok(AbortSessionOutput {
        state,
        cleanup_error,
    })
}

fn emit_abort(result: &AbortSessionOutput, json_output: bool) -> Result<()> {
    emit(
        json_output,
        json!({
            "protocol_revision": ACTION_PROTOCOL_REVISION,
            "session": result.state.session,
            "status": result.state.status,
            "cleanup_error": result.cleanup_error.as_ref().map(|error| AgentSessionError {
                code: error.code().to_string(),
                message: error.message().to_string(),
            }),
        }),
        format!(
            "Agent test session '{}' is {:?}",
            result.state.session, result.state.status
        ),
    )
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

fn emit_driver_error(
    json_output: bool,
    state: &AgentSessionState,
    error: a3s_test_core::DriverError,
) -> Result<ExitCode> {
    emit_driver_error_with_next(
        json_output,
        state,
        error,
        format!(
            "a3s-test agent observe --session {} --interactive --json",
            state.session
        ),
    )
}

fn emit_driver_error_with_next(
    json_output: bool,
    state: &AgentSessionState,
    error: a3s_test_core::DriverError,
    next: String,
) -> Result<ExitCode> {
    emit(
        json_output,
        json!({
            "protocol_revision": ACTION_PROTOCOL_REVISION,
            "session": state.session,
            "status": state.status,
            "error": AgentSessionError {
                code: error.code().to_string(),
                message: error.message().to_string(),
            },
            "next": next,
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
        || state.driver_session != driver_session_id(session)
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

fn abort_next_command(state: &AgentSessionState) -> String {
    format!("a3s-test agent abort --session {} --json", state.session)
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

async fn preserve_failed_start(
    store: &AgentSessionStore,
    state: &mut AgentSessionState,
    action: Option<Action>,
    cleanup_error: &DriverError,
) -> Result<()> {
    state.status = AgentSessionStatus::Failed;
    state.latest_observation = None;
    state.page_context_bindings = None;
    state.summary = Some(format!(
        "Agent session start failed and browser cleanup must be retried: {}",
        cleanup_error.message()
    ));
    record_failure(store, state, "start_cleanup", action, cleanup_error).await
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
