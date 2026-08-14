use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

use crate::WorkerSurface;

use super::{
    admission::{remote_error, sha256, validate_digest, validate_storage_key},
    AdmittedRemoteJob, RemoteJobSnapshot, RemoteWorkerDescriptor, RemoteWorkerError,
};

const REMOTE_JOB_STATE_PROTOCOL: &str = "a3s.test.remote-job-state/1";
const MAX_DEFINITION_BYTES: u64 = 32 * 1024;
const MAX_EVENT_BYTES: u64 = 128 * 1024;
const MAX_DESCRIPTOR_BYTES: u64 = 256 * 1024;
const MAX_EVENTS_PER_JOB: usize = 100_000;
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PersistedJobDefinition {
    protocol: String,
    pub job_id: String,
    pub dispatch_id: String,
    pub request_digest: String,
    pub manifest: String,
    pub deadline_ms: u64,
    pub max_parallel_scenarios: u16,
    pub required_surfaces: Vec<WorkerSurface>,
}

impl PersistedJobDefinition {
    pub(super) fn from_admitted(job: &AdmittedRemoteJob) -> Self {
        Self {
            protocol: REMOTE_JOB_STATE_PROTOCOL.to_string(),
            job_id: job.job_id().to_string(),
            dispatch_id: job.dispatch_id().to_string(),
            request_digest: job.request_digest().to_string(),
            manifest: job.manifest().to_string(),
            deadline_ms: job.deadline_ms(),
            max_parallel_scenarios: job.max_parallel_scenarios(),
            required_surfaces: job.required_surfaces().to_vec(),
        }
    }

