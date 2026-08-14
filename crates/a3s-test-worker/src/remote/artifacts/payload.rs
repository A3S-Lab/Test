use std::{collections::BTreeSet, path::Path};

use sha2::{Digest, Sha256};
use tokio::{fs, io::AsyncReadExt};

use super::{
    files::{
        artifact_error, io_error, is_link_like, remove_entry, sync_directory,
        validate_relative_path,
    },
    RemoteArtifactFileDescriptor, RemoteArtifactKind, MAX_ARTIFACTS_PER_JOB, MAX_RETENTION_BYTES,
};
use crate::remote::{RemoteJobSnapshot, RemoteWorkerError};

const READ_BUFFER_BYTES: usize = 64 * 1024;

pub(super) struct PayloadScan {
    pub retained_bytes: u64,
    pub files: Vec<RemoteArtifactFileDescriptor>,
}

pub(super) async fn scan(
    root: &Path,
    snapshot: &RemoteJobSnapshot,
) -> Result<PayloadScan, RemoteWorkerError> {
    let job_root = root.join("jobs").join(&snapshot.job_id);
    let mut files = Vec::new();
    if let Some(result) = &snapshot.result {
        let report_path = job_root.join("report.bin");
        let report = read_regular_contained(&report_path, &job_root, result.report.bytes).await?;
        if sha256_bytes(&report) != result.report.sha256 {
            return Err(artifact_error(
                "test.worker.artifact.report_mismatch",
                "retained report does not match its terminal snapshot",
            ));
        }
        files.push(RemoteArtifactFileDescriptor {
            kind: RemoteArtifactKind::Report,
            path: None,
            sha256: result.report.sha256.clone(),
            bytes: result.report.bytes,
            media_type: result.report.media_type.clone(),
        });
    }

    let artifacts_root = job_root.join("artifacts");
    if fs::try_exists(&artifacts_root).await.map_err(io_error)? {
        files.append(&mut scan_evidence(&artifacts_root).await?);
    }
    if files.len() > MAX_ARTIFACTS_PER_JOB as usize + 1 {
        return Err(artifact_error(
            "test.worker.artifact.count_exceeded",
            "job produced more evidence files than the artifact protocol admits",
        ));
    }

    let input_bytes = directory_bytes(&job_root.join("input")).await?;
    let artifact_bytes = files.iter().try_fold(0_u64, |total, file| {
        total.checked_add(file.bytes).ok_or_else(|| {
            artifact_error(
                "test.worker.artifact.size_overflow",
                "retained artifact size overflowed",
            )
        })
    })?;
    let retained_bytes = input_bytes.checked_add(artifact_bytes).ok_or_else(|| {
        artifact_error(
            "test.worker.artifact.size_overflow",
            "retained payload size overflowed",
        )
    })?;
    if retained_bytes > MAX_RETENTION_BYTES {
        return Err(artifact_error(
            "test.worker.artifact.job_too_large",
            "one job produced more retained bytes than the hard artifact bound",
        ));
    }
    Ok(PayloadScan {
        retained_bytes,
        files,
    })
}

pub(super) async fn read_verified_chunk(
    root: &Path,
    job_id: &str,
    descriptor: &RemoteArtifactFileDescriptor,
    offset: u64,
    max_bytes: u32,
) -> Result<(Vec<u8>, bool), RemoteWorkerError> {
    let job_root = root.join("jobs").join(job_id);
    let (path, containment_root) = match descriptor.kind {
        RemoteArtifactKind::Report => (job_root.join("report.bin"), job_root),
        RemoteArtifactKind::Evidence => {
            let artifacts_root = job_root.join("artifacts");
            (
                artifacts_root.join(descriptor.path.as_deref().unwrap_or_default()),
                artifacts_root,
            )
        }
    };
    let metadata = fs::symlink_metadata(&path).await.map_err(io_error)?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() != descriptor.bytes {
        return Err(artifact_error(
            "test.worker.artifact.file_invalid",
            "artifact is a link, non-regular file, or has changed size",
        ));
    }
    let canonical_root = fs::canonicalize(containment_root).await.map_err(io_error)?;
    let canonical_path = fs::canonicalize(path).await.map_err(io_error)?;
    if !canonical_path.starts_with(canonical_root) {
        return Err(artifact_error(
            "test.worker.artifact.path_invalid",
            "artifact resolved outside its retained root",
        ));
    }

    let mut file = fs::File::open(canonical_path).await.map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    let mut chunk = Vec::with_capacity(max_bytes as usize);
    let mut position = 0_u64;
    loop {
        let read = file.read(&mut buffer).await.map_err(io_error)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        let block_start = position;
        let block_end = position.saturating_add(read as u64);
        let chunk_end = offset.saturating_add(u64::from(max_bytes));
        let copy_start = offset.max(block_start);
        let copy_end = chunk_end.min(block_end);
        if copy_start < copy_end {
            let start = usize::try_from(copy_start - block_start).map_err(|_| {
                artifact_error(
                    "test.worker.artifact.chunk_invalid",
                    "artifact chunk start does not fit this platform",
                )
            })?;
            let end = usize::try_from(copy_end - block_start).map_err(|_| {
                artifact_error(
                    "test.worker.artifact.chunk_invalid",
                    "artifact chunk end does not fit this platform",
                )
            })?;
            chunk.extend_from_slice(&buffer[start..end]);
        }
        position = block_end;
    }
    if position != descriptor.bytes
        || format!("sha256:{:x}", digest.finalize()) != descriptor.sha256
    {
        return Err(artifact_error(
            "test.worker.artifact.digest_mismatch",
            "artifact bytes no longer match their retained digest",
        ));
    }
    let eof = offset.saturating_add(chunk.len() as u64) == descriptor.bytes;
    Ok((chunk, eof))
}

