use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use tokio::{
    sync::{mpsc, watch, Mutex},
    task::JoinHandle,
    time::Instant,
};
use tokio_util::sync::CancellationToken;

use crate::WorkerSurface;

use super::{
    admission::{remote_error, submission_request_digest, validate_storage_key, validate_token},
    persistence::{self, PersistedJobDefinition},
    RemoteJobSnapshot, RemoteJobState, RemoteScenarioCounts, RemoteWorkerCommand,
    RemoteWorkerDescriptor, RemoteWorkerError, RemoteWorkerOutcome, RemoteWorkerRequest,
    RemoteWorkerResponse,
};

#[derive(Clone, Debug)]
pub struct RemoteWorkerServiceConfig {
    pub state_root: PathBuf,
    pub descriptor: RemoteWorkerDescriptor,
}

impl RemoteWorkerServiceConfig {
    #[must_use]
    pub fn new(state_root: PathBuf, descriptor: RemoteWorkerDescriptor) -> Self {
        Self {
            state_root,
            descriptor,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RemoteExecutionJob {
    job_id: String,
    dispatch_id: String,
    request_digest: String,
    input_root: PathBuf,
    manifest_path: PathBuf,
    artifacts_root: PathBuf,
    deadline_ms: u64,
    max_parallel_scenarios: u16,
    required_surfaces: Vec<WorkerSurface>,
}

impl RemoteExecutionJob {
    #[must_use]
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    #[must_use]
    pub fn dispatch_id(&self) -> &str {
        &self.dispatch_id
    }

    #[must_use]
    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }

    #[must_use]
    pub fn input_root(&self) -> &Path {
        &self.input_root
    }

    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    #[must_use]
    pub fn artifacts_root(&self) -> &Path {
        &self.artifacts_root
    }

    #[must_use]
    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }

    #[must_use]
    pub fn max_parallel_scenarios(&self) -> u16 {
        self.max_parallel_scenarios
    }

    #[must_use]
    pub fn required_surfaces(&self) -> &[WorkerSurface] {
        &self.required_surfaces
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteExecutionResult {
    pub run_id: String,
    pub suite: String,
    pub status: RemoteJobState,
    pub scenarios: RemoteScenarioCounts,
    pub report: Vec<u8>,
    pub media_type: String,
}

#[async_trait]
pub trait RemoteJobExecutor: Send + Sync {
    async fn execute(
        &self,
        job: RemoteExecutionJob,
        cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError>;
}

#[derive(Clone)]
pub struct RemoteWorkerService {
    shared: Arc<ServiceShared>,
    worker_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    _shutdown_on_drop: Arc<ShutdownOnDrop>,
}

struct ShutdownOnDrop(CancellationToken);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl RemoteWorkerService {
    pub async fn open(
        config: RemoteWorkerServiceConfig,
        executor: Arc<dyn RemoteJobExecutor>,
    ) -> Result<Self, RemoteWorkerError> {
        let RemoteWorkerServiceConfig {
            state_root,
            descriptor,
        } = config;
        descriptor.validate()?;
        persistence::prepare_state_root(&state_root).await?;
        let state_root = tokio::fs::canonicalize(&state_root)
            .await
            .map_err(|error| {
                RemoteWorkerError::new(
                    "test.worker.remote.state_root_invalid",
                    format!("failed to resolve remote worker state root: {error}"),
                    true,
                )
            })?;
        let state_lock = persistence::acquire_state_lock(&state_root).await?;
        persistence::bind_descriptor(&state_root, &descriptor).await?;
        let clock = ServiceClock::new()?;
        let loaded =
            persistence::load_jobs(&state_root, descriptor.limits.max_report_bytes).await?;
        let mut state = ServiceState::default();
        for loaded_job in loaded {
            if state.jobs.contains_key(&loaded_job.snapshot.job_id)
                || state
                    .dispatches
                    .contains_key(&loaded_job.snapshot.dispatch_id)
            {
                return Err(remote_error(
                    "test.worker.remote.state_identity_conflict",
                    "persisted remote jobs contain duplicate identities",
                ));
            }
            let mut snapshot = loaded_job.snapshot;
            let mut sequence = loaded_job.sequence;
            if !snapshot.state.terminal() {
                let now_ms = clock.now_ms();
                snapshot.state = RemoteJobState::Interrupted;
                snapshot.finished_at_ms = Some(now_ms);
                snapshot.result = None;
                snapshot.error = Some(RemoteWorkerError::new(
                    "test.worker.remote.restart_interrupted",
                    "remote job was interrupted before worker restart recovery",
                    true,
                ));
                sequence = sequence.checked_add(1).ok_or_else(|| {
                    remote_error(
                        "test.worker.remote.state_sequence_overflow",
                        "persisted remote job event sequence overflowed",
                    )
                })?;
                persistence::append_event(
                    &state_root,
                    &snapshot.job_id,
                    sequence,
                    now_ms,
                    &snapshot,
                )
                .await?;
            }
            let (lease_tx, _) = watch::channel(snapshot.lease_expires_at_ms);
            state
                .dispatches
                .insert(snapshot.dispatch_id.clone(), snapshot.job_id.clone());
            state.jobs.insert(
                snapshot.job_id.clone(),
                JobRecord {
                    snapshot,
                    sequence,
                    definition: loaded_job.definition,
                    execution: None,
                    cancellation: CancellationToken::new(),
                    lease_tx,
                },
            );
        }

        let queue_capacity = usize::from(descriptor.limits.max_queued_jobs) + 1;
        let (queue_tx, queue_rx) = mpsc::channel(queue_capacity);
        let shared = Arc::new(ServiceShared {
            root: state_root,
            descriptor,
            executor,
            clock,
            state: Mutex::new(state),
            submission_lock: Mutex::new(()),
            queue_tx,
            shutdown: CancellationToken::new(),
            _state_lock: state_lock,
        });
        let worker_shared = Arc::clone(&shared);
        let task = tokio::spawn(async move {
            super::runtime::worker_loop(worker_shared, queue_rx).await;
        });
        Ok(Self {
            _shutdown_on_drop: Arc::new(ShutdownOnDrop(shared.shutdown.clone())),
            shared,
            worker_task: Arc::new(Mutex::new(Some(task))),
        })
    }

    #[must_use]
    pub fn descriptor(&self) -> &RemoteWorkerDescriptor {
        &self.shared.descriptor
    }

    #[must_use]
    pub fn now_ms(&self) -> u64 {
        self.shared.clock.now_ms()
    }

    pub async fn submit(
        &self,
        submission: super::RemoteJobSubmission,
    ) -> Result<RemoteJobSnapshot, RemoteWorkerError> {
        let _submission_guard = self.shared.submission_lock.lock().await;
        let request_digest =
            submission_request_digest(&submission, &self.shared.descriptor.limits)?;
        {
            let state = self.shared.state.lock().await;
            ensure_open(&state)?;
            if let Some(job_id) = state.dispatches.get(&submission.dispatch_id) {
                let existing = state.jobs.get(job_id).ok_or_else(state_index_error)?;
                if existing.snapshot.job_id == submission.job_id
                    && existing.snapshot.request_digest == request_digest
                {
                    return Ok(existing.snapshot.clone());
                }
                return Err(remote_error(
                    "test.worker.remote.dispatch_conflict",
                    "dispatch ID is already bound to a different immutable request",
                ));
            }
            if let Some(existing) = state.jobs.get(&submission.job_id) {
                if existing.snapshot.dispatch_id == submission.dispatch_id
                    && existing.snapshot.request_digest == request_digest
                {
                    return Ok(existing.snapshot.clone());
                }
                return Err(remote_error(
                    "test.worker.remote.job_conflict",
                    "job ID is already bound to a different immutable dispatch",
                ));
            }
        }

        let admitted = submission.admit(self.now_ms(), &self.shared.descriptor)?;
        {
            let state = self.shared.state.lock().await;
            ensure_open(&state)?;
            if state.queued_jobs >= usize::from(self.shared.descriptor.limits.max_queued_jobs) {
                return Err(RemoteWorkerError::new(
                    "test.worker.remote.queue_full",
                    "remote worker queue has reached its admitted bound",
                    true,
                ));
            }
        }

        let submitted_at_ms = self.now_ms();
        if admitted.deadline_ms() <= submitted_at_ms
            || admitted.lease_expires_at_ms() <= submitted_at_ms
        {
            return Err(remote_error(
                "test.worker.remote.admission_expired",
                "remote job expired while admission was in progress",
            ));
        }
        let snapshot = RemoteJobSnapshot {
            job_id: admitted.job_id().to_string(),
            dispatch_id: admitted.dispatch_id().to_string(),
            request_digest: admitted.request_digest().to_string(),
            state: RemoteJobState::Queued,
            submitted_at_ms,
            started_at_ms: None,
            finished_at_ms: None,
            deadline_ms: admitted.deadline_ms(),
            lease_expires_at_ms: admitted.lease_expires_at_ms(),
            result: None,
            error: None,
        };
        let definition =
            persistence::initialize_job(&self.shared.root, &admitted, &snapshot).await?;
        let execution = execution_job(&self.shared.root, &definition);
        let (lease_tx, _) = watch::channel(snapshot.lease_expires_at_ms);
        let job_id = snapshot.job_id.clone();
        {
            let mut state = self.shared.state.lock().await;
            ensure_open(&state)?;
            state.queued_jobs += 1;
            state
                .dispatches
                .insert(snapshot.dispatch_id.clone(), job_id.clone());
            state.jobs.insert(
                job_id.clone(),
                JobRecord {
                    snapshot: snapshot.clone(),
                    sequence: 1,
                    definition,
                    execution: Some(execution),
                    cancellation: CancellationToken::new(),
                    lease_tx,
                },
            );
        }
        if self.shared.queue_tx.send(job_id.clone()).await.is_err() {
            super::runtime::fail_dispatch(&self.shared, &job_id).await;
            return Err(RemoteWorkerError::new(
                "test.worker.remote.worker_unavailable",
                "remote worker execution loop is unavailable",
                true,
            ));
        }
        Ok(snapshot)
    }

    pub async fn status(
        &self,
        job_id: &str,
        dispatch_id: &str,
    ) -> Result<RemoteJobSnapshot, RemoteWorkerError> {
        validate_storage_key(job_id, "job ID")?;
        validate_storage_key(dispatch_id, "dispatch ID")?;
        let state = self.shared.state.lock().await;
        let record = bound_job(&state, job_id, dispatch_id)?;
        Ok(record.snapshot.clone())
    }

    pub async fn renew_lease(
        &self,
        job_id: &str,
        dispatch_id: &str,
        lease_expires_at_ms: u64,
    ) -> Result<RemoteJobSnapshot, RemoteWorkerError> {
        validate_storage_key(job_id, "job ID")?;
        validate_storage_key(dispatch_id, "dispatch ID")?;
        let now_ms = self.now_ms();
        let mut state = self.shared.state.lock().await;
        ensure_open(&state)?;
        let record = bound_job_mut(&mut state, job_id, dispatch_id)?;
        if record.snapshot.state.terminal() || record.snapshot.state == RemoteJobState::Cancelling {
            return Err(remote_error(
                "test.worker.remote.lease_state_invalid",
                "lease cannot be renewed for a cancelling or terminal job",
            ));
        }
        if lease_expires_at_ms == record.snapshot.lease_expires_at_ms {
            return Ok(record.snapshot.clone());
        }
        if lease_expires_at_ms < record.snapshot.lease_expires_at_ms {
            return Err(remote_error(
                "test.worker.remote.lease_regression",
                "renewed lease cannot shorten the current lease",
            ));
        }
        if lease_expires_at_ms <= now_ms
            || lease_expires_at_ms > record.snapshot.deadline_ms
            || lease_expires_at_ms.saturating_sub(now_ms)
                > self.shared.descriptor.limits.max_lease_ms
        {
            return Err(remote_error(
                "test.worker.remote.lease_invalid",
                "renewed lease must be future, bounded, and no later than the deadline",
            ));
        }
        let mut next = record.snapshot.clone();
        next.lease_expires_at_ms = lease_expires_at_ms;
        persist_transition(&self.shared, record, next, now_ms).await?;
        let _ = record.lease_tx.send(lease_expires_at_ms);
        Ok(record.snapshot.clone())
    }

    pub async fn cancel(
        &self,
        job_id: &str,
        dispatch_id: &str,
        reason: Option<String>,
    ) -> Result<RemoteJobSnapshot, RemoteWorkerError> {
        validate_storage_key(job_id, "job ID")?;
        validate_storage_key(dispatch_id, "dispatch ID")?;
        let reason = validate_cancel_reason(reason)?;
        let now_ms = self.now_ms();
        let mut state = self.shared.state.lock().await;
        let was_queued;
        let cancellation;
        {
            let record = bound_job_mut(&mut state, job_id, dispatch_id)?;
            if record.snapshot.state.terminal() {
                return Ok(record.snapshot.clone());
            }
            if record.snapshot.state == RemoteJobState::Cancelling {
                return Ok(record.snapshot.clone());
            }
            was_queued = record.snapshot.state == RemoteJobState::Queued;
            let mut next = record.snapshot.clone();
            next.state = if was_queued {
                RemoteJobState::Cancelled
            } else {
                RemoteJobState::Cancelling
            };
            next.finished_at_ms = was_queued.then_some(now_ms);
            next.error = Some(RemoteWorkerError::new(
                "test.worker.remote.cancelled",
                reason,
                false,
            ));
            persist_transition(&self.shared, record, next, now_ms).await?;
            cancellation = record.cancellation.clone();
        }
        if was_queued {
            state.queued_jobs = state.queued_jobs.saturating_sub(1);
        }
        let snapshot = state
            .jobs
            .get(job_id)
            .ok_or_else(state_index_error)?
            .snapshot
            .clone();
        drop(state);
        cancellation.cancel();
        Ok(snapshot)
    }

    pub async fn handle(&self, request: RemoteWorkerRequest) -> RemoteWorkerResponse {
        let request_id = if validate_token(&request.request_id, "request ID").is_ok() {
            request.request_id.clone()
        } else {
            "invalid-request".to_string()
        };
        let outcome = match request.validate() {
            Err(error) => RemoteWorkerOutcome::Error { error },
            Ok(()) => match request.command {
                RemoteWorkerCommand::Inspect => RemoteWorkerOutcome::Descriptor {
                    worker: self.shared.descriptor.clone(),
                },
                RemoteWorkerCommand::Submit { job } => outcome(self.submit(job).await),
                RemoteWorkerCommand::Status {
                    job_id,
                    dispatch_id,
                } => outcome(self.status(&job_id, &dispatch_id).await),
                RemoteWorkerCommand::RenewLease {
                    job_id,
                    dispatch_id,
                    lease_expires_at_ms,
                } => outcome(
                    self.renew_lease(&job_id, &dispatch_id, lease_expires_at_ms)
                        .await,
                ),
                RemoteWorkerCommand::Cancel {
                    job_id,
                    dispatch_id,
                    reason,
                } => outcome(self.cancel(&job_id, &dispatch_id, reason).await),
            },
        };
        RemoteWorkerResponse::new(request_id, outcome)
    }

    pub async fn shutdown(&self) -> Result<(), RemoteWorkerError> {
        let _submission_guard = self.shared.submission_lock.lock().await;
        {
            let mut state = self.shared.state.lock().await;
            if !state.closed {
                state.closed = true;
                self.shared.shutdown.cancel();
            }
        }
        let Some(mut task) = self.worker_task.lock().await.take() else {
            return Ok(());
        };
        let timeout = Duration::from_millis(self.shared.descriptor.limits.cleanup_timeout_ms);
        match tokio::time::timeout(timeout, &mut task).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(RemoteWorkerError::new(
                "test.worker.remote.worker_join_failed",
                format!("remote worker execution loop failed: {error}"),
                true,
            )),
            Err(_) => {
                task.abort();
                let _ = task.await;
                Err(RemoteWorkerError::new(
                    "test.worker.remote.shutdown_timeout",
                    "remote worker shutdown exceeded its cleanup bound",
                    true,
                ))
            }
        }
    }
}

pub(super) struct ServiceShared {
    pub root: PathBuf,
    pub descriptor: RemoteWorkerDescriptor,
    pub executor: Arc<dyn RemoteJobExecutor>,
    pub clock: ServiceClock,
    pub state: Mutex<ServiceState>,
    pub submission_lock: Mutex<()>,
    pub queue_tx: mpsc::Sender<String>,
    pub shutdown: CancellationToken,
    pub _state_lock: std::fs::File,
}

#[derive(Default)]
pub(super) struct ServiceState {
    pub jobs: BTreeMap<String, JobRecord>,
    pub dispatches: BTreeMap<String, String>,
    pub queued_jobs: usize,
    pub closed: bool,
}

pub(super) struct JobRecord {
    pub snapshot: RemoteJobSnapshot,
    pub sequence: u64,
    #[allow(dead_code)]
    pub definition: PersistedJobDefinition,
    pub execution: Option<RemoteExecutionJob>,
    pub cancellation: CancellationToken,
    pub lease_tx: watch::Sender<u64>,
}

#[derive(Clone)]
pub(super) struct ServiceClock {
    epoch_ms: u64,
    origin: Instant,
}

impl ServiceClock {
    fn new() -> Result<Self, RemoteWorkerError> {
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                remote_error(
                    "test.worker.remote.clock_invalid",
                    "system clock is earlier than the Unix epoch",
                )
            })?
            .as_millis()
            .try_into()
            .map_err(|_| {
                remote_error(
                    "test.worker.remote.clock_invalid",
                    "system clock does not fit the remote worker time range",
                )
            })?;
        Ok(Self {
            epoch_ms,
            origin: Instant::now(),
        })
    }

    pub fn now_ms(&self) -> u64 {
        let elapsed = self.origin.elapsed().as_millis();
        self.epoch_ms
            .saturating_add(u64::try_from(elapsed).unwrap_or(u64::MAX))
    }

    pub async fn sleep_until_ms(&self, deadline_ms: u64) {
        let duration = Duration::from_millis(deadline_ms.saturating_sub(self.now_ms()));
        tokio::time::sleep(duration).await;
    }
}

pub(super) async fn persist_transition(
    shared: &ServiceShared,
    record: &mut JobRecord,
    snapshot: RemoteJobSnapshot,
    now_ms: u64,
) -> Result<(), RemoteWorkerError> {
    let sequence = record.sequence.checked_add(1).ok_or_else(|| {
        remote_error(
            "test.worker.remote.state_sequence_overflow",
            "remote job event sequence overflowed",
        )
    })?;
    persistence::append_event(&shared.root, &snapshot.job_id, sequence, now_ms, &snapshot).await?;
    record.sequence = sequence;
    record.snapshot = snapshot;
    Ok(())
}

fn execution_job(root: &Path, definition: &PersistedJobDefinition) -> RemoteExecutionJob {
    let input_root = persistence::job_directory(root, &definition.job_id).join("input");
    RemoteExecutionJob {
        job_id: definition.job_id.clone(),
        dispatch_id: definition.dispatch_id.clone(),
        request_digest: definition.request_digest.clone(),
        manifest_path: input_root.join(&definition.manifest),
        artifacts_root: persistence::job_directory(root, &definition.job_id).join("artifacts"),
        input_root,
        deadline_ms: definition.deadline_ms,
        max_parallel_scenarios: definition.max_parallel_scenarios,
        required_surfaces: definition.required_surfaces.clone(),
    }
}

fn ensure_open(state: &ServiceState) -> Result<(), RemoteWorkerError> {
    if state.closed {
        return Err(RemoteWorkerError::new(
            "test.worker.remote.service_closed",
            "remote worker service is shutting down",
            true,
        ));
    }
    Ok(())
}

fn bound_job<'a>(
    state: &'a ServiceState,
    job_id: &str,
    dispatch_id: &str,
) -> Result<&'a JobRecord, RemoteWorkerError> {
    let record = state.jobs.get(job_id).ok_or_else(|| {
        remote_error(
            "test.worker.remote.job_not_found",
            "remote job does not exist on this worker",
        )
    })?;
    if record.snapshot.dispatch_id != dispatch_id {
        return Err(remote_error(
            "test.worker.remote.dispatch_mismatch",
            "remote job is bound to a different dispatch ID",
        ));
    }
    Ok(record)
}

