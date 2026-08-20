use a3s_test_core::{
    Evidence, PageContextSource, PageContextSourceMapping, RepairAclProof, RepairActor,
    RepairCheckResult, RepairDesignReference, RepairIntent, RepairSeverity, RepairStatus,
    RepairTarget, RepairThreadMessage, RepairVerification, RepairVerificationSlice,
};
use serde::{Deserialize, Serialize};

use super::{RepairLedger, RepairRecord};
use crate::SessionError;

pub const REPAIR_LOOP_PROTOCOL: &str = "a3s.test.repair-loop-record/1";
const MAX_SOURCE_TARGETS: usize = 200;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairLoopRecord {
    pub protocol: String,
    pub session: String,
    pub finding_id: String,
    pub batch_id: String,
    pub sequence: u64,
    pub status: RepairStatus,
    pub updated_at_ms: u64,
    pub attempt_id: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub intent: RepairLoopIntent,
    pub source_mapping: RepairLoopSourceMapping,
    pub before_evidence: Option<RepairLoopEvidence>,
    pub change: Option<RepairLoopChange>,
    pub verification: Option<RepairLoopVerification>,
    pub acl_promotion: RepairLoopAclPromotion,
    pub attempts: Vec<RepairLoopAttempt>,
    pub resume: RepairLoopResume,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairLoopIntent {
    pub instruction: String,
    pub success_criteria: Option<String>,
    pub kind: RepairIntent,
    pub severity: RepairSeverity,
    pub target: RepairTarget,
    pub design_reference: Option<RepairDesignReference>,
    pub page_id: String,
    pub url: String,
    pub context_revision: u64,
    pub created_at: String,
    pub submitted_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairLoopSourceMapping {
    pub component_source: Option<PageContextSource>,
    pub targets: Vec<RepairLoopSourceTarget>,
    pub truncated: bool,
    pub malformed: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairLoopSourceTarget {
    pub node_id: String,
    pub mapping: Option<PageContextSourceMapping>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairLoopChange {
    pub attempt_id: String,
    pub reported_at_ms: u64,
    pub changed_files: Vec<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairLoopAttempt {
    pub id: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub status: RepairStatus,
    pub replies: Vec<RepairThreadMessage>,
    pub before_evidence: Option<RepairLoopEvidence>,
    pub change: Option<RepairLoopChange>,
    pub verification: Option<RepairLoopVerification>,
    pub acl_promotion: RepairLoopAclPromotion,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairLoopVerification {
    pub finding_id: String,
    pub attempt_id: String,
    pub before_revision: u64,
    pub after_revision: u64,
    pub target_found: bool,
    pub success_criteria_passed: Option<bool>,
    pub new_console_errors: u32,
    pub new_page_errors: u32,
    pub changed_files: Vec<String>,
    pub checks: Vec<RepairCheckResult>,
    pub verification_slice: Option<RepairVerificationSlice>,
    pub before_evidence: Option<RepairLoopEvidence>,
    pub after_evidence: Option<RepairLoopEvidence>,
    pub passed: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairLoopEvidence {
    pub captured_at_ms: u64,
    pub context_revision: u64,
    pub context_sha256: String,
    pub console_errors: u32,
    pub page_errors: u32,
    pub screenshot: Evidence,
    pub screenshot_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairLoopAclStatus {
    NotGenerated,
    CandidateGenerated,
    ProofFailed,
    ProofPassed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairLoopAclPromotion {
    pub status: RepairLoopAclStatus,
    pub candidate: Option<String>,
    pub proof: Option<RepairAclProof>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairLoopResumeAction {
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairLoopResume {
    pub action: RepairLoopResumeAction,
    pub actor: Option<RepairActor>,
    pub mcp_tool: Option<String>,
    pub cli_command: Option<String>,
    pub requires_active_session: bool,
    pub reason: String,
}

impl RepairLedger {
    pub fn inspect_loop(
        &self,
        session: &str,
        finding_id: &str,
    ) -> Result<RepairLoopRecord, SessionError> {
        super::validate_component(session, "session id")?;
        super::validate_component(finding_id, "finding id")?;
        if let Some(owner) = self.session.as_deref() {
            if owner != session {
                return Err(SessionError::new(
                    "test.session.repair_session_mismatch",
                    format!("repair ledger belongs to session '{owner}', not '{session}'"),
                ));
            }
        }
        let record = self
            .records
            .get(finding_id)
            .ok_or_else(|| super::not_found(finding_id))?;
        Ok(RepairLoopRecord::from_record(session, record))
    }
}

impl RepairLoopRecord {
    fn from_record(session: &str, record: &RepairRecord) -> Self {
        let active_verification = record.verification.as_ref().filter(|verification| {
            record.attempt_id.as_deref() == Some(verification.attempt_id.as_str())
        });
        Self {
            protocol: REPAIR_LOOP_PROTOCOL.to_string(),
            session: session.to_string(),
            finding_id: record.finding.id.clone(),
            batch_id: record.finding.batch_id.clone(),
            sequence: record.sequence,
            status: record.status,
            updated_at_ms: record.updated_at_ms,
            attempt_id: record.attempt_id.clone(),
            lease_expires_at_ms: record.lease_expires_at_ms,
            summary: record.summary.clone(),
            message: record.message.clone(),
            intent: RepairLoopIntent::from_record(record),
            source_mapping: source_mapping(record),
            before_evidence: record.before_evidence.as_ref().map(evidence_projection),
            change: record.change.as_ref().map(change_projection),
            verification: active_verification.map(verification_projection),
            acl_promotion: acl_projection(active_verification),
            attempts: record
                .attempts
                .iter()
                .map(|attempt| RepairLoopAttempt {
                    id: attempt.id.clone(),
                    started_at_ms: attempt.started_at_ms,
                    finished_at_ms: attempt.finished_at_ms,
                    status: attempt.status,
                    replies: attempt.replies.clone(),
                    before_evidence: attempt.before_evidence.as_ref().map(evidence_projection),
                    change: attempt.change.as_ref().map(change_projection),
                    verification: attempt.verification.as_ref().map(verification_projection),
                    acl_promotion: acl_projection(attempt.verification.as_ref()),
                })
                .collect(),
            resume: resume_projection(session, record),
        }
    }
}

impl RepairLoopIntent {
    fn from_record(record: &RepairRecord) -> Self {
        Self {
            instruction: record.finding.instruction.clone(),
            success_criteria: record.finding.success_criteria.clone(),
            kind: record.finding.intent,
            severity: record.finding.severity,
            target: record.finding.target.clone(),
            design_reference: record.finding.design_reference.clone(),
            page_id: record.finding.page_id.clone(),
            url: record.finding.url.clone(),
            context_revision: record.finding.context_revision,
            created_at: record.finding.created_at.clone(),
            submitted_at: record.finding.submitted_at.clone(),
        }
    }
}

pub fn validate_repair_verification_change(
    record: &RepairRecord,
    changed_files: &[String],
) -> Result<(), SessionError> {
    if !crate::verification::valid_repair_changed_files(changed_files) {
        return Err(SessionError::new(
            "test.session.repair_change_invalid",
            "repair verification changed files are unbounded or invalid",
        ));
    }
    let Some(change) = record.change.as_ref() else {
        return Ok(());
    };
    if record.attempt_id.as_deref() == Some(change.attempt_id.as_str())
        && change.changed_files == changed_files
    {
        return Ok(());
    }
    Err(SessionError::new(
        "test.session.repair_change_mismatch",
        "repair verification changed files differ from the append-only completion report",
    ))
}

fn source_mapping(record: &RepairRecord) -> RepairLoopSourceMapping {
    let component_value = record.finding.context.pointer("/component/source");
    let component_source = component_value
        .cloned()
        .and_then(|value| serde_json::from_value::<PageContextSource>(value).ok())
        .filter(valid_component_source);
    let mut malformed = component_value.is_some() && component_source.is_none();
    let nodes = record
        .finding
        .context
        .get("nodes")
        .and_then(serde_json::Value::as_array);
    let mut targets = Vec::new();
    for node_id in record
        .finding
        .target
        .node_ids
        .iter()
        .take(MAX_SOURCE_TARGETS)
    {
        let raw = nodes.and_then(|nodes| {
            nodes.iter().find(|node| {
                node.get("id").and_then(serde_json::Value::as_str) == Some(node_id.as_str())
            })
        });
        let raw_mapping = raw.and_then(|node| node.get("sourceMapping"));
        let mapping = raw_mapping
            .cloned()
            .and_then(|value| serde_json::from_value::<PageContextSourceMapping>(value).ok())
            .filter(|mapping| mapping.validate().is_ok());
        malformed |= raw_mapping.is_some() && mapping.is_none();
        targets.push(RepairLoopSourceTarget {
            node_id: node_id.clone(),
            mapping,
        });
    }
    RepairLoopSourceMapping {
        component_source,
        targets,
        truncated: record.finding.target.node_ids.len() > MAX_SOURCE_TARGETS,
        malformed,
    }
}

fn valid_component_source(source: &PageContextSource) -> bool {
    !source.file.is_empty()
        && source.file.len() <= 2_048
        && !source.file.chars().any(char::is_control)
        && source
            .line
            .is_none_or(|value| value > 0 && value <= 10_000_000)
        && source
            .column
            .is_none_or(|value| source.line.is_some() && value > 0 && value <= 10_000_000)
        && source
            .end_line
            .is_none_or(|value| source.line.is_some() && value > 0 && value <= 10_000_000)
        && source
            .end_column
            .is_none_or(|value| source.end_line.is_some() && value > 0 && value <= 10_000_000)
}

fn change_projection(change: &a3s_test_core::RepairChange) -> RepairLoopChange {
    RepairLoopChange {
        attempt_id: change.attempt_id.clone(),
        reported_at_ms: change.reported_at_ms,
        changed_files: change.changed_files.clone(),
        summary: change.summary.clone(),
    }
}

fn evidence_projection(evidence: &a3s_test_core::RepairEvidenceBundle) -> RepairLoopEvidence {
    RepairLoopEvidence {
        captured_at_ms: evidence.captured_at_ms,
        context_revision: evidence.context_revision,
        context_sha256: evidence.context_sha256.clone(),
        console_errors: evidence.console_errors,
        page_errors: evidence.page_errors,
        screenshot: evidence.screenshot.clone(),
        screenshot_sha256: evidence.screenshot_sha256.clone(),
    }
}

fn verification_projection(verification: &RepairVerification) -> RepairLoopVerification {
    RepairLoopVerification {
        finding_id: verification.finding_id.clone(),
        attempt_id: verification.attempt_id.clone(),
        before_revision: verification.before_revision,
        after_revision: verification.after_revision,
        target_found: verification.target_found,
        success_criteria_passed: verification.success_criteria_passed,
        new_console_errors: verification.new_console_errors,
        new_page_errors: verification.new_page_errors,
        changed_files: verification.changed_files.clone(),
        checks: verification.checks.clone(),
        verification_slice: verification.verification_slice.clone(),
        before_evidence: verification
            .before_evidence
            .as_ref()
            .map(evidence_projection),
        after_evidence: verification
            .after_evidence
            .as_ref()
            .map(evidence_projection),
        passed: verification.passed,
        summary: verification.summary.clone(),
    }
}

fn acl_projection(verification: Option<&RepairVerification>) -> RepairLoopAclPromotion {
    let candidate = verification.and_then(|value| value.acl_candidate.clone());
    let proof = verification.and_then(|value| value.acl_proof.clone());
    let status = match (candidate.as_ref(), proof.as_ref()) {
        (None, _) => RepairLoopAclStatus::NotGenerated,
        (Some(_), None) => RepairLoopAclStatus::CandidateGenerated,
        (Some(_), Some(proof)) if proof.passed => RepairLoopAclStatus::ProofPassed,
        (Some(_), Some(_)) => RepairLoopAclStatus::ProofFailed,
    };
    RepairLoopAclPromotion {
        status,
        candidate,
        proof,
    }
}

fn resume_projection(session: &str, record: &RepairRecord) -> RepairLoopResume {
    let finding = &record.finding.id;
    let attempt = record.attempt_id.as_deref().unwrap_or("<attempt-id>");
    match record.status {
        RepairStatus::Queued => resume(
            RepairLoopResumeAction::Claim,
            Some(RepairActor::Agent),
            Some("test_repair_claim"),
            Some(format!(
                "a3s-test agent repair-claim {finding} --session {session} --request-id <id> --json"
            )),
            true,
            "The finding is ready for one lease-bound coding-agent attempt.",
        ),
        RepairStatus::Claimed => resume(
            RepairLoopResumeAction::StartEditing,
            Some(RepairActor::Agent),
            Some("test_repair_progress"),
            Some(format!(
                "a3s-test agent repair-progress {finding} --session {session} --request-id <id> --attempt-id {attempt} --json"
            )),
            true,
            "The attempt owns the workspace slot but has not reported editing yet.",
        ),
        RepairStatus::Repairing => resume(
            RepairLoopResumeAction::ReportChange,
            Some(RepairActor::Agent),
            Some("test_repair_complete"),
            Some(format!(
                "a3s-test agent repair-complete {finding} --session {session} --request-id <id> --attempt-id {attempt} --changed-file <path>... --json"
            )),
            true,
            "Finish the scoped edit and append its exact changed-files report.",
        ),
        RepairStatus::Verifying => resume(
            RepairLoopResumeAction::Verify,
            Some(RepairActor::Agent),
            Some("test_repair_verify"),
            Some(format!(
                "a3s-test agent repair-verify {finding} --session {session} --request-id <id> --changed-file <recorded-path>... --summary <summary> --json"
            )),
            true,
            "Re-observe the hot-reloaded page, then verify the recorded change with A3S-owned evidence.",
        ),
        RepairStatus::NeedsInput => resume(
            RepairLoopResumeAction::AwaitInput,
            Some(RepairActor::Human),
            None,
            Some(format!(
                "a3s-test agent repair-inspect {finding} --session {session} --json"
            )),
            false,
            "A human clarification or workspace review is required before another attempt.",
        ),
        RepairStatus::ReviewReady => resume(
            RepairLoopResumeAction::AwaitReview,
            Some(RepairActor::Human),
            None,
            Some(format!(
                "a3s-test agent repair-inspect {finding} --session {session} --json"
            )),
            false,
            "The verified change is waiting for human acceptance or dismissal.",
        ),
        RepairStatus::VerificationFailed => resume(
            RepairLoopResumeAction::ReopenOrStop,
            Some(RepairActor::Human),
            None,
            Some(format!(
                "a3s-test agent repair-inspect {finding} --session {session} --json"
            )),
            false,
            "Verification failed; preserve the attempt and wait for an explicit human retry.",
        ),
        RepairStatus::Resolved
        | RepairStatus::Dismissed
        | RepairStatus::Cancelled
        | RepairStatus::Failed => resume(
            RepairLoopResumeAction::Complete,
            None,
            None,
            None,
            false,
            "The current repair loop is terminal unless a human explicitly reopens it.",
        ),
        RepairStatus::Draft | RepairStatus::Reopened => resume(
            RepairLoopResumeAction::InspectOnly,
            None,
            None,
            Some(format!(
                "a3s-test agent repair-inspect {finding} --session {session} --json"
            )),
            false,
            "This transient state has no coding-agent mutation action.",
        ),
    }
}

fn resume(
    action: RepairLoopResumeAction,
    actor: Option<RepairActor>,
    mcp_tool: Option<&str>,
    cli_command: Option<String>,
    requires_active_session: bool,
    reason: &str,
) -> RepairLoopResume {
    RepairLoopResume {
        action,
        actor,
        mcp_tool: mcp_tool.map(str::to_string),
        cli_command,
        requires_active_session,
        reason: reason.to_string(),
    }
}
