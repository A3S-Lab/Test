use std::{sync::Arc, time::Duration};

use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use super::{
    admission::{remote_error, sha256, validate_token},
    persistence,
    service::{persist_transition, JobRecord, ServiceShared},
    RemoteExecutionJob, RemoteExecutionResult, RemoteJobState, RemoteReportDescriptor,
    RemoteRunSummary, RemoteWorkerError,
};

pub(super) async fn worker_loop(shared: Arc<ServiceShared>, mut queue: mpsc::Receiver<String>) {
    loop {
        tokio::select! {
            biased;
            _ = shared.shutdown.cancelled() => {
                interrupt_queued_jobs(&shared).await;
                break;
            }
            job_id = queue.recv() => {
                let Some(job_id) = job_id else {
                    break;
                };
                if let Err(error) = run_job(&shared, &job_id).await {
                    record_runtime_failure(&shared, &job_id, error).await;
                }
                super::artifacts::retention::enforce_retention(&shared).await;
            }
            _ = super::artifacts::retention::wait_until_due(&shared) => {
                super::artifacts::retention::enforce_retention(&shared).await;
            }
        }
    }
}

pub(super) async fn fail_dispatch(shared: &ServiceShared, job_id: &str) {
    record_runtime_failure(
        shared,
        job_id,
        RemoteWorkerError::new(
            "test.worker.remote.dispatch_failed",
            "remote job could not be dispatched to the execution loop",
            true,
        ),
    )
    .await;
}

struct StartedJob {
    execution: RemoteExecutionJob,
    cancellation: CancellationToken,
    lease_rx: watch::Receiver<u64>,
    deadline_ms: u64,
}

#[derive(Clone)]
struct Termination {
    state: RemoteJobState,
    error: RemoteWorkerError,
}

async fn run_job(shared: &ServiceShared, job_id: &str) -> Result<(), RemoteWorkerError> {
    let Some(started) = start_job(shared, job_id).await? else {
        return Ok(());
    };
    let StartedJob {
        execution,
        cancellation,
        mut lease_rx,
        deadline_ms,
    } = started;
    let execution_future = shared.executor.execute(execution, cancellation.clone());
    tokio::pin!(execution_future);

    let termination = loop {
        let lease_expires_at_ms = *lease_rx.borrow();
        let wake_at_ms = deadline_ms.min(lease_expires_at_ms);
        tokio::select! {
            biased;
            _ = shared.shutdown.cancelled() => {
                break Termination {
                    state: RemoteJobState::Interrupted,
                    error: RemoteWorkerError::new(
                        "test.worker.remote.shutdown_interrupted",
                        "remote job was interrupted by worker shutdown",
                        true,
                    ),
                };
            }
            _ = cancellation.cancelled() => {
                break current_termination(shared, job_id).await;
            }
            changed = lease_rx.changed() => {
                if changed.is_err() {
                    break Termination {
                        state: RemoteJobState::Interrupted,
                        error: RemoteWorkerError::new(
                            "test.worker.remote.lease_channel_closed",
                            "remote job lease supervision ended unexpectedly",
                            true,
                        ),
                    };
                }
            }
            _ = shared.clock.sleep_until_ms(wake_at_ms) => {
                let now_ms = shared.clock.now_ms();
                let current_lease = *lease_rx.borrow();
                if now_ms >= deadline_ms {
                    break Termination {
                        state: RemoteJobState::TimedOut,
                        error: RemoteWorkerError::new(
                            "test.worker.remote.deadline_exceeded",
                            "remote job exceeded its absolute deadline",
                            false,
                        ),
                    };
                }
                if now_ms >= current_lease {
                    break Termination {
                        state: RemoteJobState::Cancelled,
                        error: RemoteWorkerError::new(
                            "test.worker.remote.lease_expired",
                            "remote job claim lease expired",
                            true,
                        ),
                    };
                }
            }
            result = &mut execution_future => {
                return finish_executor(shared, job_id, result).await;
            }
        }
    };

    begin_termination(shared, job_id, &termination).await?;
    cancellation.cancel();
    let cleanup_timeout = Duration::from_millis(
        (shared
            .descriptor
            .limits
            .cleanup_timeout_ms
            .saturating_mul(3)
            / 4)
        .max(1),
    );
    let cleanup_timed_out = tokio::time::timeout(cleanup_timeout, &mut execution_future)
        .await
        .is_err();
    let termination = if cleanup_timed_out {
        let mut bounded = termination;
        bounded.error.message = format!(
            "{}; executor cleanup exceeded {} ms",
            bounded.error.message,
            cleanup_timeout.as_millis()
        );
        bounded
    } else {
        termination
    };
    finish_termination(shared, job_id, termination).await
}