fn bound_job_mut<'a>(
    state: &'a mut ServiceState,
    job_id: &str,
    dispatch_id: &str,
) -> Result<&'a mut JobRecord, RemoteWorkerError> {
    let record = state.jobs.get_mut(job_id).ok_or_else(|| {
        remote_error(
            "test.worker.remote.job_not_found",
            "remote job does not exist on this worker",
        )
    })?;
    if record.snapshot.dispatch_id != dispatch_id {
        return Err(remote_error(
            "test.worker.remote.dispatch_mismatch",
            "remote job is bound to a different dispatch ID",
        ));
    }
    Ok(record)
}

fn validate_cancel_reason(reason: Option<String>) -> Result<String, RemoteWorkerError> {
    let reason = reason.unwrap_or_else(|| "remote job cancellation was requested".to_string());
    if reason.trim().is_empty()
        || reason.len() > 1_024
        || reason
            .chars()
            .any(|character| character == '\0' || (character.is_control() && character != '\n'))
    {
        return Err(remote_error(
            "test.worker.remote.cancel_reason_invalid",
            "cancellation reason must be bounded readable text",
        ));
    }
    Ok(reason)
}

fn outcome(result: Result<RemoteJobSnapshot, RemoteWorkerError>) -> RemoteWorkerOutcome {
    match result {
        Ok(job) => RemoteWorkerOutcome::Job { job },
        Err(error) => RemoteWorkerOutcome::Error { error },
    }
}

fn state_index_error() -> RemoteWorkerError {
    remote_error(
        "test.worker.remote.state_index_invalid",
        "remote worker in-memory identity index is inconsistent",
    )
}
