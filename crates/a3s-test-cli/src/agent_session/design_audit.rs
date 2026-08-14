use std::collections::HashSet;
use std::process::ExitCode;
use std::sync::Arc;

use a3s_test_agent::{
    DesignAuditDimension, DesignAuditReport, DesignAuditRequest, DesignAuditService,
    HttpDesignAuditProvider, HttpProviderConfig,
};
use a3s_test_core::{
    bind_page_context_refs, Action, DriverError, DriverSession, PageContextBindings,
    PageContextObservation, StepOutput, SurfaceObservation,
};
use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::args::AuditArgs;
use super::events::{append_success_event, record_failure};
use super::{
    abort_next_command, canonical_workspace, connect, emit, emit_driver_error,
    emit_driver_error_with_next, load_active, load_store, validate_turn_browser_network_policy,
    BrowserConnectionPurpose,
};

mod config;

const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1_024 * 1_024;
const MAX_HTTP_RESPONSE_BYTES: usize = 8 * 1_024 * 1_024;
const MAX_PAGE_CONTEXT_NODES: usize = 5_000;

#[derive(Serialize)]
struct DesignAuditEvent<'a> {
    report: &'a DesignAuditReport,
    projected_to_review: bool,
}

pub(super) async fn execute(args: AuditArgs) -> Result<ExitCode> {
    let admitted = admit_provider(&args).await?;
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_active(&store, &workspace, &args.session).await?;
    if let Err(error) = validate_turn_browser_network_policy(&state) {
        invalidate_observation(&mut state);
        record_failure(&store, &mut state, "audit", None, &error).await?;
        return emit_driver_error_with_next(args.json, &state, error, abort_next_command(&state));
    }
    match state.latest_observation {
        Some(observation) if observation == args.observation => {}
        Some(observation) => anyhow::bail!(
            "design audit requires the latest observation {observation}; pass `--observation {observation}`"
        ),
        None => anyhow::bail!(
            "design audit requires a current observation; run `a3s-test agent observe --session {} --json` first",
            state.session
        ),
    }
    let Some(expected_revision) = state
        .page_context_bindings
        .as_ref()
        .and_then(|bindings| bindings.revision)
    else {
        anyhow::bail!(
            "design audit requires a complete embedded Test Kit page context in the latest observation"
        );
    };

    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    let requested_path = format!(
        "design-audit/observation-{}-{}.png",
        args.observation, state.next_sequence
    );
    let screenshot = match browser
        .capture_grounding_screenshot(&requested_path, Some(expected_revision))
        .await
    {
        Ok(screenshot) => screenshot,
        Err(error) => {
            invalidate_observation(&mut state);
            record_failure(&store, &mut state, "audit", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    };

    let current_context = match inspect_complete_page_context(&mut browser, expected_revision).await
    {
        Ok(context) => context,
        Err(error) => {
            invalidate_observation(&mut state);
            record_failure(&store, &mut state, "audit", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    };
    if current_context.revision != Some(expected_revision)
        || screenshot.surface_revision != Some(expected_revision)
    {
        let error = DriverError::new(
            "test.driver.web.page_context_changed",
            "page context changed after design-audit evidence capture",
        );
        invalidate_observation(&mut state);
        record_failure(&store, &mut state, "audit", None, &error).await?;
        return emit_driver_error(args.json, &state, error);
    }
    if let Err(error) =
        validate_current_page_context(&current_context, state.page_context_bindings.as_ref())
    {
        invalidate_observation(&mut state);
        record_failure(&store, &mut state, "audit", None, &error).await?;
        return emit_driver_error(args.json, &state, error);
    }
    let Some(snapshot) = current_context.snapshot.clone() else {
        let error = DriverError::new(
            "test.driver.web.page_context_required",
            "design audit requires a typed page-context snapshot",
        );
        invalidate_observation(&mut state);
        record_failure(&store, &mut state, "audit", None, &error).await?;
        return emit_driver_error(args.json, &state, error);
    };
    let request = DesignAuditRequest {
        screenshot_path: screenshot.evidence.path.clone(),
        screenshot_sha256: screenshot.sha256.clone(),
        width: screenshot.width,
        height: screenshot.height,
        observation_id: args.observation,
        surface_revision: expected_revision,
        page_context: snapshot,
        dimensions: admitted.dimensions.clone(),
        max_cost_microusd: admitted.max_cost_microusd,
    };
    let cancellation = CancellationToken::new();
    let signal_task = install_interrupt_handler(cancellation.clone());
    let audited = admitted.service.audit(request, cancellation).await;
    signal_task.abort();
    let _ = signal_task.await;
    let report = match audited {
        Ok(report) => report,
        Err(error) => {
            invalidate_observation(&mut state);
            let error = DriverError::new(
                error.code(),
                admitted.redactor.redacted_text(error.message()),
            )
            .with_retryable(error.retryable());
            record_failure(&store, &mut state, "audit", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    };
    if let Err(error) = browser.validate_context_revision(expected_revision).await {
        invalidate_observation(&mut state);
        record_failure(&store, &mut state, "audit", None, &error).await?;
        return emit_driver_error(args.json, &state, error);
    }
    let report = redact_report(&admitted.redactor, report)?;
    let projected_to_review = match browser.project_design_audit_report(&report).await {
        Ok(projected) => projected,
        Err(error) => {
            invalidate_observation(&mut state);
            record_failure(&store, &mut state, "audit", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    };
    let event = DesignAuditEvent {
        report: &report,
        projected_to_review,
    };
    let output = StepOutput::new("advisory design audit completed")
        .with_data(serde_json::to_value(event)?)
        .with_evidence(screenshot.evidence.clone())
        .with_page_context(current_context);
    append_success_event(
        &store,
        &mut state,
        "audit",
        Some(args.observation),
        Action::Screenshot {
            path: requested_path,
        },
        output.clone(),
    )
    .await?;
    store.save(&state).await?;
    emit(
        args.json,
        json!({
            "protocol": a3s_test_agent::DESIGN_AUDIT_PROVIDER_PROTOCOL,
            "report_protocol": a3s_test_agent::DESIGN_AUDIT_REPORT_PROTOCOL,
            "authority": "advisory",
            "session": state.session,
            "observation_id": args.observation,
            "projected_to_review": projected_to_review,
            "output": output,
            "next": if projected_to_review {
                "A human may review, edit, dismiss, or explicitly send individual or batched suggestions from the embedded review UI"
            } else {
                "The admitted report is retained as evidence; embed a compatible Test Kit to promote suggestions through human review"
            },
        }),
        format!("Advisory design audit completed for '{}'", state.session),
    )?;
    Ok(ExitCode::SUCCESS)
}

async fn inspect_complete_page_context(
    browser: &mut a3s_test_driver_web::AgentBrowserSession,
    expected_revision: u64,
) -> Result<PageContextObservation, DriverError> {
    const MAX_CURSOR_BYTES: usize = 4 * 1_024;
    const MAX_PAGES: usize = 64;

    let mut cursor = None;
    let mut seen_cursors = HashSet::new();
    let mut merged: Option<a3s_test_core::PageContextSnapshot> = None;
    for _ in 0..MAX_PAGES {
        let observation = browser
            .inspect_context(&a3s_test_core::PageContextInspectRequest {
                detail: "forensic".to_string(),
                scope: a3s_test_core::PageContextInspectScope::Page,
                cursor: cursor.clone(),
                limit: MAX_PAGE_CONTEXT_NODES,
            })
            .await?;
        if !observation.present || observation.revision != Some(expected_revision) {
            return Err(DriverError::new(
                "test.driver.web.page_context_changed",
                "design-audit page context is absent or changed while collecting its bounded pages",
            ));
        }
        let snapshot = observation.snapshot.ok_or_else(|| {
            DriverError::new(
                "test.driver.web.page_context_invalid",
                "design-audit page context did not include a typed snapshot",
            )
        })?;
        if snapshot.revision != Some(expected_revision) {
            return Err(DriverError::new(
                "test.driver.web.page_context_changed",
                "design-audit page-context page has a stale surface revision",
            ));
        }
        match &mut merged {
            Some(current) => {
                if !same_context_metadata(current, &snapshot) {
                    return Err(DriverError::new(
                        "test.driver.web.page_context_changed",
                        "design-audit page-context metadata changed between bounded pages",
                    ));
                }
                current.nodes.extend(snapshot.nodes);
                if current.nodes.len() > MAX_PAGE_CONTEXT_NODES {
                    return Err(DriverError::new(
                        "test.driver.web.page_context_unbounded",
                        format!("design-audit page context exceeds {MAX_PAGE_CONTEXT_NODES} nodes"),
                    ));
                }
            }
            None => merged = Some(snapshot.clone()),
        }
        if !snapshot.truncated && snapshot.next_cursor.is_none() {
            let mut snapshot = merged.expect("at least one page was collected");
            snapshot.truncated = false;
            snapshot.next_cursor = None;
            return Ok(PageContextObservation::from_snapshot(snapshot));
        }
        let next_cursor = snapshot.next_cursor.ok_or_else(|| {
            DriverError::new(
                "test.driver.web.page_context_invalid",
                "truncated design-audit page context omitted its continuation cursor",
            )
        })?;
        if next_cursor.is_empty()
            || next_cursor.len() > MAX_CURSOR_BYTES
            || !seen_cursors.insert(next_cursor.clone())
        {
            return Err(DriverError::new(
                "test.driver.web.page_context_invalid",
                "design-audit page-context cursor is empty, oversized, or repeated",
            ));
        }
        cursor = Some(next_cursor);
    }
    Err(DriverError::new(
        "test.driver.web.page_context_unbounded",
        "design-audit page context exceeded the bounded pagination limit",
    ))
}

fn same_context_metadata(
    left: &a3s_test_core::PageContextSnapshot,
    right: &a3s_test_core::PageContextSnapshot,
) -> bool {
    left.protocol == right.protocol
        && left.sdk_version == right.sdk_version
        && left.revision == right.revision
        && left.page == right.page
        && left.components == right.components
        && left.facts == right.facts
        && left.removed_node_ids == right.removed_node_ids
}

struct AdmittedProvider {
    service: DesignAuditService,
    redactor: a3s_test_agent::ProvenanceRedactor,
    dimensions: Vec<DesignAuditDimension>,
    max_cost_microusd: u64,
}

async fn admit_provider(args: &AuditArgs) -> Result<AdmittedProvider> {
    let config = config::read(&args.config).await?;
    let dimensions = if args.dimension.is_empty() {
        DesignAuditDimension::ALL.to_vec()
    } else {
        args.dimension.iter().copied().map(Into::into).collect()
    };
    if dimensions.iter().copied().collect::<HashSet<_>>().len() != dimensions.len() {
        anyhow::bail!("design-audit dimensions must be unique");
    }
    let authorization = config.read_authorization()?;
    let redactor = config.redactor(authorization.as_deref())?;
    let mut transport = HttpProviderConfig::new(config.endpoint.clone())
        .with_timeout(config.options.timeout)?
        .with_body_limits(MAX_HTTP_REQUEST_BYTES, MAX_HTTP_RESPONSE_BYTES)?;
    if let Some(value) = authorization {
        transport = transport.with_authorization(value)?;
    }
    let provider = Arc::new(HttpDesignAuditProvider::new(
        config.identity.clone(),
        transport,
    )?);
    let service = DesignAuditService::new(provider, config.options)?;
    Ok(AdmittedProvider {
        service,
        redactor,
        dimensions,
        max_cost_microusd: config.max_cost_microusd,
    })
}

fn validate_current_page_context(
    page_context: &PageContextObservation,
    expected: Option<&PageContextBindings>,
) -> Result<(), DriverError> {
    let mut observation = SurfaceObservation::new("current design-audit page context")
        .with_page_context(page_context.clone());
    let bindings = bind_page_context_refs(&mut observation);
    if expected != Some(&bindings) {
        return Err(DriverError::new(
            "test.driver.web.page_context_changed",
            "current page-context refs no longer match the latest observation",
        ));
    }
    Ok(())
}

fn redact_report(
    redactor: &a3s_test_agent::ProvenanceRedactor,
    report: DesignAuditReport,
) -> Result<DesignAuditReport> {
    let mut value = serde_json::to_value(report)?;
    redactor.redact_json(&mut value);
    Ok(serde_json::from_value(value)?)
}

fn invalidate_observation(state: &mut super::store::AgentSessionState) {
    state.latest_observation = None;
    state.page_context_bindings = None;
}

fn install_interrupt_handler(cancellation: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancellation.cancel();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::args::DesignAuditDimensionArg;
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn rejects_invalid_provider_admission_without_loading_a_session() {
        let directory = tempfile::tempdir().expect("design-audit config directory");
        let path = directory.path().join("invalid.acl");
        std::fs::write(&path, "design_audit { max_cost_microusd = 1 }")
            .expect("invalid design-audit config");
        let error = execute(AuditArgs {
            session: "missing-session".to_string(),
            observation: 1,
            config: path,
            dimension: vec![DesignAuditDimensionArg::VisualHierarchy],
            json: false,
        })
        .await
        .expect_err("provider admission must fail first");

        assert!(
            error
                .to_string()
                .contains("design_audit requires exactly one provider block"),
            "{error:#}"
        );
        assert!(!error.to_string().contains("agent session"), "{error:#}");
    }

    #[test]
    fn rejects_duplicate_dimension_flags() {
        let args = AuditArgs {
            session: "missing-session".to_string(),
            observation: 1,
            config: PathBuf::from("missing.acl"),
            dimension: vec![
                DesignAuditDimensionArg::VisualHierarchy,
                DesignAuditDimensionArg::VisualHierarchy,
            ],
            json: false,
        };
        let dimensions = args
            .dimension
            .iter()
            .copied()
            .map(DesignAuditDimension::from)
            .collect::<Vec<_>>();
        assert_ne!(
            dimensions.iter().copied().collect::<HashSet<_>>().len(),
            dimensions.len()
        );
    }
}
