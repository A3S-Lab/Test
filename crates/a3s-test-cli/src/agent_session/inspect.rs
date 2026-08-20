use std::process::ExitCode;

use a3s_test_core::{
    Action, DriverError, PageContextInspectRequest, PageContextInspectScope, StepOutput,
    SurfaceObservation, ACTION_PROTOCOL_REVISION,
};
use a3s_test_session::bind_page_context_refs;
use anyhow::{Context, Result};
use serde_json::json;

use super::args::InspectArgs;
use super::events::{append_success_event, record_failure};
use super::{
    abort_next_command, canonical_workspace, connect, emit, emit_driver_error,
    emit_driver_error_with_next, load_active, load_store, validate_turn_browser_network_policy,
    BrowserConnectionPurpose,
};

pub(super) async fn execute(args: InspectArgs) -> Result<ExitCode> {
    if !(1..=500).contains(&args.limit) {
        anyhow::bail!("inspect limit must be between 1 and 500");
    }
    if args.detail == "diff" {
        if args.since_revision.is_none_or(|revision| revision == 0) {
            anyhow::bail!("diff inspection requires --since-revision with a positive revision");
        }
    } else if args.since_revision.is_some() || args.wait_timeout_ms != 0 {
        anyhow::bail!("--since-revision and --wait-timeout-ms require --detail diff");
    }
    if args.wait_timeout_ms > 300_000 {
        anyhow::bail!("diff wait timeout must not exceed 300000 milliseconds");
    }
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_active(&store, &workspace, &args.session).await?;
    if let Err(error) = validate_turn_browser_network_policy(&state) {
        state.latest_observation = None;
        state.page_context_bindings = None;
        record_failure(&store, &mut state, "inspect", None, &error).await?;
        return emit_driver_error_with_next(args.json, &state, error, abort_next_command(&state));
    }
    let scope = if let Some(node) = args.node {
        PageContextInspectScope::Node(node)
    } else if let Some(component) = args.component {
        PageContextInspectScope::Component(component)
    } else if let Some(region) = args.region {
        parse_region(&region)?
    } else {
        PageContextInspectScope::Page
    };
    state.latest_observation = None;
    state.page_context_bindings = None;
    store.save(&state).await?;
    let request = PageContextInspectRequest {
        detail: args.detail,
        scope,
        since_revision: args.since_revision,
        wait_timeout_ms: args.wait_timeout_ms,
        cursor: args.cursor,
        limit: args.limit,
    };
    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    match browser.inspect_context(&request).await {
        Ok(page_context) => {
            if !page_context.present {
                let error = DriverError::new(
                    "test.driver.web.page_context_missing",
                    "the page does not expose a compatible A3S Test Kit context bridge",
                );
                record_failure(&store, &mut state, "inspect", None, &error).await?;
                return emit_driver_error(args.json, &state, error);
            }
            let mut observation = SurfaceObservation::new("scoped page context inspected")
                .with_page_context(page_context);
            let observation_id = state.next_observation_id;
            state.next_observation_id = state
                .next_observation_id
                .checked_add(1)
                .context("agent observation sequence exhausted")?;
            let bindings = bind_page_context_refs(&mut observation);
            state.page_context_bindings =
                (bindings.revision.is_some() || !bindings.is_empty()).then_some(bindings);
            state.latest_observation = Some(observation_id);
            let output = StepOutput::new("scoped page context inspected")
                .with_page_context(observation.page_context.expect("page context present"));
            append_success_event(
                &store,
                &mut state,
                "inspect",
                Some(observation_id),
                Action::Snapshot { interactive: false },
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
                format!("Scoped context inspected for '{}'", state.session),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            record_failure(&store, &mut state, "inspect", None, &error).await?;
            emit_driver_error(args.json, &state, error)
        }
    }
}

fn parse_region(value: &str) -> Result<PageContextInspectScope> {
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 5 || !matches!(parts[0], "viewport" | "document") {
        anyhow::bail!(
            "inspect region must be space,x,y,width,height with viewport or document space"
        );
    }
    Ok(PageContextInspectScope::Region {
        space: parts[0].to_string(),
        x: parts[1].parse().context("inspect region x is invalid")?,
        y: parts[2].parse().context("inspect region y is invalid")?,
        width: parts[3]
            .parse()
            .context("inspect region width is invalid")?,
        height: parts[4]
            .parse()
            .context("inspect region height is invalid")?,
    })
}
