use std::process::ExitCode;

use a3s_test_core::{
    RepairActor, RepairCheckResult, RepairEvidencePhase, RepairEvidenceRequest, RepairStatus,
    ACTION_PROTOCOL_REVISION,
};
use a3s_test_session::{
    build_repair_verification_with_plan, latest_prior_acl_proof_passed,
    validate_repair_verification_request, RepairLedger, RepairTransition, RepairVerifyRequest,
};
use anyhow::{Context, Result};
use serde_json::json;

use super::args::{RepairTransitionArgs, RepairVerifyArgs};
use super::store::{AgentSessionState, AgentSessionStore};
use super::{
    canonical_workspace, connect, emit, load_active, load_store, unix_ms, BrowserConnectionPurpose,
};

pub(super) async fn transition(
    args: RepairTransitionArgs,
    status: RepairStatus,
) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let repair_workspace = store.repair_workspace();
    let mut repair_lock = repair_workspace
        .acquire()
        .await
        .map_err(anyhow::Error::new)?;
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
    let (repair, event) = ledger
        .transition_in_workspace(request, now_ms, &mut repair_lock)
        .await
        .map_err(anyhow::Error::new)?;
    drop(repair_lock);
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
    let configured_checks = args.config.clone();
    let manual_checks = args
        .checks_json
        .as_deref()
        .map(serde_json::from_str::<Vec<RepairCheckResult>>)
        .transpose()
        .context("checks JSON is invalid")?;
    let mut request = RepairVerifyRequest {
        session: args.session.clone(),
        finding_id: args.finding_id.clone(),
        request_id: args.request_id,
        success_criteria_passed: args.success_criteria_passed,
        changed_files: args.changed_files,
        checks: manual_checks.clone().unwrap_or_default(),
        acl_candidate: args.acl_candidate,
        summary: args.summary,
    };
    validate_repair_verification_request(&request).map_err(anyhow::Error::new)?;
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &request.session)?;
    let repair_workspace = store.repair_workspace();
    let state = load_active(&store, &workspace, &request.session).await?;
    let repairs_path = store.root().join("repairs.jsonl");
    let (mut ledger, current, attempt_id) = {
        let repair_lock = repair_workspace
            .acquire()
            .await
            .map_err(anyhow::Error::new)?;
        let ledger = RepairLedger::load(repairs_path.clone()).await?;
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
        repair_lock
            .validate_attempt_owner(
                &request.session,
                &request.finding_id,
                &attempt_id,
                RepairStatus::Verifying,
                unix_ms(),
            )
            .await
            .map_err(anyhow::Error::new)?;
        (ledger, current, attempt_id)
    };
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
    let prior_acl_proof_passed = latest_prior_acl_proof_passed(&current.attempts, &attempt_id);
    let automatic_run = if manual_checks.is_none() {
        Some(
            crate::workspace::run_configured_checks(
                &workspace,
                &configured_checks,
                &current.finding,
                &request.changed_files,
                after_evidence
                    .console_errors
                    .saturating_sub(before_evidence.console_errors),
                after_evidence
                    .page_errors
                    .saturating_sub(before_evidence.page_errors),
                prior_acl_proof_passed,
            )
            .await?,
        )
    } else {
        None
    };
    let (verification_checks, planned_slice) = match automatic_run {
        Some(crate::workspace::VerificationRun {
            catalog,
            results,
            slice,
        }) => {
            request.checks = results;
            (catalog, Some(slice))
        }
        None => (Vec::new(), None),
    };
    validate_repair_verification_request(&request).map_err(anyhow::Error::new)?;
    let mut verification = build_repair_verification_with_plan(
        &current.finding,
        &attempt_id,
        &before_evidence,
        &after_evidence,
        &request,
        prior_acl_proof_passed,
        &verification_checks,
    )
    .map_err(anyhow::Error::new)?;
    if planned_slice
        .as_ref()
        .is_some_and(|planned| verification.verification_slice.as_ref() != Some(planned))
    {
        anyhow::bail!("verification slice changed between planning and execution");
    }
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
    let mut repair_lock = repair_workspace
        .acquire()
        .await
        .map_err(anyhow::Error::new)?;
    ledger.reload().await.map_err(anyhow::Error::new)?;
    ledger
        .require_attempt_state(&request.finding_id, RepairStatus::Verifying, &attempt_id)
        .map_err(anyhow::Error::new)?;
    repair_lock
        .validate_attempt_owner(
            &request.session,
            &request.finding_id,
            &attempt_id,
            RepairStatus::Verifying,
            unix_ms(),
        )
        .await
        .map_err(anyhow::Error::new)?;
    let transition = RepairTransition {
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
    let (mut repair, event) = ledger
        .transition_in_workspace(transition, unix_ms(), &mut repair_lock)
        .await
        .map_err(anyhow::Error::new)?;
    drop(repair_lock);
    browser.project_repair_event(&event).await?;
    if passed && state.auto_resolve_repairs {
        let mut repair_lock = repair_workspace
            .acquire()
            .await
            .map_err(anyhow::Error::new)?;
        ledger.reload().await.map_err(anyhow::Error::new)?;
        ledger
            .require_attempt_state(&current.finding.id, RepairStatus::ReviewReady, &attempt_id)
            .map_err(anyhow::Error::new)?;
        let transition = RepairTransition {
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
        let (resolved, event) = ledger
            .transition_in_workspace(transition, unix_ms(), &mut repair_lock)
            .await
            .map_err(anyhow::Error::new)?;
        drop(repair_lock);
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

pub(super) async fn interrupt_for_session_close(
    store: &AgentSessionStore,
    state: &AgentSessionState,
) -> Result<()> {
    let repairs_path = store.root().join("repairs.jsonl");
    if !repairs_path.is_file() {
        return Ok(());
    }
    let repair_workspace = store.repair_workspace();
    let mut repair_lock = repair_workspace
        .acquire()
        .await
        .map_err(anyhow::Error::new)?;
    let mut ledger = RepairLedger::load(repairs_path).await?;
    ledger
        .interrupt_active_mutation_in_workspace(&state.session, unix_ms(), &mut repair_lock)
        .await
        .map_err(anyhow::Error::new)?;
    Ok(())
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
