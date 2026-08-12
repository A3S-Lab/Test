use std::process::ExitCode;

use a3s_test_core::{
    RepairActor, RepairEvidencePhase, RepairEvidenceRequest, RepairStatus, ACTION_PROTOCOL_REVISION,
};
use a3s_test_session::{
    build_repair_verification, validate_repair_verification_request, RepairLedger,
    RepairTransition, RepairVerifyRequest,
};
use anyhow::{Context, Result};
use serde_json::json;

use super::args::{RepairTransitionArgs, RepairVerifyArgs, RepairWatchArgs};
use super::{
    canonical_workspace, connect, emit, load_active, load_store, unix_ms, validate_timeout,
    BrowserConnectionPurpose,
};

pub(super) async fn watch(args: RepairWatchArgs) -> Result<ExitCode> {
    validate_timeout(args.timeout_ms, "repair watch timeout")?;
    if args.batch_window_ms > 5_000 || args.batch_window_ms > args.timeout_ms {
        anyhow::bail!(
            "repair batch window must be at most 5000ms and no longer than the watch timeout"
        );
    }
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let _repair_lock = store.acquire_repair_lock().await?;
    let state = load_active(&store, &workspace, &args.session).await?;
    let repairs_path = store.root().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(repairs_path.clone()).await?;
    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    let recovered = ledger
        .recover_expired_leases(&state.session, unix_ms())
        .await?;
    let recovered_ids = recovered
        .iter()
        .map(|(_, event)| event.finding_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    for event in ledger.current_events() {
        if !recovered_ids.contains(event.finding_id.as_str()) {
            browser.project_repair_event(&event).await?;
        }
    }
    for (_, event) in recovered {
        browser.project_repair_event(&event).await?;
    }
    for action in browser.take_human_repair_actions(args.limit).await? {
        for (_, event) in ledger
            .apply_human_action(&state.session, action, unix_ms())
            .await?
        {
            browser.project_repair_event(&event).await?;
        }
    }
    for (_, event) in ledger.resolve_conflicts(&state.session, unix_ms()).await? {
        browser.project_repair_event(&event).await?;
    }
    capture_missing_before_evidence(&state.session, &mut ledger, &mut browser, args.limit).await?;
    let mut queued = ledger.queued(args.limit);
    if queued.is_empty() {
        let findings = browser
            .wait_for_repair_batch(args.limit, args.timeout_ms, args.batch_window_ms)
            .await?;
        let created = ledger.ingest(&state.session, findings, unix_ms()).await?;
        for record in created {
            let evidence = browser
                .capture_owned_repair_evidence(&RepairEvidenceRequest {
                    finding_id: record.finding.id.clone(),
                    attempt_id: None,
                    phase: RepairEvidencePhase::Before,
                })
                .await?;
            ledger
                .attach_before_evidence(&state.session, &record.finding.id, evidence)
                .await?;
        }
        for (_, event) in ledger.resolve_conflicts(&state.session, unix_ms()).await? {
            browser.project_repair_event(&event).await?;
        }
        capture_missing_before_evidence(&state.session, &mut ledger, &mut browser, args.limit)
            .await?;
        queued = ledger.queued(args.limit);
    }
    emit(
        args.json,
        json!({
            "protocol_revision": ACTION_PROTOCOL_REVISION,
            "session": state.session,
            "repairs": queued,
            "batches": ledger.batches(),
            "ledger_path": repairs_path,
        }),
        format!("{} queued repair finding(s)", queued.len()),
    )?;
    Ok(ExitCode::SUCCESS)
}

async fn capture_missing_before_evidence(
    session: &str,
    ledger: &mut RepairLedger,
    browser: &mut a3s_test_driver_web::AgentBrowserSession,
    limit: usize,
) -> Result<()> {
    let missing = ledger
        .queued(limit)
        .into_iter()
        .filter(|record| record.before_evidence.is_none())
        .collect::<Vec<_>>();
    for record in missing {
        let evidence = browser
            .capture_owned_repair_evidence(&RepairEvidenceRequest {
                finding_id: record.finding.id.clone(),
                attempt_id: None,
                phase: RepairEvidencePhase::Before,
            })
            .await?;
        ledger
            .attach_before_evidence(session, &record.finding.id, evidence)
            .await?;
    }
    Ok(())
}

pub(super) async fn transition(
    args: RepairTransitionArgs,
    status: RepairStatus,
) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let _repair_lock = store.acquire_repair_lock().await?;
    let state = load_active(&store, &workspace, &args.session).await?;
    let mut ledger = RepairLedger::load(store.root().join("repairs.jsonl")).await?;
    if args.lease_expires_at_ms.is_some() && args.lease_ms != 300_000 {
        anyhow::bail!("use either --lease-expires-at-ms or --lease-ms, not both");
    }
    if args.lease_ms == 0 || args.lease_ms > 15 * 60 * 1_000 {
        anyhow::bail!("repair lease duration must be between 1ms and 15 minutes");
    }
    let now_ms = unix_ms();
    let attempt_id = if status == RepairStatus::Claimed {
        Some(
            args.attempt_id
                .unwrap_or_else(|| derived_attempt_id(&args.request_id)),
        )
    } else {
        args.attempt_id
    };
    let lease_expires_at_ms = if status == RepairStatus::Claimed {
        Some(
            args.lease_expires_at_ms
                .unwrap_or_else(|| now_ms.saturating_add(args.lease_ms)),
        )
    } else {
        args.lease_expires_at_ms
    };
    let request = RepairTransition {
        session: state.session.clone(),
        finding_id: args.finding_id,
        request_id: args.request_id,
        status,
        actor: RepairActor::Agent,
        attempt_id,
        lease_expires_at_ms,
        summary: args.summary,
        message: args.message,
        verification: None,
    };
    let (repair, event) = ledger.transition(request, now_ms).await?;
    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    browser.project_repair_event(&event).await?;
    emit(
        args.json,
        json!({
            "protocol_revision": ACTION_PROTOCOL_REVISION,
            "session": state.session,
            "repair": repair,
            "next": next_command(
                &state.session,
                &repair.finding.id,
                repair.status,
                repair.attempt_id.as_deref(),
            ),
        }),
        format!("Repair '{}' is {:?}", repair.finding.id, repair.status),
    )?;
    Ok(ExitCode::SUCCESS)
}

