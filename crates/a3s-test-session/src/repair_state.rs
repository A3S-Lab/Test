use super::*;

pub(super) fn apply_event(record: &mut RepairRecord, event: &RepairEventRecord) {
    record.status = event.status;
    record.sequence = event.sequence;
    if event.attempt_id.is_some() {
        record.attempt_id.clone_from(&event.attempt_id);
    }
    if event.lease_expires_at_ms.is_some() {
        record.lease_expires_at_ms = event.lease_expires_at_ms;
    } else if !matches!(
        event.status,
        RepairStatus::Claimed | RepairStatus::Repairing | RepairStatus::Verifying
    ) {
        record.lease_expires_at_ms = None;
    }
    if event.status == RepairStatus::Queued {
        record.attempt_id = None;
        record.lease_expires_at_ms = None;
        record.before_evidence = None;
    }
    record.updated_at_ms = event.timestamp_ms;
    record.summary.clone_from(&event.summary);
    record.message.clone_from(&event.message);
    if event.verification.is_some() {
        record.verification.clone_from(&event.verification);
    }
    update_attempts(record, event);
}

pub(super) fn update_attempts(record: &mut RepairRecord, event: &RepairEventRecord) {
    let Some(attempt_id) = event.attempt_id.as_deref() else {
        return;
    };
    if !record
        .attempts
        .iter()
        .any(|attempt| attempt.id == attempt_id)
    {
        record.attempts.push(RepairAttempt {
            id: attempt_id.to_string(),
            started_at_ms: event.timestamp_ms,
            finished_at_ms: None,
            status: event.status,
            replies: Vec::new(),
            verification: None,
            before_evidence: record.before_evidence.clone(),
        });
    }
    let Some(attempt) = record
        .attempts
        .iter_mut()
        .find(|attempt| attempt.id == attempt_id)
    else {
        return;
    };
    let finished = attempt.finished_at_ms.is_some();
    if !finished {
        attempt.status = event.status;
    }
    if let Some(message) = event
        .message
        .as_ref()
        .filter(|message| !message.trim().is_empty())
    {
        if !attempt
            .replies
            .iter()
            .any(|reply| reply.request_id == event.request_id)
        {
            attempt.replies.push(RepairThreadMessage {
                request_id: event.request_id.clone(),
                actor: event.actor,
                timestamp_ms: event.timestamp_ms,
                message: message.clone(),
            });
        }
    }
    if event.verification.is_some() {
        attempt.verification.clone_from(&event.verification);
    }
    if !finished
        && matches!(
            event.status,
            RepairStatus::NeedsInput
                | RepairStatus::ReviewReady
                | RepairStatus::VerificationFailed
                | RepairStatus::Resolved
                | RepairStatus::Dismissed
                | RepairStatus::Cancelled
                | RepairStatus::Failed
        )
    {
        attempt.finished_at_ms = Some(event.timestamp_ms);
    }
}

pub(super) fn repair_batch(id: String, records: &[&RepairRecord]) -> RepairBatch {
    let statuses = records
        .iter()
        .map(|record| record.status)
        .collect::<Vec<_>>();
    let status = if statuses
        .iter()
        .all(|status| *status == RepairStatus::Resolved)
    {
        RepairBatchStatus::Resolved
    } else if statuses.iter().any(|status| {
        matches!(
            status,
            RepairStatus::Failed | RepairStatus::VerificationFailed
        )
    }) && statuses.iter().all(|status| {
        matches!(
            status,
            RepairStatus::Resolved
                | RepairStatus::Dismissed
                | RepairStatus::Cancelled
                | RepairStatus::Failed
                | RepairStatus::VerificationFailed
                | RepairStatus::ReviewReady
        )
    }) {
        RepairBatchStatus::CompletedWithFailures
    } else if statuses.contains(&RepairStatus::NeedsInput) {
        RepairBatchStatus::NeedsInput
    } else if statuses
        .iter()
        .all(|status| *status == RepairStatus::ReviewReady)
    {
        RepairBatchStatus::ReviewReady
    } else if statuses
        .iter()
        .any(|status| *status != RepairStatus::Queued)
    {
        RepairBatchStatus::InProgress
    } else {
        RepairBatchStatus::Queued
    };
    RepairBatch {
        id,
        finding_ids: records
            .iter()
            .map(|record| record.finding.id.clone())
            .collect(),
        status,
        results: records
            .iter()
            .map(|record| RepairBatchItemResult {
                finding_id: record.finding.id.clone(),
                status: record.status,
            })
            .collect(),
    }
}