    fn validate(&self) -> Result<(), RemoteWorkerError> {
        if self.protocol != REMOTE_JOB_STATE_PROTOCOL {
            return Err(state_error(
                "test.worker.remote.state_protocol_unsupported",
                "persisted remote job uses an unsupported state protocol",
            ));
        }
        validate_storage_key(&self.job_id, "persisted job ID")?;
        validate_storage_key(&self.dispatch_id, "persisted dispatch ID")?;
        validate_digest(&self.request_digest, "persisted request digest")?;
        if self.manifest.is_empty() || self.manifest.len() > 256 {
            return Err(state_error(
                "test.worker.remote.state_invalid",
                "persisted manifest path is invalid",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedJobEvent {
    protocol: String,
    sequence: u64,
    recorded_at_ms: u64,
    snapshot: RemoteJobSnapshot,
}

pub(super) struct LoadedJob {
    pub definition: PersistedJobDefinition,
    pub snapshot: RemoteJobSnapshot,
    pub sequence: u64,
}

pub(super) async fn prepare_state_root(root: &Path) -> Result<(), RemoteWorkerError> {
    if !root.is_absolute() {
        return Err(state_error(
            "test.worker.remote.state_root_invalid",
            "remote worker state root must be absolute",
        ));
    }
    ensure_private_directory(root).await?;
    ensure_private_directory(&root.join("jobs")).await?;
    ensure_private_directory(&root.join("staging")).await
}

pub(super) async fn acquire_state_lock(root: &Path) -> Result<std::fs::File, RemoteWorkerError> {
    let lock_path = root.join("worker.lock");
    tokio::task::spawn_blocking(move || {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(io_error)?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
            RemoteWorkerError::new(
                "test.worker.remote.state_locked",
                format!("remote worker state root is already locked: {error}"),
                true,
            )
        })?;
        Ok(file)
    })
    .await
    .map_err(|error| {
        RemoteWorkerError::new(
            "test.worker.remote.state_lock_failed",
            format!("remote worker state lock task failed: {error}"),
            true,
        )
    })?
}

pub(super) async fn bind_descriptor(
    root: &Path,
    descriptor: &RemoteWorkerDescriptor,
) -> Result<(), RemoteWorkerError> {
    let path = root.join("worker.json");
    if fs::try_exists(&path).await.map_err(io_error)? {
        let persisted: RemoteWorkerDescriptor =
            read_bounded_json(&path, MAX_DESCRIPTOR_BYTES).await?;
        persisted.validate()?;
        if persisted != *descriptor {
            return Err(state_error(
                "test.worker.remote.state_worker_mismatch",
                "remote worker state root is bound to a different descriptor",
            ));
        }
        return Ok(());
    }
    let bytes = serde_json::to_vec(descriptor).map_err(|error| {
        state_error(
            "test.worker.remote.state_encode_failed",
            format!("failed to encode remote worker descriptor: {error}"),
        )
    })?;
    if bytes.len() as u64 > MAX_DESCRIPTOR_BYTES {
        return Err(state_error(
            "test.worker.remote.state_descriptor_too_large",
            "remote worker descriptor exceeds its persistence limit",
        ));
    }
    write_atomic_new(&path, &bytes).await
}

pub(super) async fn initialize_job(
    root: &Path,
    job: &AdmittedRemoteJob,
    snapshot: &RemoteJobSnapshot,
) -> Result<PersistedJobDefinition, RemoteWorkerError> {
    let definition = PersistedJobDefinition::from_admitted(job);
    let stage_name = format!(
        "{}.{}.staging",
        job.request_digest().trim_start_matches("sha256:"),
        std::process::id()
    );
    let stage = root.join("staging").join(stage_name);
    let final_path = job_directory(root, job.job_id());
    if fs::try_exists(&final_path).await.map_err(io_error)? {
        return Err(state_error(
            "test.worker.remote.state_conflict",
            "remote job state already exists on disk",
        ));
    }
    match fs::create_dir(&stage).await {
        Ok(()) => set_private_permissions(&stage).await?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(state_error(
                "test.worker.remote.state_staging_conflict",
                "remote job staging state already exists",
            ));
        }
        Err(error) => return Err(io_error(error)),
    }

    let result = async {
        let input_root = stage.join("input");
        ensure_private_directory(&input_root).await?;
        for file in job.files() {
            write_input_file(&input_root, file.path(), file.bytes()).await?;
        }
        write_new_json(&stage.join("definition.json"), &definition).await?;
        ensure_private_directory(&stage.join("events")).await?;
        append_event_in_directory(&stage, 1, snapshot.submitted_at_ms, snapshot).await?;
        sync_directory(&stage).await?;
        fs::rename(&stage, &final_path).await.map_err(io_error)?;
        sync_directory(&root.join("jobs")).await?;
        sync_directory(&root.join("staging")).await?;
        Ok::<(), RemoteWorkerError>(())
    }
    .await;

    if let Err(error) = result {
        cleanup_staging_directory(&stage).await;
        return Err(error);
    }
    Ok(definition)
}

pub(super) async fn append_event(
    root: &Path,
    job_id: &str,
    sequence: u64,
    recorded_at_ms: u64,
    snapshot: &RemoteJobSnapshot,
) -> Result<(), RemoteWorkerError> {
    if sequence == 0 || sequence > MAX_EVENTS_PER_JOB as u64 {
        return Err(state_error(
            "test.worker.remote.state_event_count_invalid",
            "remote job event count exceeds its persistence bound",
        ));
    }
    append_event_in_directory(
        &job_directory(root, job_id),
        sequence,
        recorded_at_ms,
        snapshot,
    )
    .await
}

pub(super) async fn persist_report(
    root: &Path,
    job_id: &str,
    report: &[u8],
) -> Result<(), RemoteWorkerError> {
    let path = job_directory(root, job_id).join("report.bin");
    write_atomic(&path, report).await
}

pub(super) async fn load_jobs(
    root: &Path,
    max_report_bytes: u64,
) -> Result<Vec<LoadedJob>, RemoteWorkerError> {
    let jobs_root = root.join("jobs");
    let mut directory = fs::read_dir(&jobs_root).await.map_err(io_error)?;
    let mut paths = Vec::new();
    while let Some(entry) = directory.next_entry().await.map_err(io_error)? {
        paths.push(entry.path());
    }
    paths.sort();

    let mut jobs = Vec::with_capacity(paths.len());
    for path in paths {
        let metadata = fs::symlink_metadata(&path).await.map_err(io_error)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(state_error(
                "test.worker.remote.state_entry_invalid",
                "remote worker jobs root contains a non-directory entry",
            ));
        }
        let directory_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                state_error(
                    "test.worker.remote.state_entry_invalid",
                    "remote job directory name is not portable UTF-8",
                )
            })?;
        validate_storage_key(directory_name, "persisted job directory")?;
        let definition: PersistedJobDefinition =
            read_bounded_json(&path.join("definition.json"), MAX_DEFINITION_BYTES).await?;
        definition.validate()?;
        if definition.job_id != directory_name {
            return Err(state_error(
                "test.worker.remote.state_identity_mismatch",
                "persisted job definition does not match its directory",
            ));
        }
        let (snapshot, sequence) = load_last_event(&path, &definition).await?;
        validate_report(&path, &snapshot, max_report_bytes).await?;
        jobs.push(LoadedJob {
            definition,
            snapshot,
            sequence,
        });
    }
    Ok(jobs)
}

pub(super) fn job_directory(root: &Path, job_id: &str) -> PathBuf {
    root.join("jobs").join(job_id)
}

