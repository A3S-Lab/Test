use std::path::PathBuf;
use std::process::ExitCode;

use a3s_test_core::{
    RepairBatch, RepairEvidencePhase, RepairEvidenceRequest, ACTION_PROTOCOL_REVISION,
};
use a3s_test_session::{RepairLedger, RepairRecord};
use anyhow::Result;
use serde::Serialize;

use super::args::RepairWatchArgs;
use super::{
    canonical_workspace, connect, emit, load_active, load_store, unix_ms, validate_timeout,
    BrowserConnectionPurpose,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepairPickup {
    QueuedFirst,
    PageSubmission,
}

pub(crate) struct RepairWatchRequest {
    workspace: PathBuf,
    session: String,
    limit: usize,
    timeout_ms: u64,
    batch_window_ms: u64,
    pickup: RepairPickup,
}

impl RepairWatchRequest {
    pub(crate) fn bridge(workspace: PathBuf, session: String, pickup: RepairPickup) -> Self {
        Self {
            workspace,
            session,
            limit: 20,
            timeout_ms: 1_000,
            batch_window_ms: 250,
            pickup,
        }
    }

    fn cli(workspace: PathBuf, args: RepairWatchArgs) -> Self {
        Self {
            workspace,
            session: args.session,
            limit: args.limit,
            timeout_ms: args.timeout_ms,
            batch_window_ms: args.batch_window_ms,
            pickup: RepairPickup::QueuedFirst,
        }
    }

    fn validate(&self) -> Result<()> {
        validate_timeout(self.timeout_ms, "repair watch timeout")?;
        if self.batch_window_ms > 5_000 || self.batch_window_ms > self.timeout_ms {
            anyhow::bail!(
                "repair batch window must be at most 5000ms and no longer than the watch timeout"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct RepairWatchResult {
    pub(crate) protocol_revision: u32,
    pub(crate) session: String,
    pub(crate) repairs: Vec<RepairRecord>,
    pub(crate) batches: Vec<RepairBatch>,
    pub(crate) ledger_path: PathBuf,
}

pub(super) async fn watch(args: RepairWatchArgs) -> Result<ExitCode> {
    let json_output = args.json;
    let workspace = canonical_workspace().await?;
    let result = watch_session(RepairWatchRequest::cli(workspace, args)).await?;
    let repair_count = result.repairs.len();
    emit(
        json_output,
        result,
        format!("{repair_count} queued repair finding(s)"),
    )?;
    Ok(ExitCode::SUCCESS)
}

pub(crate) async fn watch_session(request: RepairWatchRequest) -> Result<RepairWatchResult> {
    request.validate()?;
    let store = load_store(&request.workspace, &request.session)?;
    let repair_workspace = store.repair_workspace();
    let state = load_active(&store, &request.workspace, &request.session).await?;
    let repairs_path = store.root().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(repairs_path.clone()).await?;
    let mut browser = connect(&state, BrowserConnectionPurpose::Turn).await?;
    let recovered = {
        let mut repair_lock = repair_workspace
            .acquire()
            .await
            .map_err(anyhow::Error::new)?;
        ledger.reload().await.map_err(anyhow::Error::new)?;
        ledger
            .recover_expired_leases_in_workspace(&state.session, unix_ms(), &mut repair_lock)
            .await?
    };
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
    for action in browser.take_human_repair_actions(request.limit).await? {
        let transitions = {
            let mut repair_lock = repair_workspace
                .acquire()
                .await
                .map_err(anyhow::Error::new)?;
            ledger
                .apply_human_action_in_workspace(
                    &state.session,
                    action,
                    unix_ms(),
                    &mut repair_lock,
                )
                .await?
        };
        for (_, event) in transitions {
            browser.project_repair_event(&event).await?;
        }
    }
    let conflicts = {
        let mut repair_lock = repair_workspace
            .acquire()
            .await
            .map_err(anyhow::Error::new)?;
        ledger
            .resolve_conflicts_in_workspace(&state.session, unix_ms(), &mut repair_lock)
            .await?
    };
    for (_, event) in conflicts {
        browser.project_repair_event(&event).await?;
    }
    capture_missing_before_evidence(
        &state.session,
        &repair_workspace,
        &mut ledger,
        &mut browser,
        request.limit,
    )
    .await?;

    let should_wait =
        request.pickup == RepairPickup::PageSubmission || ledger.queued(request.limit).is_empty();
    if should_wait {
        let findings = browser
            .wait_for_repair_batch(request.limit, request.timeout_ms, request.batch_window_ms)
            .await?;
        let created = {
            let mut repair_lock = repair_workspace
                .acquire()
                .await
                .map_err(anyhow::Error::new)?;
            ledger
                .ingest_in_workspace(&state.session, findings, unix_ms(), &mut repair_lock)
                .await?
        };
        for record in created {
            let evidence = browser
                .capture_owned_repair_evidence(&RepairEvidenceRequest {
                    finding_id: record.finding.id.clone(),
                    attempt_id: None,
                    phase: RepairEvidencePhase::Before,
                })
                .await?;
            let mut repair_lock = repair_workspace
                .acquire()
                .await
                .map_err(anyhow::Error::new)?;
            ledger
                .attach_before_evidence_in_workspace(
                    &state.session,
                    &record.finding.id,
                    evidence,
                    &mut repair_lock,
                )
                .await?;
        }
        let conflicts = {
            let mut repair_lock = repair_workspace
                .acquire()
                .await
                .map_err(anyhow::Error::new)?;
            ledger
                .resolve_conflicts_in_workspace(&state.session, unix_ms(), &mut repair_lock)
                .await?
        };
        for (_, event) in conflicts {
            browser.project_repair_event(&event).await?;
        }
        capture_missing_before_evidence(
            &state.session,
            &repair_workspace,
            &mut ledger,
            &mut browser,
            request.limit,
        )
        .await?;
    }

    Ok(RepairWatchResult {
        protocol_revision: ACTION_PROTOCOL_REVISION,
        session: state.session,
        repairs: ledger.queued(request.limit),
        batches: ledger.batches(),
        ledger_path: repairs_path,
    })
}

async fn capture_missing_before_evidence(
    session: &str,
    repair_workspace: &a3s_test_session::RepairWorkspace,
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
        let mut repair_lock = repair_workspace
            .acquire()
            .await
            .map_err(anyhow::Error::new)?;
        ledger
            .attach_before_evidence_in_workspace(
                session,
                &record.finding.id,
                evidence,
                &mut repair_lock,
            )
            .await?;
    }
    Ok(())
}
