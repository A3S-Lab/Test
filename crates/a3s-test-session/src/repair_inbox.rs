use std::cmp::Ordering;
use std::collections::HashSet;

use a3s_test_core::{RepairActor, RepairIntent, RepairSeverity, RepairStatus};
use serde::{Deserialize, Serialize};

use super::{loop_record, validate_component, RepairLedger, RepairRecord};
use crate::SessionError;

pub const REPAIR_INBOX_PROTOCOL: &str = "a3s.test.repair-inbox/1";
pub const MAX_REPAIR_INBOX_ITEMS: usize = 100;
pub const MAX_REPAIR_INBOX_SESSIONS: usize = 4_096;
const MAX_REPAIR_INBOX_CANDIDATES: usize = 10_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairInbox {
    pub protocol: String,
    pub scope: RepairInboxScope,
    pub session: Option<String>,
    pub include_terminal: bool,
    pub sessions_scanned: usize,
    pub total: usize,
    pub truncated: bool,
    pub items: Vec<RepairInboxItem>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairInboxScope {
    Workspace,
    Session,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairInboxItem {
    pub session: String,
    pub finding_id: String,
    pub batch_id: String,
    pub sequence: u64,
    pub status: RepairStatus,
    pub updated_at_ms: u64,
    pub attempt_id: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub lease_state: RepairInboxLeaseState,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub intent: RepairInboxIntent,
    pub next: RepairInboxNext,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairInboxIntent {
    pub instruction: String,
    pub success_criteria: Option<String>,
    pub kind: RepairIntent,
    pub severity: RepairSeverity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairInboxLeaseState {
    None,
    Active,
    Expired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairInboxNext {
    pub action: RepairInboxNextAction,
    pub actor: Option<RepairActor>,
    pub mcp_tool: Option<String>,
    pub cli_command: Option<String>,
    pub requires_active_session: bool,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairInboxNextAction {
    ReconcileLease,
    Claim,
    StartEditing,
    ReportChange,
    Verify,
    AwaitInput,
    AwaitReview,
    ReopenOrStop,
    Complete,
    InspectOnly,
}

impl RepairInbox {
    pub fn derive<'a, I>(
        session: Option<&str>,
        ledgers: I,
        now_ms: u64,
        include_terminal: bool,
        limit: usize,
    ) -> Result<Self, SessionError>
    where
        I: IntoIterator<Item = (&'a str, &'a RepairLedger)>,
    {
        if !(1..=MAX_REPAIR_INBOX_ITEMS).contains(&limit) {
            return Err(SessionError::new(
                "test.session.repair_inbox_invalid",
                format!("repair inbox limit must be between 1 and {MAX_REPAIR_INBOX_ITEMS}"),
            ));
        }
        if let Some(session) = session {
            validate_component(session, "session id")?;
        }

        let mut seen_sessions = HashSet::new();
        let mut items = Vec::new();
        for (ledger_session, ledger) in ledgers {
            validate_component(ledger_session, "session id")?;
            if session.is_some_and(|expected| expected != ledger_session) {
                return Err(session_mismatch(session, ledger_session));
            }
            if !seen_sessions.insert(ledger_session.to_string()) {
                return Err(SessionError::new(
                    "test.session.repair_inbox_invalid",
                    format!("repair inbox repeated session '{ledger_session}'"),
                ));
            }
            if seen_sessions.len() > MAX_REPAIR_INBOX_SESSIONS {
                return Err(SessionError::new(
                    "test.session.repair_inbox_too_large",
                    format!("repair inbox exceeds {MAX_REPAIR_INBOX_SESSIONS} session ledgers"),
                ));
            }
            if let Some(owner) = ledger.session.as_deref() {
                if owner != ledger_session {
                    return Err(session_mismatch(Some(owner), ledger_session));
                }
            }
            for finding_id in &ledger.order {
                let Some(record) = ledger.records.get(finding_id) else {
                    return Err(SessionError::new(
                        "test.session.repair_ledger_invalid",
                        format!("repair ledger order references missing finding '{finding_id}'"),
                    ));
                };
                if !include_terminal && terminal(record.status) {
                    continue;
                }
                if items.len() >= MAX_REPAIR_INBOX_CANDIDATES {
                    return Err(SessionError::new(
                        "test.session.repair_inbox_too_large",
                        format!(
                            "repair inbox exceeds {MAX_REPAIR_INBOX_CANDIDATES} matching findings"
                        ),
                    ));
                }
                items.push(RepairInboxItem::from_record(ledger_session, record, now_ms));
            }
        }

        items.sort_by(compare_items);
        let total = items.len();
        items.truncate(limit);
        Ok(Self {
            protocol: REPAIR_INBOX_PROTOCOL.to_string(),
            scope: if session.is_some() {
                RepairInboxScope::Session
            } else {
                RepairInboxScope::Workspace
            },
            session: session.map(str::to_string),
            include_terminal,
            sessions_scanned: seen_sessions.len(),
            total,
            truncated: total > items.len(),
            items,
        })
    }
}

impl RepairInboxItem {
    fn from_record(session: &str, record: &RepairRecord, now_ms: u64) -> Self {
        let lease_state = lease_state(record, now_ms);
        Self {
            session: session.to_string(),
            finding_id: record.finding.id.clone(),
            batch_id: record.finding.batch_id.clone(),
            sequence: record.sequence,
            status: record.status,
            updated_at_ms: record.updated_at_ms,
            attempt_id: record.attempt_id.clone(),
            lease_expires_at_ms: record.lease_expires_at_ms,
            lease_state,
            summary: record.summary.clone(),
            message: record.message.clone(),
            intent: RepairInboxIntent {
                instruction: record.finding.instruction.clone(),
                success_criteria: record.finding.success_criteria.clone(),
                kind: record.finding.intent,
                severity: record.finding.severity,
            },
            next: next_projection(session, record, lease_state),
        }
    }
}

fn next_projection(
    session: &str,
    record: &RepairRecord,
    lease_state: RepairInboxLeaseState,
) -> RepairInboxNext {
    if lease_state == RepairInboxLeaseState::Expired {
        return RepairInboxNext {
            action: RepairInboxNextAction::ReconcileLease,
            actor: Some(RepairActor::A3sTest),
            mcp_tool: Some("test_repair_watch".to_string()),
            cli_command: Some(format!(
                "a3s-test agent repair-watch --session {session} --limit 1 --timeout-ms 1 --batch-window-ms 0 --json"
            )),
            requires_active_session: true,
            reason: "The mutation lease expired; reconcile the durable ledger and workspace before continuing or claiming another repair."
                .to_string(),
        };
    }
    let resume = loop_record::resume_projection(session, record);
    RepairInboxNext {
        action: match resume.action {
            loop_record::RepairLoopResumeAction::Claim => RepairInboxNextAction::Claim,
            loop_record::RepairLoopResumeAction::StartEditing => {
                RepairInboxNextAction::StartEditing
            }
            loop_record::RepairLoopResumeAction::ReportChange => {
                RepairInboxNextAction::ReportChange
            }
            loop_record::RepairLoopResumeAction::Verify => RepairInboxNextAction::Verify,
            loop_record::RepairLoopResumeAction::AwaitInput => RepairInboxNextAction::AwaitInput,
            loop_record::RepairLoopResumeAction::AwaitReview => RepairInboxNextAction::AwaitReview,
            loop_record::RepairLoopResumeAction::ReopenOrStop => {
                RepairInboxNextAction::ReopenOrStop
            }
            loop_record::RepairLoopResumeAction::Complete => RepairInboxNextAction::Complete,
            loop_record::RepairLoopResumeAction::InspectOnly => RepairInboxNextAction::InspectOnly,
        },
        actor: resume.actor,
        mcp_tool: resume.mcp_tool,
        cli_command: resume.cli_command,
        requires_active_session: resume.requires_active_session,
        reason: resume.reason,
    }
}

fn lease_state(record: &RepairRecord, now_ms: u64) -> RepairInboxLeaseState {
    if !matches!(
        record.status,
        RepairStatus::Claimed | RepairStatus::Repairing | RepairStatus::Verifying
    ) {
        return RepairInboxLeaseState::None;
    }
    match record.lease_expires_at_ms {
        Some(expires_at_ms) if expires_at_ms > now_ms => RepairInboxLeaseState::Active,
        Some(_) | None => RepairInboxLeaseState::Expired,
    }
}

fn terminal(status: RepairStatus) -> bool {
    matches!(
        status,
        RepairStatus::Resolved
            | RepairStatus::Dismissed
            | RepairStatus::Cancelled
            | RepairStatus::Failed
    )
}

fn compare_items(left: &RepairInboxItem, right: &RepairInboxItem) -> Ordering {
    priority(left.next.action)
        .cmp(&priority(right.next.action))
        .then_with(|| {
            if left.next.action == RepairInboxNextAction::Complete
                && right.next.action == RepairInboxNextAction::Complete
            {
                right.updated_at_ms.cmp(&left.updated_at_ms)
            } else {
                left.updated_at_ms.cmp(&right.updated_at_ms)
            }
        })
        .then_with(|| left.session.cmp(&right.session))
        .then_with(|| left.finding_id.cmp(&right.finding_id))
}

fn priority(action: RepairInboxNextAction) -> u8 {
    match action {
        RepairInboxNextAction::ReconcileLease => 0,
        RepairInboxNextAction::StartEditing
        | RepairInboxNextAction::ReportChange
        | RepairInboxNextAction::Verify => 1,
        RepairInboxNextAction::Claim => 2,
        RepairInboxNextAction::AwaitInput
        | RepairInboxNextAction::AwaitReview
        | RepairInboxNextAction::ReopenOrStop => 3,
        RepairInboxNextAction::InspectOnly => 4,
        RepairInboxNextAction::Complete => 5,
    }
}

fn session_mismatch(expected: Option<&str>, actual: &str) -> SessionError {
    SessionError::new(
        "test.session.repair_session_mismatch",
        format!(
            "repair inbox expected session '{}', not '{actual}'",
            expected.unwrap_or("workspace")
        ),
    )
}
