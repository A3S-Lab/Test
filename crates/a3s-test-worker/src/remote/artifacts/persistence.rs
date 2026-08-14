use std::{
    collections::BTreeSet,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

use super::{
    files::{artifact_error, io_error, is_link_like, sync_directory, validate_relative_path},
    payload, RemoteArtifactFileDescriptor, RemoteArtifactKind, RemotePayloadState,
    MAX_ARTIFACTS_PER_JOB, MAX_RETENTION_BYTES,
};
use crate::remote::{
    admission::validate_digest, RemoteJobSnapshot, RemoteRunSummary, RemoteWorkerError,
};

const ARTIFACT_INDEX_PROTOCOL: &str = "a3s.test.remote-artifact-index/1";
const MAX_INDEX_BYTES: u64 = 4 * 1024 * 1024;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum StoredPayloadState {
    Retained,
    Pruning,
    Pruned,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::remote) struct StoredArtifactIndex {
    protocol: String,
    pub indexed_at_ms: u64,
    state: StoredPayloadState,
    pub retained_bytes: u64,
    pub files: Vec<RemoteArtifactFileDescriptor>,
}

impl StoredArtifactIndex {
    pub(super) fn payload_state(&self) -> RemotePayloadState {
        match self.state {
            StoredPayloadState::Retained => RemotePayloadState::Retained,
            StoredPayloadState::Pruning | StoredPayloadState::Pruned => RemotePayloadState::Pruned,
        }
    }

    pub(super) fn artifact_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }

    pub(super) fn artifact_count(&self) -> u32 {
        u32::try_from(self.files.len()).unwrap_or(u32::MAX)
    }

    pub(super) fn retained(&self) -> bool {
        self.state == StoredPayloadState::Retained
    }
}

pub(in crate::remote) async fn recover_staging(root: &Path) -> Result<(), RemoteWorkerError> {
    let staging = root.join("staging");
    let mut directory = fs::read_dir(&staging).await.map_err(io_error)?;
    while let Some(entry) = directory.next_entry().await.map_err(io_error)? {
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            artifact_error(
                "test.worker.artifact.staging_invalid",
                "artifact staging entry is not portable UTF-8",
            )
        })?;
        if !(name.ends_with(".staging") || name.ends_with(".gc")) {
            return Err(artifact_error(
                "test.worker.artifact.staging_invalid",
                "artifact staging directory contains an unknown entry",
            ));
        }
        let metadata = fs::symlink_metadata(entry.path()).await.map_err(io_error)?;
        if is_link_like(&metadata) || !metadata.is_dir() {
            return Err(artifact_error(
                "test.worker.artifact.staging_invalid",
                "artifact staging entry is a link or non-directory",
            ));
        }
        fs::remove_dir_all(entry.path()).await.map_err(io_error)?;
    }
    sync_directory(&staging).await
}

pub(in crate::remote) async fn load_or_create(
    root: &Path,
    snapshot: &RemoteJobSnapshot,
    indexed_at_ms: u64,
) -> Result<StoredArtifactIndex, RemoteWorkerError> {
    let path = index_path(root, &snapshot.job_id);
    let index = match fs::try_exists(&path).await.map_err(io_error)? {
        true => read_index(&path).await?,
        false => return finalize(root, snapshot, indexed_at_ms).await,
    };
    validate_index_shape(&index, snapshot)?;
    match index.state {
        StoredPayloadState::Retained => {
            let rebuilt = build_index(root, snapshot, index.indexed_at_ms).await?;
            if rebuilt != index {
                return Err(artifact_error(
                    "test.worker.artifact.index_mismatch",
                    "retained artifact bytes do not match their persisted index",
                ));
            }
            Ok(index)
        }
        StoredPayloadState::Pruning => finish_prune(root, snapshot, index).await,
        StoredPayloadState::Pruned => {
            payload::ensure_absent(root, &snapshot.job_id).await?;
            Ok(index)
        }
    }
}