pub(super) fn status_event(event: &RepairEventRecord) -> RepairStatusEvent {
    RepairStatusEvent {
        request_id: event.request_id.clone(),
        finding_id: event.finding_id.clone(),
        sequence: event.sequence,
        status: event.status,
        actor: event.actor,
        timestamp: event.timestamp_ms.to_string(),
        summary: event.summary.clone(),
        message: event.message.clone(),
    }
}

pub(super) fn validate_idempotent_retry(
    event: &RepairEventRecord,
    request: &RepairTransition,
) -> Result<(), SessionError> {
    if event.session == request.session
        && event.finding_id == request.finding_id
        && event.status == request.status
        && event.actor == request.actor
        && event.attempt_id == request.attempt_id
        && event.lease_expires_at_ms == request.lease_expires_at_ms
        && event.summary == request.summary
        && event.message == request.message
        && event.verification == request.verification
    {
        return Ok(());
    }
    Err(SessionError::new(
        "test.session.repair_idempotency_conflict",
        format!(
            "repair request id '{}' was already used for a different transition",
            request.request_id
        ),
    ))
}

pub(super) fn validate_idempotent_human_action(
    event: &RepairEventRecord,
    session: &str,
    action: &RepairHumanAction,
    message: Option<&str>,
) -> Result<(), SessionError> {
    let expected = match action.action {
        RepairHumanActionKind::Reply => {
            event.status == RepairStatus::Queued
                && event.summary.as_deref() == Some("Human clarification received")
        }
        RepairHumanActionKind::Accept => {
            event.status == RepairStatus::Resolved
                && event.summary.as_deref() == Some("Human accepted the verified repair")
        }
        RepairHumanActionKind::Dismiss => {
            event.status == RepairStatus::Dismissed
                && event.summary.as_deref() == Some("Human rejected the verified repair")
        }
        RepairHumanActionKind::Reopen => {
            (event.status == RepairStatus::Queued
                && event.summary.as_deref() == Some("Human retried the failed verification"))
                || (event.status == RepairStatus::Reopened
                    && event.summary.as_deref() == Some("Human reopened the repair"))
        }
    };
    if event.session == session
        && event.finding_id == action.finding_id
        && event.actor == RepairActor::Human
        && event.message.as_deref() == message
        && expected
    {
        return Ok(());
    }
    Err(SessionError::new(
        "test.session.repair_idempotency_conflict",
        format!(
            "human repair request id '{}' was already used for a different action",
            action.request_id
        ),
    ))
}

pub(super) fn valid_transition(from: RepairStatus, to: RepairStatus) -> bool {
    use RepairStatus::*;
    matches!(
        (from, to),
        (Queued, Claimed | NeedsInput | Cancelled | Failed)
            | (
                Claimed,
                Queued | Repairing | Cancelled | NeedsInput | Failed
            )
            | (Repairing, Verifying | NeedsInput | Failed)
            | (
                Verifying,
                ReviewReady | VerificationFailed | NeedsInput | Failed
            )
            | (
                NeedsInput | VerificationFailed | Reopened,
                Queued | Cancelled | Failed
            )
            | (ReviewReady, Resolved | Reopened | Dismissed)
            | (Resolved | Dismissed | Cancelled | Failed, Reopened)
    )
}