async fn start_job(
    shared: &ServiceShared,
    job_id: &str,
) -> Result<Option<StartedJob>, RemoteWorkerError> {
    let now_ms = shared.clock.now_ms();
    let mut state = shared.state.lock().await;
    let outcome = {
        let Some(record) = state.jobs.get_mut(job_id) else {
            return Err(remote_error(
                "test.worker.remote.state_index_invalid",
                "queued remote job is missing from the state index",
            ));
        };
        if record.snapshot.state != RemoteJobState::Queued {
            return Ok(None);
        }
        let expiration = if now_ms >= record.snapshot.deadline_ms {
            Some(Termination {
                state: RemoteJobState::TimedOut,
                error: RemoteWorkerError::new(
                    "test.worker.remote.deadline_exceeded",
                    "remote job expired in the queue before execution",
                    false,
                ),
            })
        } else if now_ms >= record.snapshot.lease_expires_at_ms {
            Some(Termination {
                state: RemoteJobState::Cancelled,
                error: RemoteWorkerError::new(
                    "test.worker.remote.lease_expired",
                    "remote job lease expired in the queue before execution",
                    true,
                ),
            })
        } else {
            None
        };
        if let Some(expiration) = expiration {
            let mut next = record.snapshot.clone();
            next.state = expiration.state;
            next.finished_at_ms = Some(now_ms);
            next.error = Some(expiration.error);
            persist_transition(shared, record, next, now_ms).await?;
            record.execution = None;
            record.cancellation.cancel();
            None
        } else {
            let execution = record.execution.clone().ok_or_else(|| {
                remote_error(
                    "test.worker.remote.state_definition_missing",
                    "queued remote job has no executable definition",
                )
            })?;
            let mut next = record.snapshot.clone();
            next.state = RemoteJobState::Running;
            next.started_at_ms = Some(now_ms);
            persist_transition(shared, record, next, now_ms).await?;
            Some(StartedJob {
                execution,
                cancellation: record.cancellation.clone(),
                lease_rx: record.lease_tx.subscribe(),
                deadline_ms: record.snapshot.deadline_ms,
            })
        }
    };
    state.queued_jobs = state.queued_jobs.saturating_sub(1);
    Ok(outcome)
}

async fn begin_termination(
    shared: &ServiceShared,
    job_id: &str,
    termination: &Termination,
) -> Result<(), RemoteWorkerError> {
    let now_ms = shared.clock.now_ms();
    let mut state = shared.state.lock().await;
    let Some(record) = state.jobs.get_mut(job_id) else {
        return Err(remote_error(
            "test.worker.remote.job_not_found",
            "running remote job disappeared from worker state",
        ));
    };
    if record.snapshot.state.terminal() || record.snapshot.state == RemoteJobState::Cancelling {
        return Ok(());
    }
    let mut next = record.snapshot.clone();
    next.state = RemoteJobState::Cancelling;
    next.error = Some(termination.error.clone());
    persist_transition(shared, record, next, now_ms).await
}

