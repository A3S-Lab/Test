use std::collections::HashMap;
use std::path::{Path, PathBuf};

use a3s_test_core::{
    RepairActor, RepairAttempt, RepairBatch, RepairBatchItemResult, RepairBatchStatus,
    RepairEvidenceBundle, RepairFinding, RepairHumanAction, RepairHumanActionKind, RepairStatus,
    RepairStatusEvent, RepairThreadMessage, RepairVerification,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::RepairWorkspaceLock;
use crate::SessionError;

const MAX_CLAIM_LEASE_MS: u64 = 15 * 60 * 1_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairRecord {
    pub finding: RepairFinding,
    pub status: RepairStatus,
    pub sequence: u64,
    pub attempt_id: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub updated_at_ms: u64,
    pub summary: Option<String>,
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<RepairVerification>,
    #[serde(default)]
    pub attempts: Vec<RepairAttempt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_evidence: Option<RepairEvidenceBundle>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairEventRecord {
    pub session: String,
    pub finding_id: String,
    pub request_id: String,
    pub sequence: u64,
    pub status: RepairStatus,
    pub actor: RepairActor,
    pub timestamp_ms: u64,
    pub attempt_id: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub summary: Option<String>,
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<RepairVerification>,
}

pub struct RepairLedger {
    path: PathBuf,
    session: Option<String>,
    records: HashMap<String, RepairRecord>,
    order: Vec<String>,
    request_events: HashMap<String, RepairEventRecord>,
}

impl RepairLedger {
    pub(crate) fn empty(path: PathBuf) -> Self {
        Self {
            path,
            session: None,
            records: HashMap::new(),
            order: Vec::new(),
            request_events: HashMap::new(),
        }
    }

    pub async fn load(path: PathBuf) -> Result<Self, SessionError> {
        let mut ledger = Self::empty(path.clone());
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(ledger),
            Err(error) => return Err(storage_error(&path, error)),
        };
        for (line_number, line) in contents.lines().enumerate() {
            let event: StoredLedgerEvent = serde_json::from_str(line).map_err(|error| {
                SessionError::new(
                    "test.session.repair_ledger_invalid",
                    format!(
                        "invalid repair ledger {} line {}: {error}",
                        path.display(),
                        line_number + 1
                    ),
                )
            })?;
            ledger.replay(event)?;
        }
        Ok(ledger)
    }

    pub async fn reload(&mut self) -> Result<(), SessionError> {
        *self = Self::load(self.path.clone()).await?;
        Ok(())
    }

    pub fn require_attempt_state(
        &self,
        finding_id: &str,
        expected_status: RepairStatus,
        expected_attempt_id: &str,
    ) -> Result<RepairRecord, SessionError> {
        let record = self
            .records
            .get(finding_id)
            .ok_or_else(|| not_found(finding_id))?;
        if record.status != expected_status
            || record.attempt_id.as_deref() != Some(expected_attempt_id)
        {
            return Err(SessionError::new(
                "test.session.repair_state_changed",
                format!(
                    "repair '{finding_id}' changed while work was in progress; discard the stale result and observe the current repair state"
                ),
            )
            .with_retryable(true));
        }
        Ok(record.clone())
    }

    pub async fn ingest(
        &mut self,
        session: &str,
        findings: Vec<RepairFinding>,
        now_ms: u64,
    ) -> Result<Vec<RepairRecord>, SessionError> {
        self.admit_session(session)?;
        let mut created = Vec::new();
        for finding in findings {
            validate_finding(&finding)?;
            if let Some(existing) = self.records.get(&finding.id) {
                if existing.finding != finding {
                    return Err(SessionError::new(
                        "test.session.repair_conflict",
                        format!(
                            "repair finding '{}' was resubmitted with different content",
                            finding.id
                        ),
                    ));
                }
                continue;
            }
            let event = StoredLedgerEvent::Submitted {
                session: session.to_string(),
                finding: finding.clone(),
                timestamp_ms: now_ms,
            };
            self.append(&event).await?;
            let record = RepairRecord {
                finding: finding.clone(),
                status: RepairStatus::Queued,
                sequence: 0,
                attempt_id: None,
                lease_expires_at_ms: None,
                updated_at_ms: now_ms,
                summary: None,
                message: None,
                verification: None,
                attempts: Vec::new(),
                before_evidence: None,
            };
            self.order.push(finding.id.clone());
            self.records.insert(finding.id.clone(), record.clone());
            created.push(record);
        }
        Ok(created)
    }

    pub async fn ingest_in_workspace(
        &mut self,
        session: &str,
        findings: Vec<RepairFinding>,
        now_ms: u64,
        _workspace: &mut RepairWorkspaceLock,
    ) -> Result<Vec<RepairRecord>, SessionError> {
        self.reload().await?;
        self.ingest(session, findings, now_ms).await
    }

    pub fn queued(&self, limit: usize) -> Vec<RepairRecord> {
        self.order
            .iter()
            .filter_map(|id| self.records.get(id))
            .filter(|record| record.status == RepairStatus::Queued)
            .take(limit.clamp(1, 50))
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn get(&self, finding_id: &str) -> Option<RepairRecord> {
        self.records.get(finding_id).cloned()
    }

    #[must_use]
    pub fn batches(&self) -> Vec<RepairBatch> {
        let mut ids = Vec::<String>::new();
        let mut grouped = HashMap::<String, Vec<&RepairRecord>>::new();
        for finding_id in &self.order {
            let Some(record) = self.records.get(finding_id) else {
                continue;
            };
            if !grouped.contains_key(&record.finding.batch_id) {
                ids.push(record.finding.batch_id.clone());
            }
            grouped
                .entry(record.finding.batch_id.clone())
                .or_default()
                .push(record);
        }
        ids.into_iter()
            .filter_map(|id| {
                grouped
                    .remove(&id)
                    .map(|records| repair_batch(id, &records))
            })
            .collect()
    }

    pub async fn attach_before_evidence(
        &mut self,
        session: &str,
        finding_id: &str,
        evidence: RepairEvidenceBundle,
    ) -> Result<RepairRecord, SessionError> {
        self.admit_session(session)?;
        let record = self
            .records
            .get(finding_id)
            .ok_or_else(|| not_found(finding_id))?;
        if record.status != RepairStatus::Queued {
            return Err(SessionError::new(
                "test.session.repair_evidence_invalid",
                "before evidence can only be attached while a repair is queued",
            ));
        }
        validate_evidence(&record.finding, &evidence)?;
        if let Some(existing) = record.before_evidence.as_ref() {
            return if existing == &evidence {
                Ok(record.clone())
            } else {
                Err(SessionError::new(
                    "test.session.repair_evidence_conflict",
                    "repair before evidence was already captured with different content",
                ))
            };
        }
        let event = StoredLedgerEvent::BeforeEvidence {
            session: session.to_string(),
            finding_id: finding_id.to_string(),
            evidence: evidence.clone(),
        };
        self.append(&event).await?;
        let record = self
            .records
            .get_mut(finding_id)
            .expect("validated repair remains present");
        record.before_evidence = Some(evidence);
        Ok(record.clone())
    }

    pub async fn attach_before_evidence_in_workspace(
        &mut self,
        session: &str,
        finding_id: &str,
        evidence: RepairEvidenceBundle,
        _workspace: &mut RepairWorkspaceLock,
    ) -> Result<RepairRecord, SessionError> {
        self.reload().await?;
        self.attach_before_evidence(session, finding_id, evidence)
            .await
    }

    #[must_use]
    pub fn current_events(&self) -> Vec<RepairStatusEvent> {
        self.order
            .iter()
            .filter_map(|finding_id| {
                self.request_events
                    .values()
                    .filter(|event| &event.finding_id == finding_id)
                    .max_by_key(|event| event.sequence)
                    .map(status_event)
            })
            .collect()
    }

    pub async fn transition(
        &mut self,
        request: RepairTransition,
        now_ms: u64,
    ) -> Result<(RepairRecord, RepairStatusEvent), SessionError> {
        validate_component(&request.session, "session id")?;
        validate_component(&request.finding_id, "finding id")?;
        validate_component(&request.request_id, "request id")?;
        self.admit_session(&request.session)?;
        if let Some(event) = self.request_events.get(&request.request_id) {
            validate_idempotent_retry(event, &request)?;
            let record = self
                .records
                .get(&request.finding_id)
                .cloned()
                .ok_or_else(|| not_found(&request.finding_id))?;
            return Ok((record, status_event(event)));
        }
        let record = self
            .records
            .get(&request.finding_id)
            .ok_or_else(|| not_found(&request.finding_id))?;
        if request.status == RepairStatus::Claimed {
            if record.before_evidence.is_none() {
                return Err(SessionError::new(
                    "test.session.repair_evidence_missing",
                    "repair cannot be claimed until A3S Test captures before evidence",
                )
                .with_retryable(true));
            }
            if let Some(active) = self.records.values().find(|candidate| {
                candidate.finding.id != request.finding_id
                    && matches!(
                        candidate.status,
                        RepairStatus::Claimed | RepairStatus::Repairing | RepairStatus::Verifying
                    )
            }) {
                return Err(SessionError::new(
                    "test.session.repair_workspace_busy",
                    format!(
                        "repair '{}' owns the workspace mutation slot until its verification finishes",
                        active.finding.id
                    ),
                )
                .with_retryable(true));
            }
        }
        if !valid_transition(record.status, request.status) {
            return Err(SessionError::new(
                "test.session.repair_transition_invalid",
                format!(
                    "repair '{}' cannot transition from {:?} to {:?}",
                    request.finding_id, record.status, request.status
                ),
            ));
        }
        validate_transition_request(record, &request, now_ms)?;
        let sequence = record.sequence.checked_add(1).ok_or_else(|| {
            SessionError::new(
                "test.session.repair_sequence_exhausted",
                "repair event sequence overflowed",
            )
        })?;
        let event = RepairEventRecord {
            session: request.session.clone(),
            finding_id: request.finding_id.clone(),
            request_id: request.request_id.clone(),
            sequence,
            status: request.status,
            actor: request.actor,
            timestamp_ms: now_ms,
            attempt_id: request.attempt_id.clone(),
            lease_expires_at_ms: request.lease_expires_at_ms,
            summary: request.summary.clone(),
            message: request.message.clone(),
            verification: request.verification.clone(),
        };
        self.append(&StoredLedgerEvent::Transition {
            event: Box::new(event.clone()),
        })
        .await?;
        self.request_events
            .insert(request.request_id.clone(), event.clone());
        let record = self
            .records
            .get_mut(&request.finding_id)
            .expect("validated record");
        apply_event(record, &event);
        let record = record.clone();
        Ok((record, status_event(&event)))
    }

    pub async fn transition_in_workspace(
        &mut self,
        request: RepairTransition,
        now_ms: u64,
        workspace: &mut RepairWorkspaceLock,
    ) -> Result<(RepairRecord, RepairStatusEvent), SessionError> {
        self.reload().await?;
        if self.request_events.contains_key(&request.request_id) {
            let result = self.transition(request.clone(), now_ms).await?;
            workspace
                .reconcile_record(&request.session, &result.0, now_ms)
                .await?;
            return Ok(result);
        }
        let current = self
            .records
            .get(&request.finding_id)
            .cloned()
            .ok_or_else(|| not_found(&request.finding_id))?;
        let previous_owner = workspace
            .prepare_transition(&current, &request, now_ms)
            .await?;
        let previous_status = current.status;
        match self.transition(request.clone(), now_ms).await {
            Ok(result) => {
                workspace
                    .finish_transition(previous_status, &result.0, &request.session, now_ms)
                    .await?;
                Ok(result)
            }
            Err(error) => {
                if matches!(
                    request.status,
                    RepairStatus::Claimed | RepairStatus::Repairing | RepairStatus::Verifying
                ) {
                    workspace.rollback(previous_owner).await?;
                }
                Err(error)
            }
        }
    }

    pub async fn recover_expired_leases(
        &mut self,
        session: &str,
        now_ms: u64,
    ) -> Result<Vec<(RepairRecord, RepairStatusEvent)>, SessionError> {
        self.admit_session(session)?;
        let expired = self
            .order
            .iter()
            .filter_map(|id| self.records.get(id))
            .filter(|record| {
                matches!(
                    record.status,
                    RepairStatus::Claimed | RepairStatus::Repairing | RepairStatus::Verifying
                ) && record
                    .lease_expires_at_ms
                    .is_some_and(|expires_at| expires_at <= now_ms)
            })
            .map(|record| {
                (
                    record.finding.id.clone(),
                    record.status,
                    record.sequence,
                    record.attempt_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        let mut recovered = Vec::with_capacity(expired.len());
        for (finding_id, current, sequence, attempt_id) in expired {
            let status = if current == RepairStatus::Claimed {
                RepairStatus::Queued
            } else {
                RepairStatus::NeedsInput
            };
            let event = RepairEventRecord {
                session: session.to_string(),
                finding_id: finding_id.clone(),
                request_id: recovery_request_id(&finding_id, sequence.saturating_add(1)),
                sequence: sequence.checked_add(1).ok_or_else(|| {
                    SessionError::new(
                        "test.session.repair_sequence_exhausted",
                        "repair event sequence overflowed",
                    )
                })?,
                status,
                actor: RepairActor::A3sTest,
                timestamp_ms: now_ms,
                attempt_id,
                lease_expires_at_ms: None,
                summary: Some(if status == RepairStatus::Queued {
                    "Expired pre-edit claim returned to the queue".to_string()
                } else {
                    "Editing may have occurred before the lease expired".to_string()
                }),
                message: (status == RepairStatus::NeedsInput).then(|| {
                    "Review the possibly mutated workspace before assigning another attempt"
                        .to_string()
                }),
                verification: None,
            };
            self.append(&StoredLedgerEvent::Transition {
                event: Box::new(event.clone()),
            })
            .await?;
            self.request_events
                .insert(event.request_id.clone(), event.clone());
            let record = self
                .records
                .get_mut(&finding_id)
                .expect("expired record remains present");
            apply_event(record, &event);
            recovered.push((record.clone(), status_event(&event)));
        }
        Ok(recovered)
    }

    pub async fn recover_expired_leases_in_workspace(
        &mut self,
        session: &str,
        now_ms: u64,
        workspace: &mut RepairWorkspaceLock,
    ) -> Result<Vec<(RepairRecord, RepairStatusEvent)>, SessionError> {
        self.reload().await?;
        let recovered = self.recover_expired_leases(session, now_ms).await?;
        for (record, _) in &recovered {
            workspace.reconcile_record(session, record, now_ms).await?;
        }
        Ok(recovered)
    }

    pub async fn interrupt_active_mutation_in_workspace(
        &mut self,
        session: &str,
        now_ms: u64,
        workspace: &mut RepairWorkspaceLock,
    ) -> Result<Vec<(RepairRecord, RepairStatusEvent)>, SessionError> {
        self.admit_session(session)?;
        let recovered = self
            .recover_expired_leases_in_workspace(session, now_ms, workspace)
            .await?;
        if !recovered.is_empty() {
            return Ok(recovered);
        }
        let active = self
            .order
            .iter()
            .filter_map(|id| self.records.get(id))
            .find(|record| {
                matches!(
                    record.status,
                    RepairStatus::Claimed | RepairStatus::Repairing | RepairStatus::Verifying
                )
            })
            .cloned();
        let Some(active) = active else {
            return Ok(Vec::new());
        };
        let pre_edit = active.status == RepairStatus::Claimed;
        let status = if pre_edit {
            RepairStatus::Queued
        } else {
            RepairStatus::NeedsInput
        };
        let request_id =
            interruption_request_id(&active.finding.id, active.sequence.saturating_add(1));
        let transition = RepairTransition {
            session: session.to_string(),
            finding_id: active.finding.id.clone(),
            request_id,
            status,
            actor: RepairActor::A3sTest,
            attempt_id: active.attempt_id.clone(),
            lease_expires_at_ms: None,
            summary: Some(if pre_edit {
                "Session closed before workspace editing began".to_string()
            } else {
                "Session closed after workspace editing may have occurred".to_string()
            }),
            message: (!pre_edit).then(|| {
                "Review the possibly mutated workspace before assigning another attempt".to_string()
            }),
            verification: None,
        };
        let result = self
            .transition_in_workspace(transition, now_ms, workspace)
            .await?;
        Ok(vec![result])
    }

    pub async fn resolve_conflicts(
        &mut self,
        session: &str,
        now_ms: u64,
    ) -> Result<Vec<(RepairRecord, RepairStatusEvent)>, SessionError> {
        self.admit_session(session)?;
        let queued = self
            .order
            .iter()
            .filter_map(|id| self.records.get(id))
            .filter(|record| record.status == RepairStatus::Queued)
            .map(|record| record.finding.clone())
            .collect::<Vec<_>>();
        let mut conflicts = std::collections::HashSet::new();
        for (index, left) in queued.iter().enumerate() {
            for right in queued.iter().skip(index + 1) {
                if finding_conflict(left, right) {
                    conflicts.insert(left.id.clone());
                    conflicts.insert(right.id.clone());
                }
            }
        }
        let ordered = self
            .order
            .iter()
            .filter(|id| conflicts.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        let mut resolved = Vec::with_capacity(ordered.len());
        for finding_id in ordered {
            let sequence = self
                .records
                .get(&finding_id)
                .map_or(1, |record| record.sequence.saturating_add(1));
            resolved.push(
                self.transition(
                    RepairTransition {
                        session: session.to_string(),
                        finding_id: finding_id.clone(),
                        request_id: conflict_request_id(&finding_id, sequence),
                        status: RepairStatus::NeedsInput,
                        actor: RepairActor::A3sTest,
                        attempt_id: None,
                        lease_expires_at_ms: None,
                        summary: Some("Repair conflicts with another queued finding".to_string()),
                        message: Some(
                            "Clarify the order or combine overlapping targets before editing"
                                .to_string(),
                        ),
                        verification: None,
                    },
                    now_ms,
                )
                .await?,
            );
        }
        Ok(resolved)
    }

    pub async fn resolve_conflicts_in_workspace(
        &mut self,
        session: &str,
        now_ms: u64,
        _workspace: &mut RepairWorkspaceLock,
    ) -> Result<Vec<(RepairRecord, RepairStatusEvent)>, SessionError> {
        self.reload().await?;
        self.resolve_conflicts(session, now_ms).await
    }

    pub async fn apply_human_action(
        &mut self,
        session: &str,
        action: RepairHumanAction,
        now_ms: u64,
    ) -> Result<Vec<(RepairRecord, RepairStatusEvent)>, SessionError> {
        self.admit_session(session)?;
        validate_component(&action.request_id, "request id")?;
        validate_component(&action.finding_id, "finding id")?;
        if action.timestamp.trim().is_empty() || action.timestamp.len() > 128 {
            return Err(SessionError::new(
                "test.session.repair_human_action_invalid",
                "human repair action timestamp must be bounded and non-empty",
            ));
        }
        let message = action
            .message
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if action
            .message
            .as_ref()
            .is_some_and(|value| value.len() > 8_192)
        {
            return Err(SessionError::new(
                "test.session.repair_human_action_invalid",
                "human repair action message exceeds 8192 bytes",
            ));
        }
        if let Some(event) = self.request_events.get(&action.request_id) {
            validate_idempotent_human_action(event, session, &action, message.as_deref())?;
            let record = self
                .records
                .get(&action.finding_id)
                .cloned()
                .ok_or_else(|| not_found(&action.finding_id))?;
            let mut results = vec![(record.clone(), status_event(event))];
            if event.status == RepairStatus::Reopened {
                let followup_id = followup_request_id(&action.request_id, "queued");
                let followup = self.request_events.get(&followup_id).ok_or_else(|| {
                    SessionError::new(
                        "test.session.repair_ledger_invalid",
                        "replayed human reopen action is missing its queued follow-up",
                    )
                })?;
                results.push((record, status_event(followup)));
            }
            return Ok(results);
        }
        let current = self
            .records
            .get(&action.finding_id)
            .cloned()
            .ok_or_else(|| not_found(&action.finding_id))?;
        let mut transitions = Vec::new();
        match action.action {
            RepairHumanActionKind::Reply => {
                if current.status != RepairStatus::NeedsInput || message.is_none() {
                    return Err(SessionError::new(
                        "test.session.repair_human_action_invalid",
                        "human replies require a non-empty message for a needs_input repair",
                    ));
                }
                transitions.push(RepairTransition {
                    session: session.to_string(),
                    finding_id: action.finding_id,
                    request_id: action.request_id,
                    status: RepairStatus::Queued,
                    actor: RepairActor::Human,
                    attempt_id: current.attempt_id,
                    lease_expires_at_ms: None,
                    summary: Some("Human clarification received".to_string()),
                    message,
                    verification: None,
                });
            }
            RepairHumanActionKind::Accept | RepairHumanActionKind::Dismiss => {
                if current.status != RepairStatus::ReviewReady {
                    return Err(SessionError::new(
                        "test.session.repair_human_action_invalid",
                        "accept and dismiss actions require a review_ready repair",
                    ));
                }
                transitions.push(RepairTransition {
                    session: session.to_string(),
                    finding_id: action.finding_id,
                    request_id: action.request_id,
                    status: if action.action == RepairHumanActionKind::Accept {
                        RepairStatus::Resolved
                    } else {
                        RepairStatus::Dismissed
                    },
                    actor: RepairActor::Human,
                    attempt_id: current.attempt_id,
                    lease_expires_at_ms: None,
                    summary: Some(if action.action == RepairHumanActionKind::Accept {
                        "Human accepted the verified repair".to_string()
                    } else {
                        "Human rejected the verified repair".to_string()
                    }),
                    message,
                    verification: None,
                });
            }
            RepairHumanActionKind::Reopen => {
                if current.status == RepairStatus::VerificationFailed {
                    transitions.push(RepairTransition {
                        session: session.to_string(),
                        finding_id: action.finding_id,
                        request_id: action.request_id,
                        status: RepairStatus::Queued,
                        actor: RepairActor::Human,
                        attempt_id: current.attempt_id,
                        lease_expires_at_ms: None,
                        summary: Some("Human retried the failed verification".to_string()),
                        message,
                        verification: None,
                    });
                } else {
                    if !matches!(
                        current.status,
                        RepairStatus::ReviewReady
                            | RepairStatus::Resolved
                            | RepairStatus::Dismissed
                            | RepairStatus::Cancelled
                            | RepairStatus::Failed
                    ) {
                        return Err(SessionError::new(
                            "test.session.repair_human_action_invalid",
                            "this repair state cannot be reopened",
                        ));
                    }
                    transitions.push(RepairTransition {
                        session: session.to_string(),
                        finding_id: action.finding_id.clone(),
                        request_id: action.request_id.clone(),
                        status: RepairStatus::Reopened,
                        actor: RepairActor::Human,
                        attempt_id: current.attempt_id.clone(),
                        lease_expires_at_ms: None,
                        summary: Some("Human reopened the repair".to_string()),
                        message: message.clone(),
                        verification: None,
                    });
                    transitions.push(RepairTransition {
                        session: session.to_string(),
                        finding_id: action.finding_id,
                        request_id: followup_request_id(&action.request_id, "queued"),
                        status: RepairStatus::Queued,
                        actor: RepairActor::Human,
                        attempt_id: current.attempt_id,
                        lease_expires_at_ms: None,
                        summary: Some("Reopened repair returned to the queue".to_string()),
                        message: None,
                        verification: None,
                    });
                }
            }
        }
        let mut results = Vec::with_capacity(transitions.len());
        for transition in transitions {
            results.push(self.transition(transition, now_ms).await?);
        }
        Ok(results)
    }

    pub async fn apply_human_action_in_workspace(
        &mut self,
        session: &str,
        action: RepairHumanAction,
        now_ms: u64,
        _workspace: &mut RepairWorkspaceLock,
    ) -> Result<Vec<(RepairRecord, RepairStatusEvent)>, SessionError> {
        self.reload().await?;
        self.apply_human_action(session, action, now_ms).await
    }

    async fn append(&self, event: &StoredLedgerEvent) -> Result<(), SessionError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| storage_error(parent, error))?;
        }
        let mut encoded = serde_json::to_vec(event).map_err(|error| {
            SessionError::new(
                "test.session.repair_ledger_invalid",
                format!("failed to encode repair event: {error}"),
            )
        })?;
        encoded.push(b'\n');
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(|error| storage_error(&self.path, error))?;
        file.write_all(&encoded)
            .await
            .map_err(|error| storage_error(&self.path, error))?;
        file.flush()
            .await
            .map_err(|error| storage_error(&self.path, error))
    }

    fn replay(&mut self, event: StoredLedgerEvent) -> Result<(), SessionError> {
        match event {
            StoredLedgerEvent::Submitted {
                session,
                finding,
                timestamp_ms,
            } => {
                self.admit_session(&session)?;
                validate_finding(&finding).map_err(|error| {
                    SessionError::new(
                        "test.session.repair_ledger_invalid",
                        format!("invalid submitted repair in ledger: {}", error.message()),
                    )
                })?;
                if self.records.contains_key(&finding.id) {
                    return Err(SessionError::new(
                        "test.session.repair_ledger_invalid",
                        format!("duplicate submitted repair '{}'", finding.id),
                    ));
                }
                self.order.push(finding.id.clone());
                self.records.insert(
                    finding.id.clone(),
                    RepairRecord {
                        finding,
                        status: RepairStatus::Queued,
                        sequence: 0,
                        attempt_id: None,
                        lease_expires_at_ms: None,
                        updated_at_ms: timestamp_ms,
                        summary: None,
                        message: None,
                        verification: None,
                        attempts: Vec::new(),
                        before_evidence: None,
                    },
                );
            }
            StoredLedgerEvent::Transition { event } => {
                let event = *event;
                self.admit_session(&event.session)?;
                if self.request_events.contains_key(&event.request_id) {
                    return Err(SessionError::new(
                        "test.session.repair_ledger_invalid",
                        format!("duplicate repair request id '{}'", event.request_id),
                    ));
                }
                let record = self
                    .records
                    .get_mut(&event.finding_id)
                    .ok_or_else(|| not_found(&event.finding_id))?;
                if event.sequence != record.sequence + 1
                    || !valid_transition(record.status, event.status)
                {
                    return Err(SessionError::new(
                        "test.session.repair_ledger_invalid",
                        format!("invalid transition for repair '{}'", event.finding_id),
                    ));
                }
                apply_event(record, &event);
                self.request_events.insert(event.request_id.clone(), event);
            }
            StoredLedgerEvent::BeforeEvidence {
                session,
                finding_id,
                evidence,
            } => {
                self.admit_session(&session)?;
                let record = self
                    .records
                    .get_mut(&finding_id)
                    .ok_or_else(|| not_found(&finding_id))?;
                validate_evidence(&record.finding, &evidence).map_err(|error| {
                    SessionError::new(
                        "test.session.repair_ledger_invalid",
                        format!("invalid repair evidence: {}", error.message()),
                    )
                })?;
                if let Some(existing) = record.before_evidence.as_ref() {
                    if existing != &evidence {
                        return Err(SessionError::new(
                            "test.session.repair_ledger_invalid",
                            format!("conflicting before evidence for repair '{finding_id}'"),
                        ));
                    }
                } else {
                    record.before_evidence = Some(evidence);
                }
            }
        }
        Ok(())
    }

    fn admit_session(&mut self, session: &str) -> Result<(), SessionError> {
        validate_component(session, "session id")?;
        match self.session.as_deref() {
            Some(existing) if existing != session => Err(SessionError::new(
                "test.session.repair_session_mismatch",
                format!("repair ledger belongs to session '{existing}', not '{session}'"),
            )),
            Some(_) => Ok(()),
            None => {
                self.session = Some(session.to_string());
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepairTransition {
    pub session: String,
    pub finding_id: String,
    pub request_id: String,
    pub status: RepairStatus,
    pub actor: RepairActor,
    pub attempt_id: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub verification: Option<RepairVerification>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredLedgerEvent {
    Submitted {
        session: String,
        finding: RepairFinding,
        timestamp_ms: u64,
    },
    Transition {
        event: Box<RepairEventRecord>,
    },
    BeforeEvidence {
        session: String,
        finding_id: String,
        evidence: RepairEvidenceBundle,
    },
}

#[path = "repair_state.rs"]
mod state;
use state::*;

#[cfg(test)]
#[path = "repair_tests.rs"]
mod tests;
