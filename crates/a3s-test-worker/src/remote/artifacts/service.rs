use std::collections::BTreeMap;

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use serde::{Deserialize, Serialize};

use super::{
    payload, persistence::StoredArtifactIndex, RemoteArtifactChunk, RemoteArtifactCommand,
    RemoteArtifactFileDescriptor, RemoteArtifactKind, RemoteArtifactOutcome, RemoteArtifactPage,
    RemoteArtifactRequest, RemoteArtifactResponse, RemoteArtifactSelector, RemoteReportIndexEntry,
    RemoteReportPage, RemoteReportQuery, MAX_ARTIFACT_CHUNK_BYTES, MAX_ARTIFACT_PAGE_SIZE,
    MAX_CURSOR_BYTES,
};
use crate::remote::{
    admission::{sha256, validate_digest, validate_storage_key, validate_token},
    service::{JobRecord, ServiceState},
    RemoteWorkerError, RemoteWorkerService,
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReportCursor {
    query_digest: String,
    finished_at_ms: u64,
    job_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactCursor {
    request_digest: String,
    offset: u32,
}

impl RemoteWorkerService {
    #[must_use]
    pub fn artifact_descriptor(&self) -> &super::RemoteArtifactDescriptor {
        &self.shared.artifact_descriptor
    }

    pub async fn list_reports(
        &self,
        query: RemoteReportQuery,
    ) -> Result<RemoteReportPage, RemoteWorkerError> {
        query.validate()?;
        let query_digest = report_query_digest(&query)?;
        let cursor = query
            .cursor
            .as_deref()
            .map(decode_cursor::<ReportCursor>)
            .transpose()?;
        if cursor
            .as_ref()
            .is_some_and(|cursor| cursor.query_digest != query_digest)
        {
            return Err(artifact_error(
                "test.worker.artifact.cursor_mismatch",
                "report cursor is bound to a different query",
            ));
        }

        let state = self.shared.state.lock().await;
        ensure_readable(&state)?;
        let limit = usize::from(query.limit);
        let mut selected = BTreeMap::new();
        for record in state.jobs.values() {
            let Some(index) = record.artifacts.as_ref() else {
                continue;
            };
            if !record.snapshot.state.terminal() || !report_matches(record, &query) {
                continue;
            }
            let key = report_key(record);
            if cursor
                .as_ref()
                .is_some_and(|cursor| key >= (cursor.finished_at_ms, cursor.job_id.as_str()))
            {
                continue;
            }
            selected.insert(key, (record, index));
            if selected.len() > limit + 1 {
                selected.pop_first();
            }
        }
        let has_more = selected.len() > limit;
        let reports = selected
            .into_iter()
            .rev()
            .take(limit)
            .map(|(_, (record, index))| report_entry(record, index))
            .collect::<Vec<_>>();
        let next_cursor = if has_more {
            reports
                .last()
                .map(|entry| ReportCursor {
                    query_digest,
                    finished_at_ms: entry.job.finished_at_ms.unwrap_or_default(),
                    job_id: entry.job.job_id.clone(),
                })
                .map(encode_cursor)
                .transpose()?
        } else {
            None
        };
        Ok(RemoteReportPage {
            reports,
            next_cursor,
        })
    }

    pub async fn list_artifacts(
        &self,
        job_id: &str,
        dispatch_id: &str,
        expected_request_digest: &str,
        limit: u16,
        cursor: Option<String>,
    ) -> Result<RemoteArtifactPage, RemoteWorkerError> {
        validate_binding_input(job_id, dispatch_id, expected_request_digest)?;
        if !(1..=MAX_ARTIFACT_PAGE_SIZE).contains(&limit) {
            return Err(artifact_error(
                "test.worker.artifact.query_invalid",
                "artifact page size is outside the protocol bound",
            ));
        }
        let cursor = cursor
            .as_deref()
            .map(decode_cursor::<ArtifactCursor>)
            .transpose()?;
        if cursor
            .as_ref()
            .is_some_and(|cursor| cursor.request_digest != expected_request_digest)
        {
            return Err(artifact_error(
                "test.worker.artifact.cursor_mismatch",
                "artifact cursor is bound to a different job request",
            ));
        }

        let state = self.shared.state.lock().await;
        ensure_readable(&state)?;
        let record = bound_record(&state, job_id, dispatch_id, expected_request_digest)?;
        let index = record.artifacts.as_ref().ok_or_else(index_unavailable)?;
        let offset = cursor
            .map(|cursor| usize::try_from(cursor.offset))
            .transpose()
            .map_err(|_| cursor_invalid())?
            .unwrap_or(0);
        if offset > index.files.len() {
            return Err(cursor_invalid());
        }
        let limit = usize::from(limit);
        let end = offset.saturating_add(limit).min(index.files.len());
        let artifacts = index.files[offset..end].to_vec();
        let next_cursor = (end < index.files.len())
            .then(|| {
                Ok(ArtifactCursor {
                    request_digest: expected_request_digest.to_string(),
                    offset: u32::try_from(end).map_err(|_| cursor_invalid())?,
                })
            })
            .transpose()?
            .map(encode_cursor)
            .transpose()?;
        Ok(RemoteArtifactPage {
            job_id: job_id.to_string(),
            dispatch_id: dispatch_id.to_string(),
            request_digest: expected_request_digest.to_string(),
            artifacts,
            next_cursor,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn read_artifact(
        &self,
        job_id: &str,
        dispatch_id: &str,
        expected_request_digest: &str,
        artifact: RemoteArtifactSelector,
        offset: u64,
        max_bytes: u32,
    ) -> Result<RemoteArtifactChunk, RemoteWorkerError> {
        validate_binding_input(job_id, dispatch_id, expected_request_digest)?;
        artifact.validate()?;
        if !(1..=MAX_ARTIFACT_CHUNK_BYTES).contains(&max_bytes) {
            return Err(artifact_error(
                "test.worker.artifact.chunk_invalid",
                "artifact chunk size is outside the protocol bound",
            ));
        }
        let descriptor = {
            let state = self.shared.state.lock().await;
            ensure_readable(&state)?;
            let record = bound_record(&state, job_id, dispatch_id, expected_request_digest)?;
            let index = record.artifacts.as_ref().ok_or_else(index_unavailable)?;
            if !index.retained() {
                return Err(artifact_error(
                    "test.worker.artifact.payload_pruned",
                    "artifact payload is outside the deployment retention window",
                ));
            }
            select_artifact(index, &artifact)?
        };
        if offset >= descriptor.bytes {
            return Err(artifact_error(
                "test.worker.artifact.chunk_invalid",
                "artifact chunk offset must be earlier than the retained file end",
            ));
        }
        let (bytes, eof) =
            payload::read_verified_chunk(&self.shared.root, job_id, &descriptor, offset, max_bytes)
                .await?;
        Ok(RemoteArtifactChunk {
            job_id: job_id.to_string(),
            dispatch_id: dispatch_id.to_string(),
            request_digest: expected_request_digest.to_string(),
            artifact: descriptor,
            offset,
            contents_base64: STANDARD.encode(bytes),
            eof,
        })
    }

    pub async fn handle_artifact(&self, request: RemoteArtifactRequest) -> RemoteArtifactResponse {
        let request_id = if validate_token(&request.request_id, "request ID").is_ok() {
            request.request_id.clone()
        } else {
            "invalid-request".to_string()
        };
        let outcome = match request.validate() {
            Err(error) => RemoteArtifactOutcome::Error { error },
            Ok(()) => match request.command {
                RemoteArtifactCommand::Inspect => RemoteArtifactOutcome::Descriptor {
                    service: self.shared.artifact_descriptor.clone(),
                },
                RemoteArtifactCommand::ListReports { query } => {
                    match self.list_reports(query).await {
                        Ok(page) => RemoteArtifactOutcome::Reports { page },
                        Err(error) => RemoteArtifactOutcome::Error { error },
                    }
                }
                RemoteArtifactCommand::ListArtifacts {
                    job_id,
                    dispatch_id,
                    expected_request_digest,
                    limit,
                    cursor,
                } => match self
                    .list_artifacts(
                        &job_id,
                        &dispatch_id,
                        &expected_request_digest,
                        limit,
                        cursor,
                    )
                    .await
                {
                    Ok(page) => RemoteArtifactOutcome::Artifacts { page },
                    Err(error) => RemoteArtifactOutcome::Error { error },
                },
                RemoteArtifactCommand::Read {
                    job_id,
                    dispatch_id,
                    expected_request_digest,
                    artifact,
                    offset,
                    max_bytes,
                } => match self
                    .read_artifact(
                        &job_id,
                        &dispatch_id,
                        &expected_request_digest,
                        artifact,
                        offset,
                        max_bytes,
                    )
                    .await
                {
                    Ok(chunk) => RemoteArtifactOutcome::Chunk { chunk },
                    Err(error) => RemoteArtifactOutcome::Error { error },
                },
            },
        };
        RemoteArtifactResponse::new(request_id, outcome)
    }
}

fn report_entry(record: &JobRecord, index: &StoredArtifactIndex) -> RemoteReportIndexEntry {
    RemoteReportIndexEntry {
        job: record.snapshot.clone(),
        payload_state: index.payload_state(),
        indexed_at_ms: index.indexed_at_ms,
        artifact_count: index.artifact_count(),
        artifact_bytes: index.artifact_bytes(),
    }
}

fn report_matches(record: &JobRecord, query: &RemoteReportQuery) -> bool {
    let finished_at_ms = record.snapshot.finished_at_ms.unwrap_or_default();
    query.states.binary_search(&record.snapshot.state).is_ok()
        && query.suite.as_ref().is_none_or(|suite| {
            record
                .snapshot
                .result
                .as_ref()
                .is_some_and(|run| &run.suite == suite)
        })
        && query.run_id.as_ref().is_none_or(|run_id| {
            record
                .snapshot
                .result
                .as_ref()
                .is_some_and(|run| &run.run_id == run_id)
        })
        && query
            .finished_after_ms
            .is_none_or(|after| finished_at_ms > after)
        && query
            .finished_before_ms
            .is_none_or(|before| finished_at_ms < before)
}

fn report_key(record: &JobRecord) -> (u64, &str) {
    (
        record.snapshot.finished_at_ms.unwrap_or_default(),
        record.snapshot.job_id.as_str(),
    )
}

fn bound_record<'a>(
    state: &'a ServiceState,
    job_id: &str,
    dispatch_id: &str,
    request_digest: &str,
) -> Result<&'a JobRecord, RemoteWorkerError> {
    let record = state.jobs.get(job_id).ok_or_else(|| {
        artifact_error(
            "test.worker.artifact.job_not_found",
            "artifact index does not contain the requested job",
        )
    })?;
    if record.snapshot.dispatch_id != dispatch_id {
        return Err(artifact_error(
            "test.worker.artifact.dispatch_mismatch",
            "artifact job is bound to a different dispatch ID",
        ));
    }
    if record.snapshot.request_digest != request_digest {
        return Err(artifact_error(
            "test.worker.artifact.request_mismatch",
            "artifact job is bound to a different request digest",
        ));
    }
    Ok(record)
}

fn select_artifact(
    index: &StoredArtifactIndex,
    selector: &RemoteArtifactSelector,
) -> Result<RemoteArtifactFileDescriptor, RemoteWorkerError> {
    match selector {
        RemoteArtifactSelector::Report { sha256 } => {
            let report = index
                .files
                .iter()
                .find(|file| file.kind == RemoteArtifactKind::Report)
                .ok_or_else(artifact_not_found)?;
            if &report.sha256 != sha256 {
                return Err(digest_mismatch());
            }
            Ok(report.clone())
        }
        RemoteArtifactSelector::Evidence { path, sha256 } => {
            let evidence = index
                .files
                .iter()
                .find(|file| {
                    file.kind == RemoteArtifactKind::Evidence
                        && file.path.as_deref() == Some(path.as_str())
                })
                .ok_or_else(artifact_not_found)?;
            if &evidence.sha256 != sha256 {
                return Err(digest_mismatch());
            }
            Ok(evidence.clone())
        }
    }
}

fn validate_binding_input(
    job_id: &str,
    dispatch_id: &str,
    request_digest: &str,
) -> Result<(), RemoteWorkerError> {
    validate_storage_key(job_id, "job ID")?;
    validate_storage_key(dispatch_id, "dispatch ID")?;
    validate_digest(request_digest, "request digest")
}

fn report_query_digest(query: &RemoteReportQuery) -> Result<String, RemoteWorkerError> {
    let mut canonical = query.clone();
    canonical.cursor = None;
    serde_json::to_vec(&canonical)
        .map(|bytes| sha256(&bytes))
        .map_err(|error| {
            artifact_error(
                "test.worker.artifact.query_invalid",
                format!("failed to encode report query: {error}"),
            )
        })
}

fn encode_cursor(value: impl Serialize) -> Result<String, RemoteWorkerError> {
    serde_json::to_vec(&value)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|error| {
            artifact_error(
                "test.worker.artifact.cursor_invalid",
                format!("failed to encode artifact cursor: {error}"),
            )
        })
}

fn decode_cursor<T: for<'de> Deserialize<'de>>(value: &str) -> Result<T, RemoteWorkerError> {
    if value.is_empty()
        || value.len() > MAX_CURSOR_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(cursor_invalid());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| cursor_invalid())?;
    if URL_SAFE_NO_PAD.encode(&bytes) != value {
        return Err(cursor_invalid());
    }
    serde_json::from_slice(&bytes).map_err(|_| cursor_invalid())
}

fn ensure_readable(state: &ServiceState) -> Result<(), RemoteWorkerError> {
    if state.closed {
        return Err(RemoteWorkerError::new(
            "test.worker.artifact.service_closed",
            "remote artifact service is shutting down",
            true,
        ));
    }
    Ok(())
}

fn artifact_not_found() -> RemoteWorkerError {
    artifact_error(
        "test.worker.artifact.not_found",
        "requested artifact is absent from the retained index",
    )
}

fn digest_mismatch() -> RemoteWorkerError {
    artifact_error(
        "test.worker.artifact.digest_mismatch",
        "requested artifact digest does not match the retained index",
    )
}

fn index_unavailable() -> RemoteWorkerError {
    artifact_error(
        "test.worker.artifact.index_unavailable",
        "terminal job does not have a durable artifact index",
    )
}

fn cursor_invalid() -> RemoteWorkerError {
    artifact_error(
        "test.worker.artifact.cursor_invalid",
        "artifact cursor is invalid or outside the retained index",
    )
}

fn artifact_error(code: &'static str, message: impl Into<String>) -> RemoteWorkerError {
    RemoteWorkerError::new(code, message, false)
}