pub(super) async fn discard(root: &Path, job_id: &str) -> Result<(), RemoteWorkerError> {
    let job_root = root.join("jobs").join(job_id);
    remove_entry(&job_root.join("input"), true).await?;
    remove_entry(&job_root.join("artifacts"), true).await?;
    remove_entry(&job_root.join("report.bin"), false).await?;
    sync_directory(&job_root).await
}

pub(super) async fn ensure_absent(root: &Path, job_id: &str) -> Result<(), RemoteWorkerError> {
    let job_root = root.join("jobs").join(job_id);
    for path in [
        job_root.join("input"),
        job_root.join("artifacts"),
        job_root.join("report.bin"),
    ] {
        if fs::try_exists(path).await.map_err(io_error)? {
            return Err(artifact_error(
                "test.worker.artifact.pruned_payload_present",
                "artifact index says pruned but retained payload still exists",
            ));
        }
    }
    Ok(())
}

pub(super) async fn remove_orphan_report(
    root: &Path,
    job_id: &str,
) -> Result<(), RemoteWorkerError> {
    let path = root.join("jobs").join(job_id).join("report.bin");
    if !fs::try_exists(&path).await.map_err(io_error)? {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&path).await.map_err(io_error)?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err(artifact_error(
            "test.worker.artifact.report_invalid",
            "orphan report is a link or non-regular file",
        ));
    }
    fs::remove_file(path).await.map_err(io_error)
}

