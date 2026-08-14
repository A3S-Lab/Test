use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_worker::{
    DistributedScenarioObservation, DistributedScenarioOutcome, DistributedShard,
    DistributedShardIssue, RemoteInputBundle, RemoteJobSnapshot, RemoteJobState,
    RemoteJobSubmission, RemoteWorkerCommand, RemoteWorkerOutcome, RemoteWorkerRequest,
    WorkerSurface, REMOTE_WORKER_PROTOCOL,
};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::config::DistributedConfig;
use super::report::{fetch_verified_report, observations};
use super::runtime::InspectedWorker;
use super::{request_id, unix_ms};

#[derive(Debug)]
pub(super) struct ShardExecution {
    pub index: u16,
    pub observations: Vec<DistributedScenarioObservation>,
    pub issues: Vec<DistributedShardIssue>,
}

#[derive(Debug)]
struct CallFailure {
    code: String,
    message: String,
    retryable: bool,
}

impl CallFailure {
    fn transport(error: anyhow::Error) -> Self {
        Self {
            code: "test.distributed.transport_failed".to_string(),
            message: format!("{error:#}"),
            retryable: true,
        }
    }

    fn protocol(message: impl Into<String>) -> Self {
        Self {
            code: "test.distributed.protocol_invalid".to_string(),
            message: message.into(),
            retryable: false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_shard(
    config: Arc<DistributedConfig>,
    worker: InspectedWorker,
    shard: DistributedShard,
    run_id: String,
    suite: String,
    input: Arc<RemoteInputBundle>,
    scenario_surfaces: Arc<BTreeMap<String, WorkerSurface>>,
    submission_slots: Arc<Semaphore>,
    cancellation: CancellationToken,
) -> ShardExecution {
    if cancellation.is_cancelled() {
        return terminal_shard(
            &shard,
            DistributedScenarioOutcome::Cancelled,
            "test.distributed.cancelled",
            None,
        );
    }
    match execute_inner(
        &config,
        &worker,
        &shard,
        &run_id,
        &suite,
        input,
        &scenario_surfaces,
        submission_slots,
        cancellation,
    )
    .await
    {
        Ok(execution) => execution,
        Err(failure) => failed_shard(&shard, failure),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_inner(
    config: &DistributedConfig,
    worker: &InspectedWorker,
    shard: &DistributedShard,
    run_id: &str,
    suite: &str,
    input: Arc<RemoteInputBundle>,
    scenario_surfaces: &BTreeMap<String, WorkerSurface>,
    submission_slots: Arc<Semaphore>,
    cancellation: CancellationToken,
) -> Result<ShardExecution, CallFailure> {
    let issued_at_ms = unix_ms().map_err(CallFailure::transport)?;
    let deadline_ms = issued_at_ms
        .checked_add(config.job_timeout_ms)
        .ok_or_else(|| CallFailure::protocol("distributed job deadline overflowed"))?;
    let lease_expires_at_ms = issued_at_ms
        .checked_add(config.lease_ms)
        .map(|lease| lease.min(deadline_ms))
        .ok_or_else(|| CallFailure::protocol("distributed job lease overflowed"))?;
    let job_id = format!("job-{run_id}-s{}", shard.index);
    let dispatch_id = format!("dispatch-{run_id}-s{}", shard.index);
    let required_surfaces = shard
        .scenario_ids
        .iter()
        .map(|id| {
            scenario_surfaces.get(id).copied().ok_or_else(|| {
                CallFailure::protocol(format!("planned scenario '{id}' is absent from the suite"))
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?
        .into_iter()
        .collect::<Vec<_>>();
    if required_surfaces != shard.required_surfaces {
        return Err(CallFailure::protocol(
            "planned shard surfaces do not match the selected suite scenarios",
        ));
    }

    let submission_slot = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Ok(terminal_shard(
                shard,
                DistributedScenarioOutcome::Cancelled,
                "test.distributed.cancelled",
                None,
            ));
        }
        permit = submission_slots.acquire_owned() => {
            permit.map_err(|_| CallFailure::protocol("distributed submission limiter closed"))?
        }
    };
    let submission = RemoteJobSubmission {
        job_id: job_id.clone(),
        dispatch_id: dispatch_id.clone(),
        worker_instance: shard.worker_instance.clone(),
        required_image_digest: shard.required_image_digest.clone(),
        required_inventory_digest: shard.required_inventory_digest.clone(),
        issued_at_ms,
        deadline_ms,
        lease_expires_at_ms,
        max_parallel_scenarios: shard.max_parallel_scenarios,
        required_surfaces,
        required_host_permission_digest: shard.required_host_permission_digest.clone(),
        scenario_ids: shard.scenario_ids.clone(),
        input: (*input).clone(),
    };
    let admitted = submission
        .admit(issued_at_ms, &worker.descriptor)
        .map_err(|error| CallFailure {
            code: error.code().to_string(),
            message: error.message,
            retryable: error.retryable,
        })?;
    let expected_request_digest = admitted.request_digest().to_string();
    let request = RemoteWorkerRequest {
        protocol: REMOTE_WORKER_PROTOCOL.to_string(),
        request_id: request_id("submit"),
        command: RemoteWorkerCommand::Submit { job: submission },
    };
    let submitted = submit_with_retry(
        &worker.client,
        &request,
        lease_expires_at_ms,
        Duration::from_millis(config.poll_interval_ms),
        &cancellation,
    )
    .await;
    drop(submission_slot);
    let snapshot = match submitted {
        Ok(snapshot) => snapshot,
        Err(failure) => {
            let _ = cancel_exact(
                &worker.client,
                &job_id,
                &dispatch_id,
                &expected_request_digest,
                deadline_ms,
                "distributed submission outcome was uncertain",
            )
            .await;
            return Err(failure);
        }
    };
    if let Err(failure) = verify_job_binding(
        &snapshot,
        &job_id,
        &dispatch_id,
        &expected_request_digest,
        deadline_ms,
    ) {
        let _ = cancel_exact(
            &worker.client,
            &job_id,
            &dispatch_id,
            &expected_request_digest,
            deadline_ms,
            "distributed submission acknowledgment was not bound to the dispatched job",
        )
        .await;
        return Err(failure);
    }
    if cancellation.is_cancelled() {
        let cancellation_error = cancel_exact(
            &worker.client,
            &job_id,
            &dispatch_id,
            &expected_request_digest,
            deadline_ms,
            "distributed run was interrupted during submission",
        )
        .await
        .err();
        return Ok(cancelled_shard(shard, cancellation_error));
    }
    if snapshot.state.terminal() {
        return terminal_snapshot(
            worker,
            shard,
            suite,
            scenario_surfaces,
            snapshot,
            &cancellation,
        )
        .await;
    }

    let lease_cancellation = CancellationToken::new();
    let mut lease_task = tokio::spawn(renew_leases(
        worker.client.clone(),
        job_id.clone(),
        dispatch_id.clone(),
        expected_request_digest.clone(),
        snapshot.lease_expires_at_ms,
        deadline_ms,
        config.lease_ms,
        config.poll_interval_ms,
        lease_cancellation.clone(),
    ));
    let mut poll = tokio::time::interval(Duration::from_millis(config.poll_interval_ms));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let terminal = loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                lease_cancellation.cancel();
                let cancellation_error = cancel_exact(
                    &worker.client,
                    &job_id,
                    &dispatch_id,
                    &expected_request_digest,
                    deadline_ms,
                    "distributed run was interrupted",
                ).await.err();
                let _ = lease_task.await;
                return Ok(cancelled_shard(shard, cancellation_error));
            }
            lease = &mut lease_task => {
                let failure = match lease {
                    Ok(Err(failure)) => failure,
                    Ok(Ok(())) => CallFailure::protocol("lease supervisor ended before the job became terminal"),
                    Err(error) => CallFailure::protocol(format!("lease supervisor failed: {error}")),
                };
                let _ = cancel_exact(
                    &worker.client,
                    &job_id,
                    &dispatch_id,
                    &expected_request_digest,
                    deadline_ms,
                    "distributed lease supervision failed",
                ).await;
                return Err(failure);
            }
            _ = poll.tick() => {
                let now_ms = match unix_ms() {
                    Ok(now_ms) => now_ms,
                    Err(error) => {
                        lease_cancellation.cancel();
                        let _ = cancel_exact(
                            &worker.client,
                            &job_id,
                            &dispatch_id,
                            &expected_request_digest,
                            deadline_ms,
                            "distributed coordinator clock failed during status polling",
                        ).await;
                        let _ = lease_task.await;
                        return Err(CallFailure::transport(error));
                    }
                };
                if now_ms >= deadline_ms {
                    lease_cancellation.cancel();
                    let _ = lease_task.await;
                    return Ok(terminal_shard(
                        shard,
                        DistributedScenarioOutcome::TimedOut,
                        "test.worker.remote.deadline_exceeded",
                        Some(issue(
                            shard,
                            "test.distributed.status_unavailable",
                            "worker did not return a terminal snapshot before the bound deadline",
                        )),
                    ));
                }
                let request = RemoteWorkerRequest {
                    protocol: REMOTE_WORKER_PROTOCOL.to_string(),
                    request_id: request_id("status"),
                    command: RemoteWorkerCommand::Status {
                        job_id: job_id.clone(),
                        dispatch_id: dispatch_id.clone(),
                    },
                };
                let status = tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        lease_cancellation.cancel();
                        let cancellation_error = cancel_exact(
                            &worker.client,
                            &job_id,
                            &dispatch_id,
                            &expected_request_digest,
                            deadline_ms,
                            "distributed run was interrupted during status polling",
                        ).await.err();
                        let _ = lease_task.await;
                        return Ok(cancelled_shard(shard, cancellation_error));
                    }
                    response = call_job(&worker.client, &request) => response,
                };
                match status {
                    Ok(snapshot) => {
                        if let Err(failure) = verify_job_binding(
                            &snapshot,
                            &job_id,
                            &dispatch_id,
                            &expected_request_digest,
                            deadline_ms,
                        ) {
                            lease_cancellation.cancel();
                            let _ = cancel_exact(
                                &worker.client,
                                &job_id,
                                &dispatch_id,
                                &expected_request_digest,
                                deadline_ms,
                                "distributed status snapshot lost its immutable job binding",
                            ).await;
                            let _ = lease_task.await;
                            return Err(failure);
                        }
                        if snapshot.state.terminal() {
                            break snapshot;
                        }
                    }
                    Err(error) if error.retryable => {}
                    Err(error) => {
                        lease_cancellation.cancel();
                        let _ = cancel_exact(
                            &worker.client,
                            &job_id,
                            &dispatch_id,
                            &expected_request_digest,
                            deadline_ms,
                            "distributed status protocol failed",
                        ).await;
                        let _ = lease_task.await;
                        return Err(error);
                    }
                }
            }
        }
    };
    lease_cancellation.cancel();
    let _ = lease_task.await;
    terminal_snapshot(
        worker,
        shard,
        suite,
        scenario_surfaces,
        terminal,
        &cancellation,
    )
    .await
}

async fn submit_with_retry(
    client: &super::http::RemoteHttpClient,
    request: &RemoteWorkerRequest,
    lease_expires_at_ms: u64,
    retry_delay: Duration,
    cancellation: &CancellationToken,
) -> Result<RemoteJobSnapshot, CallFailure> {
    loop {
        if cancellation.is_cancelled() {
            return Err(cancellation_failure(
                "distributed run was cancelled before submission",
            ));
        }
        let response = call_job(client, request).await;
        if cancellation.is_cancelled() {
            return match response {
                Ok(snapshot) => Ok(snapshot),
                Err(_) => Err(cancellation_failure(
                    "distributed run was cancelled during submission",
                )),
            };
        }
        match response {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) if error.retryable => {
                let now_ms = unix_ms().map_err(CallFailure::transport)?;
                let delay_ms = u64::try_from(retry_delay.as_millis()).unwrap_or(u64::MAX);
                if now_ms.saturating_add(delay_ms) >= lease_expires_at_ms {
                    return Err(error);
                }
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        return Err(cancellation_failure(
                            "distributed run was cancelled during submission retry",
                        ));
                    }
                    _ = tokio::time::sleep(retry_delay) => {}
                }
            }
            Err(error) => return Err(error),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn renew_leases(
    client: super::http::RemoteHttpClient,
    job_id: String,
    dispatch_id: String,
    expected_request_digest: String,
    mut current_lease_ms: u64,
    deadline_ms: u64,
    lease_ms: u64,
    retry_delay_ms: u64,
    cancellation: CancellationToken,
) -> Result<(), CallFailure> {
    let interval_ms = (lease_ms / 3).max(100);
    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Ok(()),
            _ = tokio::time::sleep(Duration::from_millis(interval_ms)) => {}
        }
        let now_ms = unix_ms().map_err(CallFailure::transport)?;
        if now_ms >= current_lease_ms {
            return Err(CallFailure {
                code: "test.distributed.lease_expired".to_string(),
                message: "distributed worker lease expired before renewal completed".to_string(),
                retryable: false,
            });
        }
        let next_lease = now_ms.saturating_add(lease_ms).min(deadline_ms);
        if next_lease <= current_lease_ms {
            continue;
        }
        let request = RemoteWorkerRequest {
            protocol: REMOTE_WORKER_PROTOCOL.to_string(),
            request_id: request_id("renew"),
            command: RemoteWorkerCommand::RenewLease {
                job_id: job_id.clone(),
                dispatch_id: dispatch_id.clone(),
                lease_expires_at_ms: next_lease,
            },
        };
        match tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Ok(()),
            response = call_job(&client, &request) => response,
        } {
            Ok(snapshot) if snapshot.state.terminal() => {
                verify_job_binding(
                    &snapshot,
                    &job_id,
                    &dispatch_id,
                    &expected_request_digest,
                    deadline_ms,
                )?;
                cancellation.cancelled().await;
                return Ok(());
            }
            Ok(snapshot) => {
                verify_job_binding(
                    &snapshot,
                    &job_id,
                    &dispatch_id,
                    &expected_request_digest,
                    deadline_ms,
                )?;
                if snapshot.lease_expires_at_ms != next_lease {
                    return Err(CallFailure::protocol(
                        "renewed lease snapshot did not preserve the exact job binding",
                    ));
                }
                current_lease_ms = next_lease;
            }
            Err(error) if error.code == "test.worker.remote.lease_state_invalid" => {
                cancellation.cancelled().await;
                return Ok(());
            }
            Err(error) if error.retryable => {
                let now_ms = unix_ms().map_err(CallFailure::transport)?;
                if now_ms.saturating_add(retry_delay_ms) >= current_lease_ms {
                    return Err(error);
                }
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(Duration::from_millis(retry_delay_ms)) => {}
                }
            }
            Err(error) => return Err(error),
        }
    }
}