pub(in crate::remote) async fn finalize(
    root: &Path,
    snapshot: &RemoteJobSnapshot,
    indexed_at_ms: u64,
) -> Result<StoredArtifactIndex, RemoteWorkerError> {
    let index = prepare(root, snapshot, indexed_at_ms).await?;
    persist_prepared(root, &snapshot.job_id, &index).await?;
    Ok(index)
}

pub(in crate::remote) async fn prepare(
    root: &Path,
    snapshot: &RemoteJobSnapshot,
    indexed_at_ms: u64,
) -> Result<StoredArtifactIndex, RemoteWorkerError> {
    if snapshot.result.is_none() {
        payload::remove_orphan_report(root, &snapshot.job_id).await?;
    }
    match build_index(root, snapshot, indexed_at_ms).await {
        Ok(index) => Ok(index),
        Err(_) if snapshot.result.is_none() => {
            payload::discard(root, &snapshot.job_id).await?;
            Ok(StoredArtifactIndex {
                protocol: ARTIFACT_INDEX_PROTOCOL.to_string(),
                indexed_at_ms,
                state: StoredPayloadState::Pruned,
                retained_bytes: 0,
                files: Vec::new(),
            })
        }
        Err(error) => Err(error),
    }
}

pub(in crate::remote) async fn persist_prepared(
    root: &Path,
    job_id: &str,
    index: &StoredArtifactIndex,
) -> Result<(), RemoteWorkerError> {
    write_index(root, job_id, index).await
}

pub(super) async fn prune_payload(
    root: &Path,
    snapshot: &RemoteJobSnapshot,
    index: &StoredArtifactIndex,
) -> Result<StoredArtifactIndex, RemoteWorkerError> {
    if !index.retained() {
        return Ok(index.clone());
    }
    let mut pruning = index.clone();
    pruning.state = StoredPayloadState::Pruning;
    write_index(root, &snapshot.job_id, &pruning).await?;
    finish_prune(root, snapshot, pruning).await
}

pub(super) async fn remove_indexed_job(root: &Path, job_id: &str) -> Result<(), RemoteWorkerError> {
    let source = root.join("jobs").join(job_id);
    let metadata = fs::symlink_metadata(&source).await.map_err(io_error)?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(artifact_error(
            "test.worker.artifact.job_invalid",
            "indexed job path is a link or non-directory",
        ));
    }
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let staged =
        root.join("staging")
            .join(format!("{job_id}.{}.{}.gc", std::process::id(), counter));
    fs::rename(&source, &staged).await.map_err(io_error)?;
    sync_directory(&root.join("jobs")).await?;
    sync_directory(&root.join("staging")).await?;
    fs::remove_dir_all(&staged).await.map_err(io_error)?;
    sync_directory(&root.join("staging")).await
}

async fn build_index(
    root: &Path,
    snapshot: &RemoteJobSnapshot,
    indexed_at_ms: u64,
) -> Result<StoredArtifactIndex, RemoteWorkerError> {
    let scan = payload::scan(root, snapshot).await?;
    Ok(StoredArtifactIndex {
        protocol: ARTIFACT_INDEX_PROTOCOL.to_string(),
        indexed_at_ms,
        state: StoredPayloadState::Retained,
        retained_bytes: scan.retained_bytes,
        files: scan.files,
    })
}

async fn finish_prune(
    root: &Path,
    snapshot: &RemoteJobSnapshot,
    mut index: StoredArtifactIndex,
) -> Result<StoredArtifactIndex, RemoteWorkerError> {
    payload::discard(root, &snapshot.job_id).await?;
    index.state = StoredPayloadState::Pruned;
    index.retained_bytes = 0;
    write_index(root, &snapshot.job_id, &index).await?;
    Ok(index)
}

fn validate_index_shape(
    index: &StoredArtifactIndex,
    snapshot: &RemoteJobSnapshot,
) -> Result<(), RemoteWorkerError> {
    if index.protocol != ARTIFACT_INDEX_PROTOCOL
        || index.retained_bytes > MAX_RETENTION_BYTES
        || (index.state == StoredPayloadState::Pruned && index.retained_bytes != 0)
        || !valid_files(&index.files, snapshot.result.as_ref(), index.retained_bytes)
    {
        return Err(artifact_error(
            "test.worker.artifact.index_invalid",
            "persisted artifact index is outside its admitted shape",
        ));
    }
    Ok(())
}