async fn scan_evidence(
    root: &Path,
) -> Result<Vec<RemoteArtifactFileDescriptor>, RemoteWorkerError> {
    let canonical_root = fs::canonicalize(root).await.map_err(io_error)?;
    let mut pending = vec![(root.to_path_buf(), String::new())];
    let mut files = Vec::new();
    let mut portable = BTreeSet::new();
    let mut total = 0_u64;
    while let Some((directory_path, prefix)) = pending.pop() {
        let metadata = fs::symlink_metadata(&directory_path)
            .await
            .map_err(io_error)?;
        if is_link_like(&metadata) || !metadata.is_dir() {
            return Err(artifact_error(
                "test.worker.artifact.path_invalid",
                "artifact tree contains a link or non-directory component",
            ));
        }
        let mut directory = fs::read_dir(&directory_path).await.map_err(io_error)?;
        while let Some(entry) = directory.next_entry().await.map_err(io_error)? {
            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| {
                artifact_error(
                    "test.worker.artifact.path_invalid",
                    "artifact path is not portable UTF-8",
                )
            })?;
            let relative = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            };
            validate_relative_path(&relative)?;
            let metadata = fs::symlink_metadata(entry.path()).await.map_err(io_error)?;
            if is_link_like(&metadata) {
                return Err(artifact_error(
                    "test.worker.artifact.path_invalid",
                    "artifact tree contains a link or reparse point",
                ));
            }
            if metadata.is_dir() {
                pending.push((entry.path(), relative));
                continue;
            }
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(artifact_error(
                    "test.worker.artifact.file_invalid",
                    "artifact tree contains an empty or non-regular file",
                ));
            }
            if !portable.insert(relative.to_ascii_lowercase()) {
                return Err(artifact_error(
                    "test.worker.artifact.path_collision",
                    "artifact paths collide on case-insensitive filesystems",
                ));
            }
            total = total.checked_add(metadata.len()).ok_or_else(|| {
                artifact_error(
                    "test.worker.artifact.size_overflow",
                    "artifact byte count overflowed",
                )
            })?;
            if total > MAX_RETENTION_BYTES {
                return Err(artifact_error(
                    "test.worker.artifact.job_too_large",
                    "one job produced more bytes than the hard artifact bound",
                ));
            }
            let canonical = fs::canonicalize(entry.path()).await.map_err(io_error)?;
            if !canonical.starts_with(&canonical_root) {
                return Err(artifact_error(
                    "test.worker.artifact.path_invalid",
                    "artifact resolved outside its job artifact root",
                ));
            }
            files.push(RemoteArtifactFileDescriptor {
                kind: RemoteArtifactKind::Evidence,
                path: Some(relative.clone()),
                sha256: sha256_file(&canonical, metadata.len()).await?,
                bytes: metadata.len(),
                media_type: media_type(&relative).to_string(),
            });
            if files.len() > MAX_ARTIFACTS_PER_JOB as usize {
                return Err(artifact_error(
                    "test.worker.artifact.count_exceeded",
                    "job produced more evidence files than the artifact protocol admits",
                ));
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

async fn directory_bytes(path: &Path) -> Result<u64, RemoteWorkerError> {
    if !fs::try_exists(path).await.map_err(io_error)? {
        return Ok(0);
    }
    let mut pending = vec![path.to_path_buf()];
    let mut total = 0_u64;
    while let Some(directory_path) = pending.pop() {
        let metadata = fs::symlink_metadata(&directory_path)
            .await
            .map_err(io_error)?;
        if is_link_like(&metadata) || !metadata.is_dir() {
            return Err(artifact_error(
                "test.worker.artifact.payload_invalid",
                "retained input contains a link or non-directory component",
            ));
        }
        let mut directory = fs::read_dir(directory_path).await.map_err(io_error)?;
        while let Some(entry) = directory.next_entry().await.map_err(io_error)? {
            let metadata = fs::symlink_metadata(entry.path()).await.map_err(io_error)?;
            if is_link_like(&metadata) {
                return Err(artifact_error(
                    "test.worker.artifact.payload_invalid",
                    "retained input contains a link or reparse point",
                ));
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                total = total.checked_add(metadata.len()).ok_or_else(|| {
                    artifact_error(
                        "test.worker.artifact.size_overflow",
                        "retained input byte count overflowed",
                    )
                })?;
                if total > MAX_RETENTION_BYTES {
                    return Err(artifact_error(
                        "test.worker.artifact.job_too_large",
                        "one job input exceeds the hard artifact bound",
                    ));
                }
            } else {
                return Err(artifact_error(
                    "test.worker.artifact.payload_invalid",
                    "retained input contains a non-regular entry",
                ));
            }
        }
    }
    Ok(total)
}

async fn read_regular_contained(
    path: &Path,
    root: &Path,
    expected_bytes: u64,
) -> Result<Vec<u8>, RemoteWorkerError> {
    let metadata = fs::symlink_metadata(path).await.map_err(io_error)?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() != expected_bytes {
        return Err(artifact_error(
            "test.worker.artifact.file_invalid",
            "artifact is a link, non-regular file, or has changed size",
        ));
    }
    let canonical_root = fs::canonicalize(root).await.map_err(io_error)?;
    let canonical_path = fs::canonicalize(path).await.map_err(io_error)?;
    if !canonical_path.starts_with(canonical_root) {
        return Err(artifact_error(
            "test.worker.artifact.path_invalid",
            "artifact resolved outside its retained root",
        ));
    }
    let capacity = usize::try_from(expected_bytes).map_err(|_| {
        artifact_error(
            "test.worker.artifact.file_invalid",
            "artifact size does not fit this platform",
        )
    })?;
    let file = fs::File::open(canonical_path).await.map_err(io_error)?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(expected_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .await
        .map_err(io_error)?;
    if bytes.len() as u64 != expected_bytes {
        return Err(artifact_error(
            "test.worker.artifact.file_invalid",
            "artifact changed while it was being read",
        ));
    }
    Ok(bytes)
}

async fn sha256_file(path: &Path, expected_bytes: u64) -> Result<String, RemoteWorkerError> {
    let mut file = fs::File::open(path).await.map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).await.map_err(io_error)?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or_else(|| {
            artifact_error(
                "test.worker.artifact.size_overflow",
                "artifact byte count overflowed while hashing",
            )
        })?;
        digest.update(&buffer[..read]);
    }
    if total != expected_bytes {
        return Err(artifact_error(
            "test.worker.artifact.file_invalid",
            "artifact changed while it was being indexed",
        ));
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

fn media_type(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" | "har" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webm" => "video/webm",
        "zip" => "application/zip",
        "txt" | "log" | "vt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