async fn terminal_snapshot(
    worker: &InspectedWorker,
    shard: &DistributedShard,
    suite: &str,
    scenario_surfaces: &BTreeMap<String, WorkerSurface>,
    snapshot: RemoteJobSnapshot,
    cancellation: &CancellationToken,
) -> Result<ShardExecution, CallFailure> {
    if cancellation.is_cancelled() {
        return Ok(cancelled_shard(shard, None));
    }
    match snapshot.state {
        RemoteJobState::Passed | RemoteJobState::Failed if snapshot.result.is_some() => {
            if snapshot.result.as_ref().is_some_and(|summary| {
                summary.report.bytes > worker.descriptor.limits.max_report_bytes
            }) {
                return Err(CallFailure::protocol(
                    "remote run report exceeds the inspected worker report limit",
                ));
            }
            let result = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Ok(cancelled_shard(shard, None)),
                result = fetch_verified_report(
                    &worker.client,
                    &worker.artifacts,
                    &snapshot,
                    suite,
                    &shard.scenario_ids,
                    scenario_surfaces,
                ) => result,
            }
            .map_err(|error| CallFailure {
                code: "test.distributed.report_invalid".to_string(),
                message: format!("{error:#}"),
                retryable: false,
            })?;
            Ok(ShardExecution {
                index: shard.index,
                observations: observations(&result),
                issues: Vec::new(),
            })
        }
        RemoteJobState::Passed => Err(CallFailure::protocol(
            "passed remote job did not retain a digest-bound report",
        )),
        RemoteJobState::Failed => {
            let (code, message) = snapshot
                .error
                .map(|error| (error.code, error.message))
                .unwrap_or_else(|| {
                    (
                        "test.distributed.remote_failed".to_string(),
                        "remote job failed without a typed error or report".to_string(),
                    )
                });
            Ok(terminal_shard(
                shard,
                DistributedScenarioOutcome::InfrastructureFailed,
                &code,
                Some(issue(shard, &code, &message)),
            ))
        }
        RemoteJobState::TimedOut => Ok(terminal_shard(
            shard,
            DistributedScenarioOutcome::TimedOut,
            snapshot
                .error
                .as_ref()
                .map_or("test.worker.remote.deadline_exceeded", |error| {
                    error.code.as_str()
                }),
            snapshot
                .error
                .as_ref()
                .map(|error| issue(shard, &error.code, &error.message)),
        )),
        RemoteJobState::Cancelled => Ok(terminal_shard(
            shard,
            DistributedScenarioOutcome::Cancelled,
            snapshot
                .error
                .as_ref()
                .map_or("test.worker.remote.cancelled", |error| error.code.as_str()),
            None,
        )),
        RemoteJobState::Interrupted => Ok(terminal_shard(
            shard,
            DistributedScenarioOutcome::Interrupted,
            snapshot
                .error
                .as_ref()
                .map_or("test.worker.remote.interrupted", |error| {
                    error.code.as_str()
                }),
            snapshot
                .error
                .as_ref()
                .map(|error| issue(shard, &error.code, &error.message)),
        )),
        RemoteJobState::Queued | RemoteJobState::Running | RemoteJobState::Cancelling => Err(
            CallFailure::protocol("non-terminal remote job reached terminal report handling"),
        ),
    }
}

