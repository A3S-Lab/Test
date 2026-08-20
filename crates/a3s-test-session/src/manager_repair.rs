use super::*;

impl AgentSessionManager {
    pub async fn take_repairs(
        &self,
        session: &str,
        limit: usize,
    ) -> Result<Vec<RepairFinding>, SessionError> {
        let managed = self.get(session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        managed
            .driver
            .take_repairs(limit.clamp(1, 50))
            .await
            .map_err(SessionError::from_driver)
    }

    pub async fn apply_repair_event(
        &self,
        session: &str,
        event: &RepairStatusEvent,
    ) -> Result<(), SessionError> {
        let managed = self.get(session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        managed
            .driver
            .apply_repair_event(event)
            .await
            .map_err(SessionError::from_driver)
    }

    pub async fn ingest_repairs(
        &self,
        session: &str,
        limit: usize,
    ) -> Result<Vec<RepairRecord>, SessionError> {
        let managed = self.get(session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        let findings = managed
            .driver
            .take_repairs(limit.clamp(1, 50))
            .await
            .map_err(SessionError::from_driver)?;
        let session_id = managed.id.clone();
        let created = {
            let mut workspace = self.repair_workspace.acquire().await?;
            managed
                .repair_ledger
                .ingest_in_workspace(&session_id, findings, unix_ms(), &mut workspace)
                .await?
        };
        for record in &created {
            let evidence = managed
                .driver
                .capture_repair_evidence(&RepairEvidenceRequest {
                    finding_id: record.finding.id.clone(),
                    attempt_id: None,
                    phase: RepairEvidencePhase::Before,
                })
                .await
                .map_err(SessionError::from_driver)?;
            let mut workspace = self.repair_workspace.acquire().await?;
            managed
                .repair_ledger
                .attach_before_evidence_in_workspace(
                    &session_id,
                    &record.finding.id,
                    evidence,
                    &mut workspace,
                )
                .await?;
        }
        let conflicts = {
            let mut workspace = self.repair_workspace.acquire().await?;
            managed
                .repair_ledger
                .resolve_conflicts_in_workspace(&session_id, unix_ms(), &mut workspace)
                .await?
        };
        for (_, event) in conflicts {
            managed
                .driver
                .apply_repair_event(&event)
                .await
                .map_err(SessionError::from_driver)?;
        }
        Ok(created
            .into_iter()
            .filter_map(|record| managed.repair_ledger.get(&record.finding.id))
            .collect())
    }

    pub async fn watch_repairs(
        &self,
        session: &str,
        limit: usize,
        timeout_ms: u64,
        batch_window_ms: u64,
    ) -> Result<Vec<RepairRecord>, SessionError> {
        let managed = self.get(session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        let recovered = {
            let mut workspace = self.repair_workspace.acquire().await?;
            managed.repair_ledger.reload().await?;
            managed
                .repair_ledger
                .recover_expired_leases_in_workspace(session, unix_ms(), &mut workspace)
                .await?
        };
        let recovered_ids = recovered
            .iter()
            .map(|(_, event)| event.finding_id.as_str())
            .collect::<HashSet<_>>();
        for event in managed.repair_ledger.current_events() {
            if recovered_ids.contains(event.finding_id.as_str()) {
                continue;
            }
            managed
                .driver
                .apply_repair_event(&event)
                .await
                .map_err(|error| {
                    SessionError::new(
                        "test.session.repair_projection_failed",
                        format!(
                            "durable repair state could not be replayed to the page: {}",
                            error.message()
                        ),
                    )
                    .with_retryable(true)
                })?;
        }
        for (_, event) in recovered {
            managed
                .driver
                .apply_repair_event(&event)
                .await
                .map_err(|error| {
                    SessionError::new(
                        "test.session.repair_projection_failed",
                        format!(
                            "expired repair lease was durably recovered but page projection failed: {}",
                            error.message()
                        ),
                    )
                    .with_retryable(true)
                })?;
        }
        let human_actions = managed
            .driver
            .take_repair_actions(limit.clamp(1, 50))
            .await
            .map_err(SessionError::from_driver)?;
        let session_id = managed.id.clone();
        for action in human_actions {
            let transitions = {
                let mut workspace = self.repair_workspace.acquire().await?;
                managed
                    .repair_ledger
                    .apply_human_action_in_workspace(&session_id, action, unix_ms(), &mut workspace)
                    .await?
            };
            for (_, event) in transitions {
                managed
                    .driver
                    .apply_repair_event(&event)
                    .await
                    .map_err(|error| {
                        SessionError::new(
                            "test.session.repair_projection_failed",
                            format!(
                                "human repair action was durably recorded but page projection failed: {}",
                                error.message()
                            ),
                        )
                        .with_retryable(true)
                    })?;
            }
        }
        let conflicts = {
            let mut workspace = self.repair_workspace.acquire().await?;
            managed
                .repair_ledger
                .resolve_conflicts_in_workspace(&session_id, unix_ms(), &mut workspace)
                .await?
        };
        for (_, event) in conflicts {
            managed
                .driver
                .apply_repair_event(&event)
                .await
                .map_err(SessionError::from_driver)?;
        }
        self.capture_missing_before_evidence(&mut managed, limit.clamp(1, 50))
            .await?;
        let queued = managed.repair_ledger.queued(limit);
        if !queued.is_empty() {
            return Ok(queued);
        }
        let findings = managed
            .driver
            .wait_for_repairs(
                limit.clamp(1, 50),
                timeout_ms.min(300_000),
                batch_window_ms.min(5_000),
            )
            .await
            .map_err(SessionError::from_driver)?;
        let session_id = managed.id.clone();
        {
            let mut workspace = self.repair_workspace.acquire().await?;
            managed
                .repair_ledger
                .ingest_in_workspace(&session_id, findings, unix_ms(), &mut workspace)
                .await?;
        }
        self.capture_missing_before_evidence(&mut managed, limit.clamp(1, 50))
            .await?;
        let conflicts = {
            let mut workspace = self.repair_workspace.acquire().await?;
            managed
                .repair_ledger
                .resolve_conflicts_in_workspace(&session_id, unix_ms(), &mut workspace)
                .await?
        };
        for (_, event) in conflicts {
            managed
                .driver
                .apply_repair_event(&event)
                .await
                .map_err(SessionError::from_driver)?;
        }
        Ok(managed.repair_ledger.queued(limit))
    }

    async fn capture_missing_before_evidence(
        &self,
        managed: &mut ManagedSession,
        limit: usize,
    ) -> Result<(), SessionError> {
        let missing = managed
            .repair_ledger
            .queued(limit)
            .into_iter()
            .filter(|record| record.before_evidence.is_none())
            .collect::<Vec<_>>();
        let session_id = managed.id.clone();
        for record in missing {
            let evidence = managed
                .driver
                .capture_repair_evidence(&RepairEvidenceRequest {
                    finding_id: record.finding.id.clone(),
                    attempt_id: None,
                    phase: RepairEvidencePhase::Before,
                })
                .await
                .map_err(SessionError::from_driver)?;
            let mut workspace = self.repair_workspace.acquire().await?;
            managed
                .repair_ledger
                .attach_before_evidence_in_workspace(
                    &session_id,
                    &record.finding.id,
                    evidence,
                    &mut workspace,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn queued_repairs(
        &self,
        session: &str,
        limit: usize,
    ) -> Result<Vec<RepairRecord>, SessionError> {
        let managed = self.get(session).await?;
        let managed = managed.lock().await;
        ensure_active(&managed)?;
        Ok(managed.repair_ledger.queued(limit))
    }

    pub async fn repair_batches(&self, session: &str) -> Result<Vec<RepairBatch>, SessionError> {
        let managed = self.get(session).await?;
        let managed = managed.lock().await;
        ensure_active(&managed)?;
        Ok(managed.repair_ledger.batches())
    }

    pub async fn repair(
        &self,
        session: &str,
        finding_id: &str,
    ) -> Result<RepairRecord, SessionError> {
        let managed = self.get(session).await?;
        let managed = managed.lock().await;
        ensure_active(&managed)?;
        managed.repair_ledger.get(finding_id).ok_or_else(|| {
            SessionError::new(
                "test.session.repair_not_found",
                format!("repair finding '{finding_id}' does not exist"),
            )
        })
    }

    pub async fn inspect_repair_loop(
        &self,
        session: &str,
        finding_id: &str,
    ) -> Result<RepairLoopRecord, SessionError> {
        let managed = self.get(session).await?;
        let managed = managed.lock().await;
        ensure_active(&managed)?;
        managed.repair_ledger.inspect_loop(session, finding_id)
    }

    pub async fn transition_repair(
        &self,
        transition: RepairTransition,
    ) -> Result<RepairRecord, SessionError> {
        let managed = self.get(&transition.session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        let mut workspace = self.repair_workspace.acquire().await?;
        managed.repair_ledger.reload().await?;
        let (record, event) = managed
            .repair_ledger
            .transition_in_workspace(transition, unix_ms(), &mut workspace)
            .await?;
        drop(workspace);
        managed
            .driver
            .apply_repair_event(&event)
            .await
            .map_err(|error| {
                SessionError::new(
                    "test.session.repair_projection_failed",
                    format!(
                        "repair transition was durably recorded but page projection failed; retry the same request id: {}",
                        error.message()
                    ),
                )
                .with_retryable(true)
            })?;
        Ok(record)
    }

    pub async fn verify_repair(
        &self,
        request: RepairVerifyRequest,
    ) -> Result<RepairRecord, SessionError> {
        validate_repair_verification_request(&request)?;
        let managed = self.get(&request.session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        let current = {
            let workspace = self.repair_workspace.acquire().await?;
            managed.repair_ledger.reload().await?;
            let current = managed
                .repair_ledger
                .get(&request.finding_id)
                .ok_or_else(|| {
                    SessionError::new(
                        "test.session.repair_not_found",
                        format!("repair finding '{}' does not exist", request.finding_id),
                    )
                })?;
            if current.status != RepairStatus::Verifying {
                return Err(SessionError::new(
                    "test.session.repair_verify_invalid",
                    "repair verification requires the verifying state",
                ));
            }
            let attempt_id = current.attempt_id.as_deref().ok_or_else(|| {
                SessionError::new(
                    "test.session.repair_attempt_invalid",
                    "repair verification is missing its active attempt",
                )
            })?;
            workspace
                .validate_attempt_owner(
                    &request.session,
                    &request.finding_id,
                    attempt_id,
                    RepairStatus::Verifying,
                    unix_ms(),
                )
                .await?;
            current
        };
        let attempt_id = current.attempt_id.clone().ok_or_else(|| {
            SessionError::new(
                "test.session.repair_attempt_invalid",
                "repair verification is missing its active attempt",
            )
        })?;
        let before_evidence = current.before_evidence.clone().ok_or_else(|| {
            SessionError::new(
                "test.session.repair_evidence_missing",
                "repair verification is missing A3S-owned before evidence",
            )
        })?;
        validate_repair_verification_change(&current, &request.changed_files)?;
        let after_evidence = managed
            .driver
            .capture_repair_evidence(&RepairEvidenceRequest {
                finding_id: current.finding.id.clone(),
                attempt_id: Some(attempt_id.clone()),
                phase: RepairEvidencePhase::After,
            })
            .await
            .map_err(SessionError::from_driver)?;
        let prior_acl_proof_passed = latest_prior_acl_proof_passed(&current.attempts, &attempt_id);
        let mut verification = build_repair_verification_with_plan(
            &current.finding,
            &attempt_id,
            &before_evidence,
            &after_evidence,
            &request,
            prior_acl_proof_passed,
            &[],
        )?;
        if verification.passed {
            match verification.acl_candidate.as_deref() {
                Some(candidate) => {
                    let proof = managed
                        .driver
                        .prove_repair_acl(
                            &current.finding.id,
                            &attempt_id,
                            &current.finding.url,
                            candidate,
                        )
                        .await
                        .map_err(SessionError::from_driver)?;
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
        let auto_resolve_repairs = managed.auto_resolve_repairs;
        let verification_request_id = request.request_id.clone();
        let mut workspace = self.repair_workspace.acquire().await?;
        managed.repair_ledger.reload().await?;
        managed.repair_ledger.require_attempt_state(
            &request.finding_id,
            RepairStatus::Verifying,
            &attempt_id,
        )?;
        workspace
            .validate_attempt_owner(
                &request.session,
                &request.finding_id,
                &attempt_id,
                RepairStatus::Verifying,
                unix_ms(),
            )
            .await?;
        let transition = RepairTransition {
            session: request.session.clone(),
            finding_id: request.finding_id.clone(),
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
            changed_files: None,
        };
        let (mut record, event) = managed
            .repair_ledger
            .transition_in_workspace(transition, unix_ms(), &mut workspace)
            .await?;
        drop(workspace);
        managed
            .driver
            .apply_repair_event(&event)
            .await
            .map_err(SessionError::from_driver)?;
        if passed && auto_resolve_repairs {
            let mut workspace = self.repair_workspace.acquire().await?;
            managed.repair_ledger.reload().await?;
            managed.repair_ledger.require_attempt_state(
                &request.finding_id,
                RepairStatus::ReviewReady,
                &attempt_id,
            )?;
            let transition = RepairTransition {
                session: request.session,
                finding_id: request.finding_id,
                request_id: auto_resolution_request_id(&verification_request_id),
                status: RepairStatus::Resolved,
                actor: RepairActor::A3sTest,
                attempt_id: Some(attempt_id),
                lease_expires_at_ms: None,
                summary: Some(
                    "A3S Test automatically accepted the fully verified repair".to_string(),
                ),
                message: None,
                verification: None,
                changed_files: None,
            };
            let (resolved, event) = managed
                .repair_ledger
                .transition_in_workspace(transition, unix_ms(), &mut workspace)
                .await?;
            drop(workspace);
            managed
                .driver
                .apply_repair_event(&event)
                .await
                .map_err(SessionError::from_driver)?;
            record = resolved;
        }
        Ok(record)
    }
}