fn valid_files(
    files: &[RemoteArtifactFileDescriptor],
    result: Option<&RemoteRunSummary>,
    retained_bytes: u64,
) -> bool {
    let mut evidence_paths = BTreeSet::new();
    let mut artifact_bytes = 0_u64;
    let mut reports = 0_u32;
    let mut evidence = 0_u32;
    for (position, file) in files.iter().enumerate() {
        if file.bytes == 0
            || file.bytes > MAX_RETENTION_BYTES
            || validate_digest(&file.sha256, "artifact digest").is_err()
            || file.media_type.is_empty()
            || file.media_type.len() > 128
            || !file.media_type.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return false;
        }
        artifact_bytes = match artifact_bytes.checked_add(file.bytes) {
            Some(bytes) if bytes <= MAX_RETENTION_BYTES => bytes,
            _ => return false,
        };
        match file.kind {
            RemoteArtifactKind::Report => {
                reports += 1;
                if position != 0 || file.path.is_some() {
                    return false;
                }
            }
            RemoteArtifactKind::Evidence => {
                evidence += 1;
                let Some(path) = file.path.as_deref() else {
                    return false;
                };
                if validate_relative_path(path).is_err()
                    || !evidence_paths.insert(path.to_ascii_lowercase())
                {
                    return false;
                }
            }
        }
    }
    let report_matches = match result {
        Some(result) => files.first().is_some_and(|file| {
            file.kind == RemoteArtifactKind::Report
                && file.sha256 == result.report.sha256
                && file.bytes == result.report.bytes
                && file.media_type == result.report.media_type
        }),
        None => reports == 0,
    };
    reports == u32::from(result.is_some())
        && evidence <= MAX_ARTIFACTS_PER_JOB
        && report_matches
        && (retained_bytes == 0 || artifact_bytes <= retained_bytes)
        && files.windows(2).all(|pair| {
            pair[0].kind == RemoteArtifactKind::Report
                || pair[0].path.as_deref() < pair[1].path.as_deref()
        })
}

async fn read_index(path: &Path) -> Result<StoredArtifactIndex, RemoteWorkerError> {
    let metadata = fs::symlink_metadata(path).await.map_err(io_error)?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > MAX_INDEX_BYTES {
        return Err(artifact_error(
            "test.worker.artifact.index_invalid",
            "artifact index is a link, non-file, or oversized",
        ));
    }
    let bytes = fs::read(path).await.map_err(io_error)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        artifact_error(
            "test.worker.artifact.index_invalid",
            format!("failed to decode persisted artifact index: {error}"),
        )
    })
}

async fn write_index(
    root: &Path,
    job_id: &str,
    index: &StoredArtifactIndex,
) -> Result<(), RemoteWorkerError> {
    let bytes = serde_json::to_vec(index).map_err(|error| {
        artifact_error(
            "test.worker.artifact.index_invalid",
            format!("failed to encode artifact index: {error}"),
        )
    })?;
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return Err(artifact_error(
            "test.worker.artifact.index_too_large",
            "artifact index exceeds its persistence bound",
        ));
    }
    write_atomic(&index_path(root, job_id), &bytes).await
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), RemoteWorkerError> {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            artifact_error(
                "test.worker.artifact.path_invalid",
                "artifact index path has no portable file name",
            )
        })?;
    let temporary = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        counter
    ));
    let result = async {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .await
            .map_err(io_error)?;
        file.write_all(bytes).await.map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        fs::rename(&temporary, path).await.map_err(io_error)?;
        sync_directory(path.parent().ok_or_else(|| {
            artifact_error(
                "test.worker.artifact.path_invalid",
                "artifact index path has no parent",
            )
        })?)
        .await
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    result
}

fn index_path(root: &Path, job_id: &str) -> std::path::PathBuf {
    root.join("jobs").join(job_id).join("artifact-index.json")
}