async fn finish_termination(
    shared: &ServiceShared,
    job_id: &str,
    termination: Termination,
) -> Result<(), RemoteWorkerError> {
    let now_ms = shared.clock.now_ms();
    let mut state = shared.state.lock().await;
    let Some(record) = state.jobs.get_mut(job_id) else {
        return Err(remote_error(
            "test.worker.remote.job_not_found",
            "terminating remote job disappeared from worker state",
        ));
    };
    if record.snapshot.state.terminal() {
        return Ok(());
    }
    let mut next = record.snapshot.clone();
    next.state = termination.state;
    next.finished_at_ms = Some(now_ms);
    next.result = None;
    next.error = Some(termination.error);
    persist_transition(shared, record, next, now_ms).await?;
    record.execution = None;
    Ok(())
}

async fn finish_executor(
    shared: &ServiceShared,
    job_id: &str,
    result: Result<RemoteExecutionResult, RemoteWorkerError>,
) -> Result<(), RemoteWorkerError> {
    let now_ms = shared.clock.now_ms();
    let mut state = shared.state.lock().await;
    let Some(record) = state.jobs.get_mut(job_id) else {
        return Err(remote_error(
            "test.worker.remote.job_not_found",
            "completed remote job disappeared from worker state",
        ));
    };
    if record.snapshot.state == RemoteJobState::Cancelling {
        let termination = termination_from_record(record);
        drop(state);
        return finish_termination(shared, job_id, termination).await;
    }
    if record.snapshot.state.terminal() {
        return Ok(());
    }
    if record.snapshot.state != RemoteJobState::Running {
        return Err(remote_error(
            "test.worker.remote.state_transition_invalid",
            "executor completed from an invalid remote job state",
        ));
    }

    let mut next = record.snapshot.clone();
    next.finished_at_ms = Some(now_ms);
    match result {
        Ok(output) => {
            validate_execution_result(&output, shared.descriptor.limits.max_report_bytes)?;
            let descriptor = RemoteReportDescriptor {
                sha256: sha256(&output.report),
                bytes: output.report.len() as u64,
                media_type: output.media_type,
            };
            persistence::persist_report(&shared.root, job_id, &output.report).await?;
            next.state = output.status;
            next.result = Some(RemoteRunSummary {
                run_id: output.run_id,
                suite: output.suite,
                status: output.status,
                scenarios: output.scenarios,
                report: descriptor,
            });
            next.error = None;
        }
        Err(error) => {
            next.state = RemoteJobState::Failed;
            next.result = None;
            next.error = Some(validate_executor_error(error));
        }
    }
    persist_transition(shared, record, next, now_ms).await?;
    record.execution = None;
    Ok(())
}

async fn current_termination(shared: &ServiceShared, job_id: &str) -> Termination {
    let state = shared.state.lock().await;
    state
        .jobs
        .get(job_id)
        .map(termination_from_record)
        .unwrap_or_else(|| Termination {
            state: RemoteJobState::Interrupted,
            error: RemoteWorkerError::new(
                "test.worker.remote.state_index_invalid",
                "cancelled remote job disappeared from worker state",
                true,
            ),
        })
}

fn termination_from_record(record: &JobRecord) -> Termination {
    let error = record.snapshot.error.clone().unwrap_or_else(|| {
        RemoteWorkerError::new(
            "test.worker.remote.cancelled",
            "remote job cancellation was requested",
            false,
        )
    });
    let state = match error.code() {
        "test.worker.remote.deadline_exceeded" => RemoteJobState::TimedOut,
        "test.worker.remote.lease_expired" | "test.worker.remote.cancelled" => {
            RemoteJobState::Cancelled
        }
        _ => RemoteJobState::Interrupted,
    };
    Termination { state, error }
}