async fn call_job(
    client: &super::http::RemoteHttpClient,
    request: &RemoteWorkerRequest,
) -> Result<RemoteJobSnapshot, CallFailure> {
    let response = client
        .worker(request)
        .await
        .map_err(CallFailure::transport)?;
    match response.outcome {
        RemoteWorkerOutcome::Job { job } => Ok(job),
        RemoteWorkerOutcome::Error { error } => Err(CallFailure {
            code: error.code,
            message: error.message,
            retryable: error.retryable,
        }),
        RemoteWorkerOutcome::Descriptor { .. } => Err(CallFailure::protocol(
            "remote worker returned a descriptor for a job command",
        )),
    }
}

async fn cancel_exact(
    client: &super::http::RemoteHttpClient,
    job_id: &str,
    dispatch_id: &str,
    expected_request_digest: &str,
    deadline_ms: u64,
    reason: &str,
) -> Result<RemoteJobSnapshot, CallFailure> {
    let request = RemoteWorkerRequest {
        protocol: REMOTE_WORKER_PROTOCOL.to_string(),
        request_id: request_id("cancel"),
        command: RemoteWorkerCommand::Cancel {
            job_id: job_id.to_string(),
            dispatch_id: dispatch_id.to_string(),
            reason: Some(reason.to_string()),
        },
    };
    let snapshot = call_job(client, &request).await?;
    verify_job_binding(
        &snapshot,
        job_id,
        dispatch_id,
        expected_request_digest,
        deadline_ms,
    )?;
    Ok(snapshot)
}