pub(super) fn finding_conflict(left: &RepairFinding, right: &RepairFinding) -> bool {
    if left.id == right.id {
        return false;
    }
    let declared_conflict = left.relations.iter().any(|relation| {
        matches!(
            relation,
            a3s_test_core::RepairRelation::ConflictsWith { finding_id }
                if finding_id == &right.id
        )
    }) || right.relations.iter().any(|relation| {
        matches!(
            relation,
            a3s_test_core::RepairRelation::ConflictsWith { finding_id }
                if finding_id == &left.id
        )
    });
    if declared_conflict {
        return true;
    }
    if left.batch_id != right.batch_id {
        return false;
    }
    let shared_node = left
        .target
        .node_ids
        .iter()
        .any(|node_id| right.target.node_ids.contains(node_id));
    let overlapping_region = left
        .target
        .region
        .as_ref()
        .zip(right.target.region.as_ref())
        .is_some_and(|(left, right)| {
            left.x < right.x + right.width
                && left.x + left.width > right.x
                && left.y < right.y + right.height
                && left.y + left.height > right.y
        });
    let shared_source = source_hint(left)
        .zip(source_hint(right))
        .is_some_and(|(left, right)| left == right);
    shared_node || overlapping_region || shared_source
}

pub(super) fn source_hint(finding: &RepairFinding) -> Option<&str> {
    finding
        .context
        .get("component")?
        .get("source")?
        .get("file")?
        .as_str()
}

pub(super) fn validate_transition_request(
    record: &RepairRecord,
    request: &RepairTransition,
    now_ms: u64,
) -> Result<(), SessionError> {
    if let Some(attempt_id) = request.attempt_id.as_deref() {
        validate_component(attempt_id, "attempt id")?;
    }
    if request.status == RepairStatus::Claimed {
        if request.actor != RepairActor::Agent {
            return Err(SessionError::new(
                "test.session.repair_claim_invalid",
                "repair claims must be owned by the connected coding agent",
            ));
        }
        let attempt_id = request.attempt_id.as_deref().ok_or_else(|| {
            SessionError::new(
                "test.session.repair_claim_invalid",
                "repair claim requires an attempt id",
            )
        })?;
        validate_component(attempt_id, "attempt id")?;
        let expires_at = request.lease_expires_at_ms.ok_or_else(|| {
            SessionError::new(
                "test.session.repair_claim_invalid",
                "repair claim requires a lease expiry",
            )
        })?;
        if expires_at <= now_ms || expires_at.saturating_sub(now_ms) > MAX_CLAIM_LEASE_MS {
            return Err(SessionError::new(
                "test.session.repair_claim_invalid",
                "repair claim lease must expire within the next 15 minutes",
            ));
        }
        return Ok(());
    }
    if record.status == RepairStatus::Claimed
        && request.status == RepairStatus::Queued
        && request.actor != RepairActor::A3sTest
    {
        return Err(SessionError::new(
            "test.session.repair_transition_invalid",
            "only A3S Test lease recovery can return a claimed repair to the queue",
        ));
    }

    if matches!(
        record.status,
        RepairStatus::Claimed | RepairStatus::Repairing | RepairStatus::Verifying
    ) {
        if record
            .lease_expires_at_ms
            .is_some_and(|expires_at| expires_at <= now_ms)
        {
            return Err(SessionError::new(
                "test.session.repair_lease_expired",
                "repair lease expired; watch the queue to recover it before continuing",
            ));
        }
        let attempt_id = request.attempt_id.as_deref().ok_or_else(|| {
            SessionError::new(
                "test.session.repair_attempt_invalid",
                "active repair transition requires the claimed attempt id",
            )
        })?;
        if record.attempt_id.as_deref() != Some(attempt_id) {
            return Err(SessionError::new(
                "test.session.repair_attempt_invalid",
                "repair transition does not belong to the active attempt",
            ));
        }
    }
    if let Some(expires_at) = request.lease_expires_at_ms {
        if expires_at <= now_ms || expires_at.saturating_sub(now_ms) > MAX_CLAIM_LEASE_MS {
            return Err(SessionError::new(
                "test.session.repair_lease_invalid",
                "repair lease extension must expire within the next 15 minutes",
            ));
        }
    }
    Ok(())
}