async fn load_last_event(
    job_directory: &Path,
    definition: &PersistedJobDefinition,
) -> Result<(RemoteJobSnapshot, u64), RemoteWorkerError> {
    let events_root = job_directory.join("events");
    let metadata = fs::symlink_metadata(&events_root).await.map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(state_error(
            "test.worker.remote.state_events_invalid",
            "persisted remote job events path is not a directory",
        ));
    }
    let mut directory = fs::read_dir(&events_root).await.map_err(io_error)?;
    let mut events = Vec::new();
    while let Some(entry) = directory.next_entry().await.map_err(io_error)? {
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            state_error(
                "test.worker.remote.state_event_invalid",
                "persisted event name is not portable UTF-8",
            )
        })?;
        if name.starts_with('.') && name.ends_with(".tmp") {
            continue;
        }
        let sequence = name
            .strip_suffix(".json")
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                state_error(
                    "test.worker.remote.state_event_invalid",
                    "persisted event name is not a canonical sequence",
                )
            })?;
        events.push((sequence, entry.path()));
    }
    events.sort_by_key(|(sequence, _)| *sequence);
    if events.is_empty() || events.len() > MAX_EVENTS_PER_JOB {
        return Err(state_error(
            "test.worker.remote.state_event_count_invalid",
            "persisted job event count is outside the admitted bounds",
        ));
    }

    let mut last = None;
    for (index, (sequence, path)) in events.into_iter().enumerate() {
        if sequence != index as u64 + 1 {
            return Err(state_error(
                "test.worker.remote.state_event_sequence_invalid",
                "persisted job events are not contiguous",
            ));
        }
        let event: PersistedJobEvent = read_bounded_json(&path, MAX_EVENT_BYTES).await?;
        if event.protocol != REMOTE_JOB_STATE_PROTOCOL || event.sequence != sequence {
            return Err(state_error(
                "test.worker.remote.state_event_invalid",
                "persisted job event envelope is invalid",
            ));
        }
        validate_snapshot_binding(&event.snapshot, definition)?;
        last = Some((event.snapshot, sequence));
    }
    last.ok_or_else(|| {
        state_error(
            "test.worker.remote.state_event_count_invalid",
            "persisted job has no state events",
        )
    })
}

fn validate_snapshot_binding(
    snapshot: &RemoteJobSnapshot,
    definition: &PersistedJobDefinition,
) -> Result<(), RemoteWorkerError> {
    if snapshot.job_id != definition.job_id
        || snapshot.dispatch_id != definition.dispatch_id
        || snapshot.request_digest != definition.request_digest
        || snapshot.deadline_ms != definition.deadline_ms
    {
        return Err(state_error(
            "test.worker.remote.state_identity_mismatch",
            "persisted job snapshot does not match its immutable definition",
        ));
    }
    Ok(())
}

async fn validate_report(
    job_directory: &Path,
    snapshot: &RemoteJobSnapshot,
    max_report_bytes: u64,
) -> Result<(), RemoteWorkerError> {
    let Some(summary) = &snapshot.result else {
        return Ok(());
    };
    validate_digest(&summary.report.sha256, "persisted report digest")?;
    if summary.report.bytes == 0 || summary.report.bytes > max_report_bytes {
        return Err(state_error(
            "test.worker.remote.state_report_invalid",
            "persisted report size is outside the worker limit",
        ));
    }
    let report_path = job_directory.join("report.bin");
    let report = read_bounded(&report_path, max_report_bytes).await?;
    if report.len() as u64 != summary.report.bytes || sha256(&report) != summary.report.sha256 {
        return Err(state_error(
            "test.worker.remote.state_report_mismatch",
            "persisted report does not match its recorded descriptor",
        ));
    }
    Ok(())
}

async fn append_event_in_directory(
    job_directory: &Path,
    sequence: u64,
    recorded_at_ms: u64,
    snapshot: &RemoteJobSnapshot,
) -> Result<(), RemoteWorkerError> {
    let event = PersistedJobEvent {
        protocol: REMOTE_JOB_STATE_PROTOCOL.to_string(),
        sequence,
        recorded_at_ms,
        snapshot: snapshot.clone(),
    };
    let bytes = serde_json::to_vec(&event).map_err(|error| {
        state_error(
            "test.worker.remote.state_encode_failed",
            format!("failed to encode remote job state event: {error}"),
        )
    })?;
    if bytes.len() as u64 > MAX_EVENT_BYTES {
        return Err(state_error(
            "test.worker.remote.state_event_too_large",
            "remote job state event exceeds its persistence limit",
        ));
    }
    let path = job_directory
        .join("events")
        .join(format!("{sequence:020}.json"));
    write_atomic_new(&path, &bytes).await
}