pub(super) async fn verify(args: RepairVerifyArgs) -> Result<ExitCode> {
    let json_output = args.json;
    let checks = serde_json::from_str(&args.checks_json).context("checks JSON is invalid")?;
    let request = RepairVerifyRequest {
        session: args.session.clone(),
        finding_id: args.finding_id.clone(),
        request_id: args.request_id,
        success_criteria_passed: args.success_criteria_passed,
        changed_files: args.changed_files,
        checks,
        acl_candidate: args.acl_candidate,
        summary: args.summary,
    };
    validate_repair_verification_request(&request).map_err(anyhow::Error::new)?;
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &request.session)?;
    let _repair_lock = store.acquire_repair_lock().await?;
    let state = load_active(&store, &workspace, &request.session).await?;
    let mut ledger = RepairLedger::load(store.root().join("repairs.jsonl")).await?;
    let current = ledger
        .get(&request.finding_id)
        .context("repair finding does not exist")?;
    if current.status != RepairStatus::Verifying {
        anyhow::bail!("repair verification requires the verifying state");
    }
    let attempt_id = current
        .attempt_id
        .clone()
        .context("repair verification is missing its active attempt")?;
    let before_evidence = current
        .before_evidence
        .clone()
        .context("repair verification is missing A3S-owned before evidence")?;
    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    let after_evidence = browser
        .capture_owned_repair_evidence(&RepairEvidenceRequest {
            finding_id: current.finding.id.clone(),
            attempt_id: Some(attempt_id.clone()),
            phase: RepairEvidencePhase::After,
        })
        .await?;
    let mut verification = build_repair_verification(
        &current.finding,
        &attempt_id,
        &after_evidence.context,
        after_evidence.console_errors,
        after_evidence.page_errors,
        &before_evidence,
        &after_evidence,
        &request,
    )
    .map_err(anyhow::Error::new)?;
    if verification.passed {
        match verification.acl_candidate.as_deref() {
            Some(candidate) => {
                let proof = browser
                    .prove_repair_acl_candidate(
                        &current.finding.id,
                        &attempt_id,
                        &current.finding.url,
                        candidate,
                    )
                    .await?;
                verification.passed = proof.passed;
                verification.acl_proof = Some(proof);
            }
            None => {
                verification.passed = false;
                verification.summary = format!(
                    "{}; no stable regression ACL could be generated or supplied",
                    verification.summary
                );
            }
        }
    }
    let passed = verification.passed;
    let verification_request_id = request.request_id.clone();
    let request = RepairTransition {
        session: state.session.clone(),
        finding_id: current.finding.id.clone(),
        request_id: request.request_id,
        status: if passed {
            RepairStatus::ReviewReady
        } else {
            RepairStatus::VerificationFailed
        },
        actor: RepairActor::A3sTest,
        attempt_id: Some(attempt_id.clone()),
        lease_expires_at_ms: None,
        summary: Some(request.summary),
        message: None,
        verification: Some(verification),
    };
    let (mut repair, event) = ledger.transition(request, unix_ms()).await?;
    browser.project_repair_event(&event).await?;
    if passed && state.auto_resolve_repairs {
        let request = RepairTransition {
            session: state.session.clone(),
            finding_id: current.finding.id.clone(),
            request_id: auto_resolution_request_id(&verification_request_id),
            status: RepairStatus::Resolved,
            actor: RepairActor::A3sTest,
            attempt_id: Some(attempt_id),
            lease_expires_at_ms: None,
            summary: Some("A3S Test automatically accepted the fully verified repair".to_string()),
            message: None,
            verification: None,
        };
        let (resolved, event) = ledger.transition(request, unix_ms()).await?;
        browser.project_repair_event(&event).await?;
        repair = resolved;
    }
    emit(
        json_output,
        json!({
            "protocol_revision": ACTION_PROTOCOL_REVISION,
            "session": state.session,
            "repair": repair,
            "next": next_command(
                &state.session,
                &repair.finding.id,
                repair.status,
                repair.attempt_id.as_deref(),
            ),
        }),
        format!("Repair '{}' verification completed", repair.finding.id),
    )?;
    Ok(ExitCode::SUCCESS)
}

fn derived_attempt_id(request_id: &str) -> String {
    let prefix = request_id.chars().take(120).collect::<String>();
    format!("attempt-{prefix}")
}

fn auto_resolution_request_id(verification_request_id: &str) -> String {
    let prefix = verification_request_id
        .chars()
        .take(115)
        .collect::<String>();
    format!("auto-resolve-{prefix}")
}

fn next_command(
    session: &str,
    finding_id: &str,
    status: RepairStatus,
    attempt_id: Option<&str>,
) -> String {
    let attempt = attempt_id
        .map(|value| format!(" --attempt-id {value}"))
        .unwrap_or_default();
    match status {
        RepairStatus::Claimed => format!(
            "a3s-test agent repair-progress {finding_id} --session {session} --request-id <id>{attempt} --json"
        ),
        RepairStatus::Repairing => format!(
            "a3s-test agent repair-complete {finding_id} --session {session} --request-id <id>{attempt} --json"
        ),
        RepairStatus::Verifying => {
            format!("a3s-test agent observe --session {session} --interactive --json")
        }
        _ => format!("a3s-test agent repair-watch --session {session} --json"),
    }
}