pub(super) fn recovery_request_id(finding_id: &str, sequence: u64) -> String {
    let available = 128usize.saturating_sub(24 + sequence.to_string().len());
    let prefix = finding_id.chars().take(available).collect::<String>();
    format!("lease-recovery-{prefix}-{sequence}")
}

pub(super) fn conflict_request_id(finding_id: &str, sequence: u64) -> String {
    let available = 128usize.saturating_sub(18 + sequence.to_string().len());
    let prefix = finding_id.chars().take(available).collect::<String>();
    format!("conflict-{prefix}-{sequence}")
}

pub(super) fn interruption_request_id(finding_id: &str, sequence: u64) -> String {
    let available = 128usize.saturating_sub(28 + sequence.to_string().len());
    let prefix = finding_id.chars().take(available).collect::<String>();
    format!("session-interruption-{prefix}-{sequence}")
}

pub(super) fn followup_request_id(request_id: &str, suffix: &str) -> String {
    let available = 128usize.saturating_sub(suffix.len() + 1);
    let prefix = request_id.chars().take(available).collect::<String>();
    format!("{prefix}-{suffix}")
}

pub(super) fn validate_finding(finding: &RepairFinding) -> Result<(), SessionError> {
    validate_component(&finding.id, "finding id")?;
    validate_component(&finding.batch_id, "batch id")?;
    if finding.instruction.trim().is_empty() || finding.instruction.len() > 8_192 {
        return Err(SessionError::new(
            "test.session.repair_invalid",
            "repair instruction must contain 1-8192 bytes",
        ));
    }
    if finding.status != RepairStatus::Queued {
        return Err(SessionError::new(
            "test.session.repair_invalid",
            "submitted repair must be queued",
        ));
    }
    if let Some(reference) = finding.design_reference.as_ref() {
        validate_design_reference(reference)?;
    }
    validate_target(&finding.target)?;
    if finding.relations.len() > 100 {
        return Err(SessionError::new(
            "test.session.repair_invalid",
            "repair finding may declare at most 100 relations",
        ));
    }
    let mut related_ids = std::collections::HashSet::new();
    for relation in &finding.relations {
        let a3s_test_core::RepairRelation::ConflictsWith { finding_id } = relation;
        validate_component(finding_id, "related finding id")?;
        if finding_id == &finding.id || !related_ids.insert(finding_id) {
            return Err(SessionError::new(
                "test.session.repair_invalid",
                "repair conflict relations must reference distinct other findings",
            ));
        }
    }
    Ok(())
}

fn validate_design_reference(
    reference: &a3s_test_core::RepairDesignReference,
) -> Result<(), SessionError> {
    const MAX_INLINE_BYTES: usize = 384 * 1_024;
    let dimensions_valid = reference.width > 0
        && reference.width <= 1_600
        && reference.height > 0
        && reference.height <= 1_200
        && u64::from(reference.width) * u64::from(reference.height) <= 1_920_000;
    let image_valid = match &reference.image {
        a3s_test_core::RepairDesignReferenceImage::Inline {
            media_type,
            data_url,
        } => {
            let prefix = format!("data:{media_type};base64,");
            let encoded = data_url.strip_prefix(&prefix);
            matches!(media_type.as_str(), "image/png" | "image/jpeg")
                && data_url.len() <= MAX_INLINE_BYTES
                && encoded.is_some_and(|encoded| {
                    !encoded.is_empty()
                        && encoded.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
                        })
                })
        }
        a3s_test_core::RepairDesignReferenceImage::Artifact { evidence, sha256 } => {
            matches!(evidence.media_type.as_str(), "image/png" | "image/jpeg")
                && !evidence.name.is_empty()
                && evidence.name.len() <= 256
                && !evidence.path.is_empty()
                && evidence.path.len() <= 4_096
                && sha256.len() == 64
                && sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        }
    };
    if dimensions_valid && image_valid {
        Ok(())
    } else {
        Err(SessionError::new(
            "test.session.repair_invalid",
            "repair design reference is unbounded or invalid",
        ))
    }
}