async fn write_input_file(
    input_root: &Path,
    relative_path: &str,
    bytes: &[u8],
) -> Result<(), RemoteWorkerError> {
    let components = relative_path.split('/').collect::<Vec<_>>();
    let (file_name, directories) = components.split_last().ok_or_else(|| {
        state_error(
            "test.worker.remote.input_path_invalid",
            "input path is empty",
        )
    })?;
    let mut parent = input_root.to_path_buf();
    for component in directories {
        parent.push(component);
        ensure_private_directory(&parent).await?;
    }
    let path = parent.join(file_name);
    write_new_bytes(&path, bytes).await
}

async fn write_new_json<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), RemoteWorkerError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        state_error(
            "test.worker.remote.state_encode_failed",
            format!("failed to encode remote job state: {error}"),
        )
    })?;
    write_new_bytes(path, &bytes).await
}

async fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), RemoteWorkerError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await
        .map_err(io_error)?;
    file.write_all(bytes).await.map_err(io_error)?;
    file.sync_all().await.map_err(io_error)?;
    drop(file);
    sync_parent_directory(path).await
}

async fn write_atomic_new(path: &Path, bytes: &[u8]) -> Result<(), RemoteWorkerError> {
    if fs::try_exists(path).await.map_err(io_error)? {
        return Err(state_error(
            "test.worker.remote.state_event_conflict",
            "remote job state event sequence already exists",
        ));
    }
    write_atomic(path, bytes).await
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), RemoteWorkerError> {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            state_error(
                "test.worker.remote.state_path_invalid",
                "remote job state path is not portable UTF-8",
            )
        })?;
    let temporary = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        counter
    ));
    let result = async {
        write_new_bytes(&temporary, bytes).await?;
        fs::rename(&temporary, path).await.map_err(io_error)?;
        sync_parent_directory(path).await
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    result
}

async fn read_bounded_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    max_bytes: u64,
) -> Result<T, RemoteWorkerError> {
    let bytes = read_bounded(path, max_bytes).await?;
    serde_json::from_slice(&bytes).map_err(|error| {
        state_error(
            "test.worker.remote.state_decode_failed",
            format!("failed to decode persisted remote job state: {error}"),
        )
    })
}

async fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, RemoteWorkerError> {
    let metadata = fs::symlink_metadata(path).await.map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes {
        return Err(state_error(
            "test.worker.remote.state_file_invalid",
            "persisted remote job state file is unsafe or oversized",
        ));
    }
    fs::read(path).await.map_err(io_error)
}

async fn ensure_private_directory(path: &Path) -> Result<(), RemoteWorkerError> {
    let mut created = false;
    match fs::create_dir(path).await {
        Ok(()) => created = true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).await.map_err(io_error)?;
            created = true;
        }
        Err(error) => return Err(io_error(error)),
    }
    let metadata = fs::symlink_metadata(path).await.map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(state_error(
            "test.worker.remote.state_directory_invalid",
            "remote worker state directory is not a real directory",
        ));
    }
    set_private_permissions(path).await?;
    if created {
        sync_parent_directory(path).await?;
    }
    Ok(())
}

#[cfg(unix)]
async fn set_private_permissions(path: &Path) -> Result<(), RemoteWorkerError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .await
        .map_err(io_error)
}

#[cfg(not(unix))]
async fn set_private_permissions(_path: &Path) -> Result<(), RemoteWorkerError> {
    Ok(())
}

async fn cleanup_staging_directory(path: &Path) {
    if let Ok(metadata) = fs::symlink_metadata(path).await {
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            let _ = fs::remove_dir_all(path).await;
        }
    }
}

async fn sync_parent_directory(path: &Path) -> Result<(), RemoteWorkerError> {
    let parent = path.parent().ok_or_else(|| {
        state_error(
            "test.worker.remote.state_path_invalid",
            "remote job state path has no parent directory",
        )
    })?;
    sync_directory(parent).await
}

#[cfg(unix)]
async fn sync_directory(path: &Path) -> Result<(), RemoteWorkerError> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(io_error)
    })
    .await
    .map_err(|error| {
        RemoteWorkerError::new(
            "test.worker.remote.state_sync_failed",
            format!("remote worker state sync task failed: {error}"),
            true,
        )
    })?
}

#[cfg(not(unix))]
async fn sync_directory(_path: &Path) -> Result<(), RemoteWorkerError> {
    Ok(())
}

fn io_error(error: std::io::Error) -> RemoteWorkerError {
    RemoteWorkerError::new(
        "test.worker.remote.state_io_failed",
        format!("remote worker state I/O failed: {error}"),
        true,
    )
}

fn state_error(code: &'static str, message: impl Into<String>) -> RemoteWorkerError {
    remote_error(code, message)
}
