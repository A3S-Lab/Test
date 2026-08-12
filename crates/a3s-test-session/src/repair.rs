use std::collections::HashMap;
use std::path::{Path, PathBuf};

use a3s_test_core::{
    RepairActor, RepairAttempt, RepairBatch, RepairBatchItemResult, RepairBatchStatus,
    RepairEvidenceBundle, RepairFinding, RepairHumanAction, RepairHumanActionKind, RepairStatus,
    RepairStatusEvent, RepairThreadMessage, RepairVerification,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

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
            event: event.clone(),
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
                    RepairStatus::Claimed | RepairStatus::Repairing
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
                event: event.clone(),
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
        event: RepairEventRecord,
    },
    BeforeEvidence {
        session: String,
        finding_id: String,
        evidence: RepairEvidenceBundle,
    },
}

fn apply_event(record: &mut RepairRecord, event: &RepairEventRecord) {
    record.status = event.status;
    record.sequence = event.sequence;
    if event.attempt_id.is_some() {
        record.attempt_id.clone_from(&event.attempt_id);
    }
    if event.lease_expires_at_ms.is_some() {
        record.lease_expires_at_ms = event.lease_expires_at_ms;
    } else if !matches!(
        event.status,
        RepairStatus::Claimed | RepairStatus::Repairing
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

fn update_attempts(record: &mut RepairRecord, event: &RepairEventRecord) {
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

fn repair_batch(id: String, records: &[&RepairRecord]) -> RepairBatch {
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
    } else if statuses
        .iter()
        .any(|status| *status == RepairStatus::NeedsInput)
    {
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

fn status_event(event: &RepairEventRecord) -> RepairStatusEvent {
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

fn validate_idempotent_retry(
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

fn validate_idempotent_human_action(
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

fn valid_transition(from: RepairStatus, to: RepairStatus) -> bool {
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

fn finding_conflict(left: &RepairFinding, right: &RepairFinding) -> bool {
    if left.batch_id != right.batch_id || left.id == right.id {
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

fn source_hint(finding: &RepairFinding) -> Option<&str> {
    finding
        .context
        .get("component")?
        .get("source")?
        .get("file")?
        .as_str()
}

fn validate_transition_request(
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
        RepairStatus::Claimed | RepairStatus::Repairing
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
        if request.actor == RepairActor::Agent {
            let attempt_id = request.attempt_id.as_deref().ok_or_else(|| {
                SessionError::new(
                    "test.session.repair_attempt_invalid",
                    "agent repair transition requires the claimed attempt id",
                )
            })?;
            if record.attempt_id.as_deref() != Some(attempt_id) {
                return Err(SessionError::new(
                    "test.session.repair_attempt_invalid",
                    "repair transition does not belong to the active attempt",
                ));
            }
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

fn recovery_request_id(finding_id: &str, sequence: u64) -> String {
    let available = 128usize.saturating_sub(24 + sequence.to_string().len());
    let prefix = finding_id.chars().take(available).collect::<String>();
    format!("lease-recovery-{prefix}-{sequence}")
}

fn conflict_request_id(finding_id: &str, sequence: u64) -> String {
    let available = 128usize.saturating_sub(18 + sequence.to_string().len());
    let prefix = finding_id.chars().take(available).collect::<String>();
    format!("conflict-{prefix}-{sequence}")
}

fn followup_request_id(request_id: &str, suffix: &str) -> String {
    let available = 128usize.saturating_sub(suffix.len() + 1);
    let prefix = request_id.chars().take(available).collect::<String>();
    format!("{prefix}-{suffix}")
}

fn validate_finding(finding: &RepairFinding) -> Result<(), SessionError> {
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
    Ok(())
}

fn validate_evidence(
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

fn validate_component(value: &str, field: &str) -> Result<(), SessionError> {
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

fn not_found(finding_id: &str) -> SessionError {
    SessionError::new(
        "test.session.repair_not_found",
        format!("repair finding '{finding_id}' does not exist"),
    )
}

fn storage_error(path: &Path, error: std::io::Error) -> SessionError {
    SessionError::new(
        "test.session.repair_storage_failed",
        format!("failed to access {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_test_core::{
        Evidence, PageContextSnapshot, RepairIntent, RepairSeverity, RepairTarget, RepairTargetKind,
    };
    use serde_json::json;

    fn finding(id: &str) -> RepairFinding {
        RepairFinding {
            id: id.to_string(),
            batch_id: "batch-1".to_string(),
            instruction: format!("Fix {id}"),
            success_criteria: Some("The issue is visibly gone".to_string()),
            intent: RepairIntent::Fix,
            severity: RepairSeverity::Important,
            target: RepairTarget {
                kind: RepairTargetKind::Node,
                node_ids: vec!["n1".to_string()],
                selected_text: None,
                region: None,
                drawing: None,
            },
            created_at: "2026-08-12T00:00:00Z".to_string(),
            page_id: "checkout".to_string(),
            url: "http://127.0.0.1/checkout".to_string(),
            context_revision: 3,
            context: json!({ "untrusted": true }),
            status: RepairStatus::Queued,
            submitted_at: "2026-08-12T00:00:01Z".to_string(),
        }
    }

    fn transition(status: RepairStatus, request_id: &str) -> RepairTransition {
        RepairTransition {
            session: "repair-session".to_string(),
            finding_id: "finding-1".to_string(),
            request_id: request_id.to_string(),
            status,
            actor: RepairActor::Agent,
            attempt_id: Some("attempt-1".to_string()),
            lease_expires_at_ms: Some(10_000),
            summary: Some("working".to_string()),
            message: None,
            verification: None,
        }
    }

    fn evidence(revision: u64) -> RepairEvidenceBundle {
        RepairEvidenceBundle {
            captured_at_ms: 1,
            context_revision: revision,
            context_sha256: "a".repeat(64),
            context: ready_page_context(revision),
            console_errors: 0,
            page_errors: 0,
            screenshot: Evidence {
                name: "before".to_string(),
                path: "repairs/finding-1/submitted/before.png".to_string(),
                media_type: "image/png".to_string(),
            },
            screenshot_sha256: "b".repeat(64),
        }
    }

    fn ready_page_context(revision: u64) -> PageContextSnapshot {
        use a3s_test_core::{
            PageContextPage, PageContextPoint, PageContextSize, PageContextTheme,
            PageContextViewport,
        };
        PageContextSnapshot {
            protocol: Some("a3s.test.page-context/1".to_string()),
            sdk_version: Some("0.1.0".to_string()),
            revision: Some(revision),
            page: Some(PageContextPage {
                id: "repair-page".to_string(),
                url: "http://127.0.0.1/checkout".to_string(),
                route: "/checkout".to_string(),
                title: "Checkout".to_string(),
                ready: true,
                viewport: PageContextViewport {
                    width: 1280.0,
                    height: 720.0,
                    dpr: 1.0,
                },
                document: PageContextSize {
                    width: 1280.0,
                    height: 720.0,
                },
                scroll: PageContextPoint { x: 0.0, y: 0.0 },
                language: "en".to_string(),
                theme: PageContextTheme::Light,
            }),
            components: Vec::new(),
            nodes: Vec::new(),
            facts: Default::default(),
            removed_node_ids: Vec::new(),
            truncated: false,
            next_cursor: None,
        }
    }

    async fn attach_all_evidence(ledger: &mut RepairLedger) {
        let records = ledger.queued(50);
        for record in records {
            ledger
                .attach_before_evidence(
                    "repair-session",
                    &record.finding.id,
                    evidence(record.finding.context_revision),
                )
                .await
                .expect("attach before evidence");
        }
    }

    #[tokio::test]
    async fn persists_order_and_idempotent_state_transitions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("repairs.jsonl");
        let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
        ledger
            .ingest(
                "repair-session",
                vec![finding("finding-1"), finding("finding-2")],
                1,
            )
            .await
            .expect("ingest");
        attach_all_evidence(&mut ledger).await;
        assert_eq!(
            ledger
                .queued(10)
                .iter()
                .map(|record| record.finding.id.as_str())
                .collect::<Vec<_>>(),
            ["finding-1", "finding-2"]
        );
        assert_eq!(
            ledger
                .transition(transition(RepairStatus::Claimed, "request-1"), 2)
                .await
                .expect("claim")
                .0
                .sequence,
            1
        );
        assert_eq!(
            ledger
                .transition(transition(RepairStatus::Claimed, "request-1"), 3)
                .await
                .expect("idempotent")
                .0
                .sequence,
            1
        );
        assert!(ledger
            .transition(transition(RepairStatus::Resolved, "request-2"), 4)
            .await
            .is_err());
        let replayed = RepairLedger::load(path).await.expect("replay");
        assert_eq!(
            replayed
                .queued(10)
                .iter()
                .map(|record| record.finding.id.as_str())
                .collect::<Vec<_>>(),
            ["finding-2"]
        );
    }

    #[tokio::test]
    async fn rejects_request_id_reuse_for_a_different_transition() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        ledger
            .ingest("repair-session", vec![finding("finding-1")], 1)
            .await
            .expect("ingest");
        attach_all_evidence(&mut ledger).await;
        let (claimed, event) = ledger
            .transition(transition(RepairStatus::Claimed, "request-1"), 2)
            .await
            .expect("claim");
        let (retried, retried_event) = ledger
            .transition(transition(RepairStatus::Claimed, "request-1"), 99)
            .await
            .expect("same request retry");
        assert_eq!(retried, claimed);
        assert_eq!(retried_event, event);

        let conflict = ledger
            .transition(transition(RepairStatus::Repairing, "request-1"), 3)
            .await
            .expect_err("conflicting idempotency key");
        assert_eq!(conflict.code(), "test.session.repair_idempotency_conflict");
    }

    #[tokio::test]
    async fn rejects_corrupt_or_cross_session_replay_without_mutating_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("repairs.jsonl");
        let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
        ledger
            .ingest("repair-session", vec![finding("finding-1")], 1)
            .await
            .expect("ingest");
        let mismatch = ledger
            .ingest("another-session", vec![finding("finding-2")], 2)
            .await
            .expect_err("session mismatch");
        assert_eq!(mismatch.code(), "test.session.repair_session_mismatch");

        tokio::fs::write(&path, "{not-json}\n")
            .await
            .expect("corrupt ledger");
        let invalid = match RepairLedger::load(path).await {
            Ok(_) => panic!("corrupt replay was accepted"),
            Err(error) => error,
        };
        assert_eq!(invalid.code(), "test.session.repair_ledger_invalid");
    }

    #[tokio::test]
    async fn recovers_pre_edit_and_possibly_mutated_expired_leases_differently() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("repairs.jsonl");
        let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
        ledger
            .ingest(
                "repair-session",
                vec![finding("finding-1"), finding("finding-2")],
                1,
            )
            .await
            .expect("ingest");
        attach_all_evidence(&mut ledger).await;
        ledger
            .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
            .await
            .expect("first claim");
        let pre_edit_recovery = ledger
            .recover_expired_leases("repair-session", 10_000)
            .await
            .expect("recover pre-edit claim");
        assert_eq!(pre_edit_recovery[0].0.status, RepairStatus::Queued);
        attach_all_evidence(&mut ledger).await;
        let mut second_claim = transition(RepairStatus::Claimed, "claim-2");
        second_claim.finding_id = "finding-2".to_string();
        second_claim.attempt_id = Some("attempt-2".to_string());
        second_claim.lease_expires_at_ms = Some(20_000);
        ledger
            .transition(second_claim, 10_001)
            .await
            .expect("second claim");
        let mut editing = transition(RepairStatus::Repairing, "editing-2");
        editing.finding_id = "finding-2".to_string();
        editing.attempt_id = Some("attempt-2".to_string());
        editing.lease_expires_at_ms = None;
        ledger.transition(editing, 10_002).await.expect("editing");

        let recovered = ledger
            .recover_expired_leases("repair-session", 20_000)
            .await
            .expect("recover leases");
        assert_eq!(
            recovered
                .iter()
                .map(|(record, _)| (record.finding.id.as_str(), record.status))
                .collect::<Vec<_>>(),
            [("finding-2", RepairStatus::NeedsInput)]
        );
        assert_eq!(ledger.queued(10)[0].finding.id, "finding-1");
        assert_eq!(ledger.current_events().len(), 2);

        let replayed = RepairLedger::load(path).await.expect("replay");
        assert_eq!(replayed.queued(10)[0].finding.id, "finding-1");
        assert_eq!(
            replayed.records["finding-2"].status,
            RepairStatus::NeedsInput
        );
    }

    #[tokio::test]
    async fn requires_the_active_attempt_and_recovers_an_expired_claim() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        ledger
            .ingest("repair-session", vec![finding("finding-1")], 1)
            .await
            .expect("ingest");
        attach_all_evidence(&mut ledger).await;

        let mut claim = transition(RepairStatus::Claimed, "claim-1");
        claim.lease_expires_at_ms = Some(100);
        ledger.transition(claim, 10).await.expect("claim");

        let mut missing_attempt = transition(RepairStatus::Repairing, "progress-missing");
        missing_attempt.attempt_id = None;
        missing_attempt.lease_expires_at_ms = None;
        let error = ledger
            .transition(missing_attempt, 20)
            .await
            .expect_err("missing attempt must fail");
        assert_eq!(error.code(), "test.session.repair_attempt_invalid");

        let mut wrong_attempt = transition(RepairStatus::Repairing, "progress-wrong");
        wrong_attempt.attempt_id = Some("attempt-wrong".to_string());
        wrong_attempt.lease_expires_at_ms = None;
        let error = ledger
            .transition(wrong_attempt, 30)
            .await
            .expect_err("wrong attempt must fail");
        assert_eq!(error.code(), "test.session.repair_attempt_invalid");

        let mut expired_attempt = transition(RepairStatus::Repairing, "progress-expired");
        expired_attempt.lease_expires_at_ms = None;
        let error = ledger
            .transition(expired_attempt, 100)
            .await
            .expect_err("expired lease must fail");
        assert_eq!(error.code(), "test.session.repair_lease_expired");

        let recovered = ledger
            .recover_expired_leases("repair-session", 100)
            .await
            .expect("recover expired claim");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].0.status, RepairStatus::Queued);
        assert!(recovered[0].0.attempt_id.is_none());
        assert!(recovered[0].0.lease_expires_at_ms.is_none());
    }

    #[tokio::test]
    async fn persists_human_reply_review_and_attempt_history() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("repairs.jsonl");
        let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
        ledger
            .ingest("repair-session", vec![finding("finding-1")], 1)
            .await
            .expect("ingest");
        attach_all_evidence(&mut ledger).await;
        ledger
            .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
            .await
            .expect("claim");
        let mut question = transition(RepairStatus::NeedsInput, "question-1");
        question.message = Some("Which state?".to_string());
        question.lease_expires_at_ms = None;
        ledger.transition(question, 3).await.expect("question");

        let replied = ledger
            .apply_human_action(
                "repair-session",
                RepairHumanAction {
                    request_id: "human-reply-1".to_string(),
                    finding_id: "finding-1".to_string(),
                    action: RepairHumanActionKind::Reply,
                    timestamp: "2026-08-13T00:00:00Z".to_string(),
                    message: Some("Use the enabled state".to_string()),
                },
                4,
            )
            .await
            .expect("reply");
        assert_eq!(replied[0].0.status, RepairStatus::Queued);
        assert_eq!(replied[0].0.attempts.len(), 1);
        assert_eq!(replied[0].0.attempts[0].replies.len(), 2);
        attach_all_evidence(&mut ledger).await;

        let mut second_claim = transition(RepairStatus::Claimed, "claim-2");
        second_claim.attempt_id = Some("attempt-2".to_string());
        ledger
            .transition(second_claim, 5)
            .await
            .expect("second claim");
        let mut progress = transition(RepairStatus::Repairing, "progress-2");
        progress.attempt_id = Some("attempt-2".to_string());
        progress.lease_expires_at_ms = None;
        ledger.transition(progress, 6).await.expect("progress");
        let mut complete = transition(RepairStatus::Verifying, "complete-2");
        complete.attempt_id = Some("attempt-2".to_string());
        complete.lease_expires_at_ms = None;
        ledger.transition(complete, 7).await.expect("complete");
        let mut verified = transition(RepairStatus::ReviewReady, "verified-2");
        verified.actor = RepairActor::A3sTest;
        verified.attempt_id = Some("attempt-2".to_string());
        verified.lease_expires_at_ms = None;
        ledger.transition(verified, 8).await.expect("verified");
        let accepted = ledger
            .apply_human_action(
                "repair-session",
                RepairHumanAction {
                    request_id: "human-accept-1".to_string(),
                    finding_id: "finding-1".to_string(),
                    action: RepairHumanActionKind::Accept,
                    timestamp: "2026-08-13T00:00:01Z".to_string(),
                    message: None,
                },
                9,
            )
            .await
            .expect("accept");
        assert_eq!(accepted[0].0.status, RepairStatus::Resolved);
        assert_eq!(accepted[0].0.attempts.len(), 2);

        let replayed = RepairLedger::load(path).await.expect("replay");
        let record = replayed.get("finding-1").expect("record");
        assert_eq!(record.status, RepairStatus::Resolved);
        assert_eq!(record.attempts.len(), 2);
        assert_eq!(record.attempts[0].replies.len(), 2);
        assert_eq!(replayed.batches()[0].status, RepairBatchStatus::Resolved);
    }

    #[tokio::test]
    async fn replays_page_human_actions_idempotently_after_the_page_reload() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("repairs.jsonl");
        let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
        ledger
            .ingest("repair-session", vec![finding("finding-1")], 1)
            .await
            .expect("ingest");
        attach_all_evidence(&mut ledger).await;
        ledger
            .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
            .await
            .expect("claim");
        let mut question = transition(RepairStatus::NeedsInput, "question-1");
        question.message = Some("Which state?".to_string());
        question.lease_expires_at_ms = None;
        ledger.transition(question, 3).await.expect("question");
        let action = RepairHumanAction {
            request_id: "human-reply-replayed".to_string(),
            finding_id: "finding-1".to_string(),
            action: RepairHumanActionKind::Reply,
            timestamp: "2026-08-13T00:00:00Z".to_string(),
            message: Some("Use the enabled state".to_string()),
        };
        ledger
            .apply_human_action("repair-session", action.clone(), 4)
            .await
            .expect("first reply");
        let replayed = ledger
            .apply_human_action("repair-session", action.clone(), 99)
            .await
            .expect("same live action is idempotent");
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].0.sequence, 3);

        let mut reloaded = RepairLedger::load(path).await.expect("reload");
        let replayed = reloaded
            .apply_human_action("repair-session", action, 100)
            .await
            .expect("page reload action is idempotent");
        assert_eq!(replayed[0].0.status, RepairStatus::Queued);
        assert_eq!(replayed[0].0.attempts[0].status, RepairStatus::NeedsInput);
        assert_eq!(replayed[0].0.attempts[0].finished_at_ms, Some(3));
    }

    #[tokio::test]
    async fn admits_a_newer_ready_before_evidence_baseline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        ledger
            .ingest("repair-session", vec![finding("finding-1")], 1)
            .await
            .expect("ingest");
        let attached = ledger
            .attach_before_evidence("repair-session", "finding-1", evidence(5))
            .await
            .expect("newer evidence");
        assert_eq!(attached.before_evidence.unwrap().context_revision, 5);

        let mut stale = evidence(2);
        stale.context.page.as_mut().expect("page").ready = true;
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        ledger
            .ingest("repair-session", vec![finding("finding-1")], 1)
            .await
            .expect("ingest");
        assert_eq!(
            ledger
                .attach_before_evidence("repair-session", "finding-1", stale)
                .await
                .expect_err("stale evidence")
                .code(),
            "test.session.repair_evidence_invalid"
        );
    }

    #[tokio::test]
    async fn independent_batch_failures_retain_order_and_per_item_results() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        ledger
            .ingest(
                "repair-session",
                vec![finding("finding-1"), finding("finding-2")],
                1,
            )
            .await
            .expect("ingest");
        let mut failed = transition(RepairStatus::Failed, "failed-1");
        failed.actor = RepairActor::A3sTest;
        failed.attempt_id = None;
        failed.lease_expires_at_ms = None;
        ledger.transition(failed, 2).await.expect("fail first");
        assert_eq!(ledger.queued(10)[0].finding.id, "finding-2");
        let batch = &ledger.batches()[0];
        assert_eq!(batch.finding_ids, ["finding-1", "finding-2"]);
        assert_eq!(batch.results[0].status, RepairStatus::Failed);
        assert_eq!(batch.results[1].status, RepairStatus::Queued);
        assert_eq!(batch.status, RepairBatchStatus::InProgress);
    }

    #[tokio::test]
    async fn serializes_workspace_mutations_and_moves_overlaps_to_needs_input() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        let mut first = finding("finding-1");
        first.target.node_ids = vec!["shared-node".to_string()];
        let mut second = finding("finding-2");
        second.target.node_ids = vec!["shared-node".to_string()];
        ledger
            .ingest("repair-session", vec![first, second], 1)
            .await
            .expect("ingest");
        let conflicts = ledger
            .resolve_conflicts("repair-session", 2)
            .await
            .expect("resolve conflicts");
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts
            .iter()
            .all(|(record, _)| record.status == RepairStatus::NeedsInput));

        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        let first = finding("finding-1");
        let mut second = finding("finding-2");
        second.target.node_ids = vec!["n2".to_string()];
        ledger
            .ingest("repair-session", vec![first, second], 1)
            .await
            .expect("ingest");
        attach_all_evidence(&mut ledger).await;
        ledger
            .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
            .await
            .expect("first claim");
        let mut second_claim = transition(RepairStatus::Claimed, "claim-2");
        second_claim.finding_id = "finding-2".to_string();
        second_claim.attempt_id = Some("attempt-2".to_string());
        let error = ledger
            .transition(second_claim, 3)
            .await
            .expect_err("concurrent workspace claim");
        assert_eq!(error.code(), "test.session.repair_workspace_busy");
        assert!(error.retryable());
    }
}
