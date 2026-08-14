use std::{collections::BTreeSet, path::Path};

use super::{
    persistence::{self, StoredArtifactIndex},
    RemoteRetentionPolicy,
};
use crate::remote::{
    service::{ServiceShared, ServiceState},
    RemoteWorkerError,
};

const RETRY_DELAY_MS: u64 = 1_000;

pub(in crate::remote) async fn wait_until_due(shared: &ServiceShared) {
    let deadline_ms = {
        let state = shared.state.lock().await;
        next_deadline_ms(
            &state,
            &shared.artifact_descriptor.retention,
            shared.clock.now_ms(),
        )
    };
    match deadline_ms {
        Some(deadline_ms) => shared.clock.sleep_until_ms(deadline_ms).await,
        None => std::future::pending::<()>().await,
    }
}

pub(in crate::remote) async fn enforce_retention(shared: &ServiceShared) {
    let now_ms = shared.clock.now_ms();
    let mut state = shared.state.lock().await;
    match enforce_retention_state(
        &shared.root,
        &shared.artifact_descriptor.retention,
        &mut state,
        now_ms,
    )
    .await
    {
        Ok(()) => state.retention_error = None,
        Err(error) => state.retention_error = Some(error),
    }
}

fn next_deadline_ms(
    state: &ServiceState,
    policy: &RemoteRetentionPolicy,
    now_ms: u64,
) -> Option<u64> {
    if let Some(error) = &state.retention_error {
        return error
            .retryable
            .then(|| now_ms.saturating_add(RETRY_DELAY_MS));
    }
    state
        .jobs
        .values()
        .filter_map(|record| {
            let finished_at_ms = record.snapshot.finished_at_ms?;
            let index = record.artifacts.as_ref()?;
            let index_deadline = finished_at_ms.saturating_add(policy.max_index_age_ms);
            Some(if index.retained() {
                index_deadline.min(finished_at_ms.saturating_add(policy.max_retention_age_ms))
            } else {
                index_deadline
            })
        })
        .min()
}

pub(in crate::remote) async fn enforce_retention_state(
    root: &Path,
    policy: &RemoteRetentionPolicy,
    state: &mut ServiceState,
    now_ms: u64,
) -> Result<(), RemoteWorkerError> {
    if state
        .jobs
        .values()
        .any(|record| record.snapshot.state.terminal() && record.artifacts.is_none())
    {
        return Err(index_unavailable());
    }
    let mut terminal = terminal_job_ids(state);
    let expired_indexes = terminal
        .iter()
        .filter(|(_, finished_at_ms, _)| {
            now_ms.saturating_sub(*finished_at_ms) >= policy.max_index_age_ms
        })
        .map(|(job_id, _, _)| job_id.clone())
        .collect::<BTreeSet<_>>();
    let mut remove = expired_indexes;
    let excess = terminal
        .len()
        .saturating_sub(remove.len())
        .saturating_sub(policy.max_indexed_jobs as usize);
    let additional = terminal
        .iter()
        .filter(|(job_id, _, _)| !remove.contains(job_id))
        .take(excess)
        .map(|(job_id, _, _)| job_id.clone())
        .collect::<Vec<_>>();
    remove.extend(additional);
    for job_id in remove {
        persistence::remove_indexed_job(root, &job_id).await?;
        remove_record(state, &job_id)?;
    }

    terminal = terminal_job_ids(state);
    let mut retained_count = terminal
        .iter()
        .filter(|(_, _, index)| index.retained())
        .count();
    let mut retained_bytes = terminal
        .iter()
        .filter(|(_, _, index)| index.retained())
        .try_fold(0_u64, |total, (_, _, index)| {
            total.checked_add(index.retained_bytes).ok_or_else(|| {
                artifact_error(
                    "test.worker.artifact.size_overflow",
                    "retained payload byte count overflowed",
                )
            })
        })?;
    for (job_id, finished_at_ms, index) in terminal {
        if !index.retained() {
            continue;
        }
        let expired = now_ms.saturating_sub(finished_at_ms) >= policy.max_retention_age_ms;
        let over_count = retained_count > policy.max_retained_jobs as usize;
        let over_bytes = retained_bytes > policy.max_retained_bytes;
        if !(expired || over_count || over_bytes) {
            continue;
        }
        let snapshot = state
            .jobs
            .get(&job_id)
            .ok_or_else(index_inconsistent)?
            .snapshot
            .clone();
        let pruned = persistence::prune_payload(root, &snapshot, &index).await?;
        let record = state.jobs.get_mut(&job_id).ok_or_else(index_inconsistent)?;
        record.artifacts = Some(pruned);
        retained_count = retained_count.saturating_sub(1);
        retained_bytes = retained_bytes.saturating_sub(index.retained_bytes);
    }
    Ok(())
}

fn terminal_job_ids(state: &ServiceState) -> Vec<(String, u64, StoredArtifactIndex)> {
    let mut jobs = state
        .jobs
        .iter()
        .filter_map(|(job_id, record)| {
            let finished_at_ms = record.snapshot.finished_at_ms?;
            let index = record.artifacts.clone()?;
            record
                .snapshot
                .state
                .terminal()
                .then(|| (job_id.clone(), finished_at_ms, index))
        })
        .collect::<Vec<_>>();
    jobs.sort_by(|left, right| (left.1, &left.0).cmp(&(right.1, &right.0)));
    jobs
}

fn remove_record(state: &mut ServiceState, job_id: &str) -> Result<(), RemoteWorkerError> {
    let record = state.jobs.remove(job_id).ok_or_else(index_inconsistent)?;
    match state.dispatches.remove(&record.snapshot.dispatch_id) {
        Some(bound_job_id) if bound_job_id == job_id => Ok(()),
        _ => Err(index_inconsistent()),
    }
}

fn index_unavailable() -> RemoteWorkerError {
    artifact_error(
        "test.worker.artifact.index_unavailable",
        "terminal job does not have a durable artifact index",
    )
}

fn index_inconsistent() -> RemoteWorkerError {
    artifact_error(
        "test.worker.artifact.index_inconsistent",
        "artifact index and worker identity maps are inconsistent",
    )
}

fn artifact_error(code: &'static str, message: impl Into<String>) -> RemoteWorkerError {
    RemoteWorkerError::new(code, message, false)
}