fn validate_target(target: &a3s_test_core::RepairTarget) -> Result<(), SessionError> {
    if target.node_ids.len() > 5_000
        || target
            .node_ids
            .iter()
            .any(|node_id| node_id.is_empty() || node_id.len() > 128)
        || target
            .selected_text
            .as_ref()
            .is_some_and(|text| text.len() > 4_096)
        || target
            .region
            .as_ref()
            .is_some_and(|region| !valid_rect(region))
        || target.drawing.as_ref().is_some_and(|drawing| {
            drawing.len() > 2_000
                || drawing
                    .iter()
                    .any(|point| !point.x.is_finite() || !point.y.is_finite())
        })
    {
        return Err(SessionError::new(
            "test.session.repair_invalid",
            "repair target contains unbounded or invalid geometry",
        ));
    }
    let Some(layout) = target.layout.as_ref() else {
        return Ok(());
    };
    let purpose_valid = match layout {
        a3s_test_core::RepairLayoutIntent::Placement { purpose, .. }
        | a3s_test_core::RepairLayoutIntent::Rearrange { purpose, .. } => purpose
            .as_ref()
            .is_none_or(|purpose| purpose.len() <= 2_048),
    };
    let shape_valid = match layout {
        a3s_test_core::RepairLayoutIntent::Placement { component_type, .. } => {
            target.kind == a3s_test_core::RepairTargetKind::Region
                && target.region.is_some()
                && !component_type.trim().is_empty()
                && component_type.len() <= 128
        }
        a3s_test_core::RepairLayoutIntent::Rearrange {
            original_region, ..
        } => {
            target.kind == a3s_test_core::RepairTargetKind::Node
                && !target.node_ids.is_empty()
                && target.region.is_some()
                && valid_rect(original_region)
        }
    };
    if !purpose_valid || !shape_valid {
        return Err(SessionError::new(
            "test.session.repair_invalid",
            "repair layout intent does not match its bounded target geometry",
        ));
    }
    Ok(())
}

fn valid_rect(rect: &a3s_test_core::PageContextRect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width > 0.0
        && rect.height > 0.0
}

pub(super) fn validate_evidence(
    finding: &RepairFinding,
    evidence: &RepairEvidenceBundle,
) -> Result<(), SessionError> {
    if evidence.context_revision < finding.context_revision
        || evidence.context.revision != Some(evidence.context_revision)
        || !evidence
            .context
            .page
            .as_ref()
            .is_some_and(|page| page.ready)
        || evidence.context_sha256.len() != 64
        || !evidence
            .context_sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        || evidence.screenshot_sha256.len() != 64
        || !evidence
            .screenshot_sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        || evidence.screenshot.path.is_empty()
        || evidence.screenshot.path.len() > 1_024
    {
        return Err(SessionError::new(
            "test.session.repair_evidence_invalid",
            "repair evidence must contain a ready context no older than submission, SHA-256 digests, and a bounded screenshot",
        ));
    }
    Ok(())
}

pub(super) fn validate_component(value: &str, field: &str) -> Result<(), SessionError> {
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(SessionError::new(
            "test.session.repair_invalid",
            format!("{field} contains unsupported characters"),
        ));
    }
    Ok(())
}

pub(super) fn not_found(finding_id: &str) -> SessionError {
    SessionError::new(
        "test.session.repair_not_found",
        format!("repair finding '{finding_id}' does not exist"),
    )
}

pub(super) fn storage_error(path: &Path, error: std::io::Error) -> SessionError {
    SessionError::new(
        "test.session.repair_storage_failed",
        format!("failed to access {}: {error}", path.display()),
    )
}