fn verify_job_binding(
    snapshot: &RemoteJobSnapshot,
    job_id: &str,
    dispatch_id: &str,
    request_digest: &str,
    deadline_ms: u64,
) -> Result<(), CallFailure> {
    if snapshot.job_id != job_id
        || snapshot.dispatch_id != dispatch_id
        || snapshot.request_digest != request_digest
        || snapshot.deadline_ms != deadline_ms
        || snapshot.lease_expires_at_ms > deadline_ms
    {
        return Err(CallFailure::protocol(
            "remote worker snapshot did not preserve the immutable dispatch binding",
        ));
    }
    Ok(())
}

fn failed_shard(shard: &DistributedShard, failure: CallFailure) -> ShardExecution {
    terminal_shard(
        shard,
        if failure.code == "test.distributed.cancelled" {
            DistributedScenarioOutcome::Cancelled
        } else {
            DistributedScenarioOutcome::InfrastructureFailed
        },
        &failure.code,
        Some(issue(shard, &failure.code, &failure.message)),
    )
}

fn cancellation_failure(message: &str) -> CallFailure {
    CallFailure {
        code: "test.distributed.cancelled".to_string(),
        message: message.to_string(),
        retryable: false,
    }
}

fn cancelled_shard(shard: &DistributedShard, failure: Option<CallFailure>) -> ShardExecution {
    terminal_shard(
        shard,
        DistributedScenarioOutcome::Cancelled,
        "test.distributed.cancelled",
        failure.map(|failure| issue(shard, &failure.code, &failure.message)),
    )
}

fn terminal_shard(
    shard: &DistributedShard,
    outcome: DistributedScenarioOutcome,
    failure_code: &str,
    issue: Option<DistributedShardIssue>,
) -> ShardExecution {
    ShardExecution {
        index: shard.index,
        observations: shard
            .scenario_ids
            .iter()
            .map(|id| DistributedScenarioObservation {
                id: id.clone(),
                outcome,
                duration_ms: 0,
                failure_code: Some(bounded(failure_code, 128)),
            })
            .collect(),
        issues: issue.into_iter().collect(),
    }
}

fn issue(shard: &DistributedShard, code: &str, message: &str) -> DistributedShardIssue {
    DistributedShardIssue {
        shard_index: shard.index,
        worker_instance: shard.worker_instance.clone(),
        code: bounded(code, 128),
        message: bounded(message, 2_048),
    }
}

fn bounded(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}
