use std::process::ExitCode;
use std::sync::Arc;

use a3s_test_agent::{
    GroundingPageContext, GroundingRequest, GroundingResult, GroundingTrigger, HttpProviderConfig,
    HttpVisualGroundingProvider, SemanticFallbackReason, VisualGroundingService,
};
use a3s_test_core::{
    bind_page_context_refs, Action, DriverError, DriverSession, PageContextBindings, StepOutput,
    SurfaceObservation,
};
use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::args::{GroundArgs, GroundingReason};
use super::events::{append_success_event, record_failure};
use super::{
    abort_next_command, canonical_workspace, connect, emit, emit_driver_error,
    emit_driver_error_with_next, load_active, load_store, validate_turn_browser_network_policy,
    BrowserConnectionPurpose,
};

mod config;

const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_HTTP_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
struct GroundingEvent<'a> {
    query: &'a str,
    trigger: GroundingTrigger,
    result: &'a GroundingResult,
}

pub(super) async fn execute(args: GroundArgs) -> Result<ExitCode> {
    let admitted = admit_provider(&args).await?;
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let mut state = load_active(&store, &workspace, &args.session).await?;
    if let Err(error) = validate_turn_browser_network_policy(&state) {
        state.latest_observation = None;
        state.page_context_bindings = None;
        record_failure(&store, &mut state, "ground", None, &error).await?;
        return emit_driver_error_with_next(args.json, &state, error, abort_next_command(&state));
    }
    match state.latest_observation {
        Some(observation) if observation == args.observation => {}
        Some(observation) => anyhow::bail!(
            "visual grounding requires the latest observation {observation}; pass `--observation {observation}`"
        ),
        None => anyhow::bail!(
            "visual grounding requires a current observation; run `a3s-test agent observe --session {} --interactive --json` first",
            state.session
        ),
    }

    let trigger = grounding_trigger(args.reason);
    let expected_revision = state
        .page_context_bindings
        .as_ref()
        .and_then(|bindings| bindings.revision);
    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    let requested_path = format!(
        "grounding/observation-{}-{}.png",
        args.observation, state.next_sequence
    );
    let screenshot = match browser
        .capture_grounding_screenshot(&requested_path, expected_revision)
        .await
    {
        Ok(screenshot) => screenshot,
        Err(error) => {
            invalidate_observation(&mut state);
            record_failure(&store, &mut state, "ground", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    };

    let current_context = match browser
        .inspect_context(&a3s_test_core::PageContextInspectRequest {
            detail: "summary".to_string(),
            scope: a3s_test_core::PageContextInspectScope::Page,
            since_revision: None,
            wait_timeout_ms: 0,
            cursor: None,
            limit: 500,
        })
        .await
    {
        Ok(context) if context.present => context,
        Ok(_) => a3s_test_core::PageContextObservation::absent(),
        Err(error) => {
            invalidate_observation(&mut state);
            record_failure(&store, &mut state, "ground", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    };
    if current_context.revision != screenshot.surface_revision
        || expected_revision != screenshot.surface_revision
    {
        let error = DriverError::new(
            "test.driver.web.page_context_changed",
            "page context changed after grounding evidence capture",
        );
        invalidate_observation(&mut state);
        record_failure(&store, &mut state, "ground", None, &error).await?;
        return emit_driver_error(args.json, &state, error);
    }

    let current_context =
        match bind_current_page_context(current_context, state.page_context_bindings.as_ref()) {
            Ok(value) => value,
            Err(error) => {
                invalidate_observation(&mut state);
                record_failure(&store, &mut state, "ground", None, &error).await?;
                return emit_driver_error(args.json, &state, error);
            }
        };
    let request = GroundingRequest {
        screenshot_path: screenshot.evidence.path.clone(),
        screenshot_sha256: screenshot.sha256.clone(),
        width: screenshot.width,
        height: screenshot.height,
        query: args.query.clone(),
        observation_id: args.observation,
        trigger,
        max_cost_microusd: admitted.max_cost_microusd,
    };
    let snapshot = current_context.snapshot.as_ref();
    let page_context =
        snapshot
            .zip(screenshot.surface_revision)
            .map(|(snapshot, surface_revision)| GroundingPageContext {
                observation_id: args.observation,
                surface_revision,
                snapshot,
            });
    let cancellation = CancellationToken::new();
    let signal_task = install_interrupt_handler(cancellation.clone());
    let grounded = admitted
        .service
        .ground(request, page_context, cancellation)
        .await;
    signal_task.abort();
    let _ = signal_task.await;
    let result = match grounded {
        Ok(result) => result,
        Err(error) => {
            invalidate_observation(&mut state);
            let error = DriverError::new(
                error.code(),
                admitted.redactor.redacted_text(error.message()),
            )
            .with_retryable(error.retryable());
            record_failure(&store, &mut state, "ground", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    };
    if let Some(revision) = screenshot.surface_revision {
        if let Err(error) = browser.validate_context_revision(revision).await {
            invalidate_observation(&mut state);
            record_failure(&store, &mut state, "ground", None, &error).await?;
            return emit_driver_error(args.json, &state, error);
        }
    }
    let event = GroundingEvent {
        query: &args.query,
        trigger,
        result: &result,
    };
    let mut event_value = serde_json::to_value(event)?;
    admitted.redactor.redact_json(&mut event_value);
    let output = StepOutput::new("advisory visual grounding completed")
        .with_data(event_value)
        .with_evidence(screenshot.evidence.clone())
        .with_page_context(current_context);
    append_success_event(
        &store,
        &mut state,
        "ground",
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
            "protocol": a3s_test_agent::VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "authority": "advisory",
            "session": state.session,
            "observation_id": args.observation,
            "output": output,
            "next": format!(
                "If exactly one semantic match was returned, use its target with `a3s-test agent act --session {} --observation {} ...`; image-bound candidates require a fresh observation or human decision",
                state.session, args.observation
            ),
        }),
        format!(
            "Advisory visual grounding completed for '{}'",
            state.session
        ),
    )?;
    Ok(ExitCode::SUCCESS)
}

struct AdmittedProvider {
    service: VisualGroundingService,
    redactor: a3s_test_agent::ProvenanceRedactor,
    max_cost_microusd: u64,
}

async fn admit_provider(args: &GroundArgs) -> Result<AdmittedProvider> {
    let config = config::read(&args.config).await?;
    validate_query(&args.query, config.options.max_query_bytes)?;
    let authorization = config.read_authorization()?;
    let redactor = config.redactor(authorization.as_deref())?;
    let mut transport = HttpProviderConfig::new(config.endpoint.clone())
        .with_timeout(config.options.timeout)?
        .with_body_limits(MAX_HTTP_REQUEST_BYTES, MAX_HTTP_RESPONSE_BYTES)?;
    if let Some(value) = authorization {
        transport = transport.with_authorization(value)?;
    }
    let provider = Arc::new(HttpVisualGroundingProvider::new(
        config.identity.clone(),
        transport,
    )?);
    let service = VisualGroundingService::new(provider, config.options)?;
    Ok(AdmittedProvider {
        service,
        redactor,
        max_cost_microusd: config.max_cost_microusd,
    })
}

fn bind_current_page_context(
    page_context: a3s_test_core::PageContextObservation,
    expected: Option<&PageContextBindings>,
) -> Result<a3s_test_core::PageContextObservation, DriverError> {
    if !page_context.present {
        if expected.is_some() {
            return Err(DriverError::new(
                "test.driver.web.page_context_changed",
                "the current page no longer exposes the observed page context",
            ));
        }
        return Ok(page_context);
    }
    let mut observation =
        SurfaceObservation::new("current grounding page context").with_page_context(page_context);
    let bindings = bind_page_context_refs(&mut observation);
    if expected != Some(&bindings) {
        return Err(DriverError::new(
            "test.driver.web.page_context_changed",
            "current page-context refs no longer match the latest observation",
        ));
    }
    let page_context = observation
        .page_context
        .expect("present page context remains attached");
    Ok(page_context)
}

fn validate_query(query: &str, max_query_bytes: usize) -> Result<()> {
    if query.trim().is_empty() || query.len() > max_query_bytes {
        anyhow::bail!(
            "grounding query must contain 1 to {max_query_bytes} bytes under the admitted ACL"
        );
    }
    Ok(())
}

fn invalidate_observation(state: &mut super::store::AgentSessionState) {
    state.latest_observation = None;
    state.page_context_bindings = None;
}

fn grounding_trigger(reason: GroundingReason) -> GroundingTrigger {
    match reason {
        GroundingReason::Explicit => GroundingTrigger::ExplicitRequest,
        GroundingReason::Canvas => GroundingTrigger::SemanticFallback {
            reason: SemanticFallbackReason::Canvas,
        },
        GroundingReason::ImageOnly => GroundingTrigger::SemanticFallback {
            reason: SemanticFallbackReason::ImageOnly,
        },
        GroundingReason::RemoteDesktop => GroundingTrigger::SemanticFallback {
            reason: SemanticFallbackReason::RemoteDesktop,
        },
        GroundingReason::DesignReference => GroundingTrigger::SemanticFallback {
            reason: SemanticFallbackReason::DesignReference,
        },
        GroundingReason::NoSemanticMatch => GroundingTrigger::SemanticFallback {
            reason: SemanticFallbackReason::NoSemanticMatch,
        },
    }
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
    use super::*;
    use a3s_test_core::{
        PageContextGeometry, PageContextLocator, PageContextNode, PageContextNodeState,
        PageContextObservation, PageContextPosition, PageContextRect, PageContextSnapshot,
    };
    #[test]
    fn binds_current_context_to_the_exact_observation_refs() {
        let context = current_context("private-pay", "pay", 42);
        let expected = bindings_for_context(current_context("private-pay", "pay", 42));

        let bound = bind_current_page_context(context, Some(&expected)).expect("bound context");
        let node = &bound.snapshot.expect("snapshot").nodes[0];
        assert_eq!(node.r#ref.as_deref(), Some("@c1"));
        assert!(node.id.is_empty());
    }

    #[test]
    fn rejects_changed_or_unexpected_current_context_bindings() {
        let expected = bindings_for_context(current_context("private-pay", "pay", 42));
        for (context, bindings) in [
            (
                current_context("private-pay", "changed", 42),
                Some(&expected),
            ),
            (current_context("private-pay", "pay", 42), None),
        ] {
            let error = bind_current_page_context(context, bindings)
                .expect_err("context binding drift must fail closed");
            assert_eq!(error.code(), "test.driver.web.page_context_changed");
        }
    }

    #[tokio::test]
    async fn rejects_invalid_provider_admission_without_loading_a_session() {
        let directory = tempfile::tempdir().expect("grounding config directory");
        let path = directory.path().join("invalid.acl");
        std::fs::write(&path, "visual_grounding { max_cost_microusd = 1 }")
            .expect("invalid grounding config");
        let error = execute(GroundArgs {
            query: "Pay".to_string(),
            session: "missing-session".to_string(),
            observation: 1,
            config: path,
            reason: GroundingReason::Explicit,
            json: false,
        })
        .await
        .expect_err("provider admission must fail first");

        assert!(
            error
                .to_string()
                .contains("visual_grounding requires exactly one provider block"),
            "{error:#}"
        );
        assert!(!error.to_string().contains("agent session"), "{error:#}");
    }

    fn current_context(node_id: &str, test_id: &str, revision: u64) -> PageContextObservation {
        let rect = PageContextRect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 40.0,
        };
        PageContextObservation::from_snapshot(PageContextSnapshot {
            protocol: Some("a3s.test.page-context/1".to_string()),
            sdk_version: Some("0.2.0".to_string()),
            revision: Some(revision),
            page: None,
            components: Vec::new(),
            nodes: vec![PageContextNode {
                id: node_id.to_string(),
                r#ref: None,
                parent_id: None,
                component_id: None,
                tag: "button".to_string(),
                role: Some("button".to_string()),
                name: Some("Pay".to_string()),
                text: Some("Pay".to_string()),
                description: None,
                test_id: Some(test_id.to_string()),
                geometry: Some(PageContextGeometry {
                    viewport: rect.clone(),
                    document: rect.clone(),
                    normalized: rect,
                    visible_ratio: 1.0,
                    occluded: false,
                    position: PageContextPosition::Static,
                    transformed: false,
                    scroll_container_node_id: None,
                }),
                state: PageContextNodeState {
                    visible: true,
                    disabled: Some(false),
                    checked: None,
                    selected: None,
                    expanded: None,
                    focused: None,
                    readonly: None,
                    required: None,
                    invalid: None,
                },
                locators: vec![PageContextLocator::TestId {
                    value: test_id.to_string(),
                }],
                classes: None,
                attributes: None,
                computed_styles: None,
                source_mapping: None,
            }],
            facts: serde_json::Map::new(),
            ui: None,
            delta: None,
            removed_node_ids: Vec::new(),
            truncated: false,
            next_cursor: None,
        })
    }

    fn bindings_for_context(context: PageContextObservation) -> PageContextBindings {
        let mut observation =
            SurfaceObservation::new("observed page context").with_page_context(context);
        bind_page_context_refs(&mut observation)
    }
}