async fn interrupt_queued_jobs(shared: &ServiceShared) {
    let job_ids = {
        let state = shared.state.lock().await;
        state
            .jobs
            .iter()
            .filter(|(_, record)| record.snapshot.state == RemoteJobState::Queued)
            .map(|(job_id, _)| job_id.clone())
            .collect::<Vec<_>>()
    };
    for job_id in job_ids {
        let error = RemoteWorkerError::new(
            "test.worker.remote.shutdown_interrupted",
            "queued remote job was interrupted by worker shutdown",
            true,
        );
        record_runtime_failure_with_state(shared, &job_id, RemoteJobState::Interrupted, error)
            .await;
    }
}

async fn record_runtime_failure(shared: &ServiceShared, job_id: &str, error: RemoteWorkerError) {
    record_runtime_failure_with_state(shared, job_id, RemoteJobState::Failed, error).await;
}

async fn record_runtime_failure_with_state(
    shared: &ServiceShared,
    job_id: &str,
    terminal_state: RemoteJobState,
    error: RemoteWorkerError,
) {
    let now_ms = shared.clock.now_ms();
    let mut state = shared.state.lock().await;
    let mut was_queued = false;
    if let Some(record) = state.jobs.get_mut(job_id) {
        if record.snapshot.state.terminal() {
            return;
        }
        was_queued = record.snapshot.state == RemoteJobState::Queued;
        let mut next = record.snapshot.clone();
        next.state = terminal_state;
        next.finished_at_ms = Some(now_ms);
        next.result = None;
        next.error = Some(error);
        if persist_transition(shared, record, next.clone(), now_ms)
            .await
            .is_err()
        {
            record.snapshot = next;
        }
        record.execution = None;
        record.cancellation.cancel();
    }
    if was_queued {
        state.queued_jobs = state.queued_jobs.saturating_sub(1);
    }
}

fn validate_execution_result(
    result: &RemoteExecutionResult,
    max_report_bytes: u64,
) -> Result<(), RemoteWorkerError> {
    validate_token(&result.run_id, "executor run ID")?;
    if result.suite.trim().is_empty()
        || result.suite.len() > 256
        || result.suite.chars().any(char::is_control)
    {
        return Err(remote_error(
            "test.worker.remote.result_invalid",
            "executor suite name must be bounded readable text",
        ));
    }
    if !matches!(
        result.status,
        RemoteJobState::Passed | RemoteJobState::Failed
    ) {
        return Err(remote_error(
            "test.worker.remote.result_invalid",
            "executor result status must be passed or failed",
        ));
    }
    let counts = &result.scenarios;
    let total = counts
        .passed
        .checked_add(counts.failed)
        .and_then(|value| value.checked_add(counts.timed_out))
        .and_then(|value| value.checked_add(counts.cancelled))
        .ok_or_else(|| {
            remote_error(
                "test.worker.remote.result_invalid",
                "executor scenario counts overflowed",
            )
        })?;
    let non_passed = total - counts.passed;
    if total == 0
        || (result.status == RemoteJobState::Passed && non_passed != 0)
        || (result.status == RemoteJobState::Failed && non_passed == 0)
    {
        return Err(remote_error(
            "test.worker.remote.result_invalid",
            "executor status does not match its scenario counts",
        ));
    }
    if result.report.is_empty() || result.report.len() as u64 > max_report_bytes {
        return Err(remote_error(
            "test.worker.remote.report_size_invalid",
            "executor report size is outside the worker limit",
        ));
    }
    if result.media_type.is_empty()
        || result.media_type.len() > 128
        || !result
            .media_type
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
    {
        return Err(remote_error(
            "test.worker.remote.result_invalid",
            "executor report media type is invalid",
        ));
    }
    Ok(())
}

fn validate_executor_error(error: RemoteWorkerError) -> RemoteWorkerError {
    let code_valid = validate_token(&error.code, "executor error code").is_ok();
    let message_valid = !error.message.trim().is_empty()
        && error.message.len() <= 2_048
        && !error.message.contains('\0');
    if code_valid && message_valid {
        error
    } else {
        RemoteWorkerError::new(
            "test.worker.remote.executor_error_invalid",
            "remote executor returned an invalid error envelope",
            false,
        )
    }
}
