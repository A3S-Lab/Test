use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use super::REMOTE_ARTIFACT_PROTOCOL;
use crate::remote::{
    admission::{validate_digest, validate_storage_key, validate_token},
    RemoteJobSnapshot, RemoteJobState, RemoteWorkerDescriptor, RemoteWorkerError,
    RemoteWorkerIdentity,
};

pub const DEFAULT_MAX_RETAINED_JOBS: u32 = 256;
pub const DEFAULT_MAX_RETAINED_BYTES: u64 = 20 * 1024 * 1024 * 1024;
pub const DEFAULT_MAX_RETENTION_AGE_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
pub const DEFAULT_MAX_INDEXED_JOBS: u32 = 10_000;
pub const DEFAULT_MAX_INDEX_AGE_MS: u64 = 90 * 24 * 60 * 60 * 1_000;
pub const MAX_REPORT_PAGE_SIZE: u16 = 100;
pub const MAX_ARTIFACT_PAGE_SIZE: u16 = 256;
pub const MAX_ARTIFACT_CHUNK_BYTES: u32 = 1024 * 1024;
pub const MAX_ARTIFACTS_PER_JOB: u32 = 4_096;

const MIN_RETENTION_BYTES: u64 = 1024 * 1024;
pub(super) const MAX_RETENTION_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const MIN_RETENTION_AGE_MS: u64 = 1_000;
const MAX_RETENTION_AGE_MS: u64 = 365 * 24 * 60 * 60 * 1_000;
const MAX_INDEX_AGE_MS: u64 = 5 * 365 * 24 * 60 * 60 * 1_000;
pub(super) const MAX_CURSOR_BYTES: usize = 512;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRetentionPolicy {
    #[schemars(range(min = 1, max = 100000))]
    pub max_retained_jobs: u32,
    #[schemars(range(min = 1048576_u64, max = 1099511627776_u64))]
    pub max_retained_bytes: u64,
    #[schemars(range(min = 1000_u64, max = 31536000000_u64))]
    pub max_retention_age_ms: u64,
    #[schemars(range(min = 1, max = 1000000))]
    pub max_indexed_jobs: u32,
    #[schemars(range(min = 1000_u64, max = 157680000000_u64))]
    pub max_index_age_ms: u64,
}

impl Default for RemoteRetentionPolicy {
    fn default() -> Self {
        Self {
            max_retained_jobs: DEFAULT_MAX_RETAINED_JOBS,
            max_retained_bytes: DEFAULT_MAX_RETAINED_BYTES,
            max_retention_age_ms: DEFAULT_MAX_RETENTION_AGE_MS,
            max_indexed_jobs: DEFAULT_MAX_INDEXED_JOBS,
            max_index_age_ms: DEFAULT_MAX_INDEX_AGE_MS,
        }
    }
}

impl RemoteRetentionPolicy {
    pub fn validate(&self) -> Result<(), RemoteWorkerError> {
        let valid = (1..=100_000).contains(&self.max_retained_jobs)
            && (MIN_RETENTION_BYTES..=MAX_RETENTION_BYTES).contains(&self.max_retained_bytes)
            && (MIN_RETENTION_AGE_MS..=MAX_RETENTION_AGE_MS).contains(&self.max_retention_age_ms)
            && (self.max_retained_jobs..=1_000_000).contains(&self.max_indexed_jobs)
            && (self.max_retention_age_ms..=MAX_INDEX_AGE_MS).contains(&self.max_index_age_ms);
        if !valid {
            return Err(artifact_error(
                "test.worker.artifact.retention_invalid",
                "artifact retention limits are outside the reviewed bounds or are not ordered",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteArtifactLimits {
    #[schemars(range(min = 1, max = 100))]
    pub max_report_page_size: u16,
    #[schemars(range(min = 1, max = 256))]
    pub max_artifact_page_size: u16,
    #[schemars(range(min = 1, max = 1048576))]
    pub max_chunk_bytes: u32,
    #[schemars(range(min = 1, max = 4096))]
    pub max_artifacts_per_job: u32,
}

impl Default for RemoteArtifactLimits {
    fn default() -> Self {
        Self {
            max_report_page_size: MAX_REPORT_PAGE_SIZE,
            max_artifact_page_size: MAX_ARTIFACT_PAGE_SIZE,
            max_chunk_bytes: MAX_ARTIFACT_CHUNK_BYTES,
            max_artifacts_per_job: MAX_ARTIFACTS_PER_JOB,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteArtifactDescriptor {
    #[schemars(schema_with = "remote_artifact_protocol_field_schema")]
    pub protocol: String,
    pub worker: RemoteWorkerIdentity,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub inventory_digest: String,
    pub retention: RemoteRetentionPolicy,
    pub limits: RemoteArtifactLimits,
}

impl RemoteArtifactDescriptor {
    pub fn validate(&self) -> Result<(), RemoteWorkerError> {
        if self.protocol != REMOTE_ARTIFACT_PROTOCOL {
            return Err(artifact_error(
                "test.worker.artifact.protocol_unsupported",
                format!("unsupported remote artifact protocol {:?}", self.protocol),
            ));
        }
        validate_token(&self.worker.instance_id, "artifact worker instance ID")?;
        validate_digest(&self.worker.image_digest, "artifact worker image digest")?;
        validate_digest(&self.inventory_digest, "artifact inventory digest")?;
        self.retention.validate()?;
        if !(1..=MAX_REPORT_PAGE_SIZE).contains(&self.limits.max_report_page_size)
            || !(1..=MAX_ARTIFACT_PAGE_SIZE).contains(&self.limits.max_artifact_page_size)
            || !(1..=MAX_ARTIFACT_CHUNK_BYTES).contains(&self.limits.max_chunk_bytes)
            || !(1..=MAX_ARTIFACTS_PER_JOB).contains(&self.limits.max_artifacts_per_job)
        {
            return Err(artifact_error(
                "test.worker.artifact.limits_invalid",
                "artifact service limits are outside the reviewed bounds",
            ));
        }
        Ok(())
    }
}

impl RemoteWorkerDescriptor {
    #[must_use]
    pub fn artifact_descriptor(
        &self,
        retention: RemoteRetentionPolicy,
    ) -> RemoteArtifactDescriptor {
        RemoteArtifactDescriptor {
            protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
            worker: self.identity.clone(),
            inventory_digest: self.inventory_digest.clone(),
            retention,
            limits: RemoteArtifactLimits::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteArtifactRequest {
    #[schemars(schema_with = "remote_artifact_protocol_field_schema")]
    pub protocol: String,
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub command: RemoteArtifactCommand,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RemoteArtifactCommand {
    Inspect,
    ListReports {
        query: RemoteReportQuery,
    },
    ListArtifacts {
        #[schemars(length(min = 1, max = 128))]
        job_id: String,
        #[schemars(length(min = 1, max = 128))]
        dispatch_id: String,
        #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
        expected_request_digest: String,
        #[schemars(range(min = 1, max = 256))]
        limit: u16,
        #[schemars(length(min = 1, max = 512))]
        cursor: Option<String>,
    },
    Read {
        #[schemars(length(min = 1, max = 128))]
        job_id: String,
        #[schemars(length(min = 1, max = 128))]
        dispatch_id: String,
        #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
        expected_request_digest: String,
        artifact: RemoteArtifactSelector,
        offset: u64,
        #[schemars(range(min = 1, max = 1048576))]
        max_bytes: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReportQuery {
    #[schemars(length(min = 1, max = 5))]
    pub states: Vec<RemoteJobState>,
    #[schemars(length(min = 1, max = 256))]
    pub suite: Option<String>,
    #[schemars(length(min = 1, max = 128))]
    pub run_id: Option<String>,
    pub finished_after_ms: Option<u64>,
    pub finished_before_ms: Option<u64>,
    #[schemars(range(min = 1, max = 100))]
    pub limit: u16,
    #[schemars(length(min = 1, max = 512))]
    pub cursor: Option<String>,
}

impl RemoteReportQuery {
    pub fn validate(&self) -> Result<(), RemoteWorkerError> {
        if self.states.is_empty()
            || self.states.len() > 5
            || self.states.windows(2).any(|states| states[0] >= states[1])
            || self.states.iter().any(|state| !state.terminal())
            || !(1..=MAX_REPORT_PAGE_SIZE).contains(&self.limit)
        {
            return Err(query_error());
        }
        if let Some(suite) = &self.suite {
            validate_readable(suite, 256).map_err(|_| query_error())?;
        }
        if let Some(run_id) = &self.run_id {
            validate_token(run_id, "run ID").map_err(|_| query_error())?;
        }
        if self
            .finished_after_ms
            .zip(self.finished_before_ms)
            .is_some_and(|(after, before)| after >= before)
        {
            return Err(query_error());
        }
        validate_cursor(self.cursor.as_deref()).map_err(|_| query_error())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RemoteArtifactSelector {
    Report {
        #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
        sha256: String,
    },
    Evidence {
        #[schemars(length(min = 1, max = 512))]
        path: String,
        #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
        sha256: String,
    },
}

impl RemoteArtifactSelector {
    pub(super) fn validate(&self) -> Result<(), RemoteWorkerError> {
        match self {
            Self::Report { sha256 } => validate_digest(sha256, "report digest"),
            Self::Evidence { path, sha256 } => {
                validate_artifact_path(path)?;
                validate_digest(sha256, "artifact digest")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemotePayloadState {
    Retained,
    Pruned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteArtifactKind {
    Report,
    Evidence,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteArtifactFileDescriptor {
    pub kind: RemoteArtifactKind,
    #[schemars(length(min = 1, max = 512))]
    pub path: Option<String>,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub sha256: String,
    pub bytes: u64,
    #[schemars(length(min = 1, max = 128))]
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReportIndexEntry {
    pub job: RemoteJobSnapshot,
    pub payload_state: RemotePayloadState,
    pub indexed_at_ms: u64,
    pub artifact_count: u32,
    pub artifact_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReportPage {
    pub reports: Vec<RemoteReportIndexEntry>,
    #[schemars(length(min = 1, max = 512))]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteArtifactPage {
    #[schemars(length(min = 1, max = 128))]
    pub job_id: String,
    #[schemars(length(min = 1, max = 128))]
    pub dispatch_id: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub request_digest: String,
    pub artifacts: Vec<RemoteArtifactFileDescriptor>,
    #[schemars(length(min = 1, max = 512))]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteArtifactChunk {
    #[schemars(length(min = 1, max = 128))]
    pub job_id: String,
    #[schemars(length(min = 1, max = 128))]
    pub dispatch_id: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub request_digest: String,
    pub artifact: RemoteArtifactFileDescriptor,
    pub offset: u64,
    #[schemars(length(min = 1, max = 1398104))]
    pub contents_base64: String,
    pub eof: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteArtifactResponse {
    #[schemars(schema_with = "remote_artifact_protocol_field_schema")]
    pub protocol: String,
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub outcome: RemoteArtifactOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RemoteArtifactOutcome {
    Descriptor { service: RemoteArtifactDescriptor },
    Reports { page: RemoteReportPage },
    Artifacts { page: RemoteArtifactPage },
    Chunk { chunk: RemoteArtifactChunk },
    Error { error: RemoteWorkerError },
}

impl RemoteArtifactRequest {
    pub fn validate(&self) -> Result<(), RemoteWorkerError> {
        if self.protocol != REMOTE_ARTIFACT_PROTOCOL {
            return Err(artifact_error(
                "test.worker.artifact.protocol_unsupported",
                format!("unsupported remote artifact protocol {:?}", self.protocol),
            ));
        }
        validate_token(&self.request_id, "request ID")?;
        match &self.command {
            RemoteArtifactCommand::Inspect => Ok(()),
            RemoteArtifactCommand::ListReports { query } => query.validate(),
            RemoteArtifactCommand::ListArtifacts {
                job_id,
                dispatch_id,
                expected_request_digest,
                limit,
                cursor,
            } => {
                validate_job_binding(job_id, dispatch_id, expected_request_digest)?;
                if !(1..=MAX_ARTIFACT_PAGE_SIZE).contains(limit) {
                    return Err(query_error());
                }
                validate_cursor(cursor.as_deref()).map_err(|_| query_error())
            }
            RemoteArtifactCommand::Read {
                job_id,
                dispatch_id,
                expected_request_digest,
                artifact,
                max_bytes,
                ..
            } => {
                validate_job_binding(job_id, dispatch_id, expected_request_digest)?;
                artifact.validate()?;
                if !(1..=MAX_ARTIFACT_CHUNK_BYTES).contains(max_bytes) {
                    return Err(artifact_error(
                        "test.worker.artifact.chunk_invalid",
                        "artifact chunk size is outside the protocol bound",
                    ));
                }
                Ok(())
            }
        }
    }
}

impl RemoteArtifactResponse {
    #[must_use]
    pub fn new(request_id: impl Into<String>, outcome: RemoteArtifactOutcome) -> Self {
        Self {
            protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
            request_id: request_id.into(),
            outcome,
        }
    }
}

fn validate_job_binding(
    job_id: &str,
    dispatch_id: &str,
    request_digest: &str,
) -> Result<(), RemoteWorkerError> {
    validate_storage_key(job_id, "job ID")?;
    validate_storage_key(dispatch_id, "dispatch ID")?;
    validate_digest(request_digest, "request digest")
}

fn validate_readable(value: &str, max_bytes: usize) -> Result<(), RemoteWorkerError> {
    if value.trim().is_empty()
        || value.len() > max_bytes
        || value
            .chars()
            .any(|character| character == '\0' || character.is_control())
    {
        return Err(query_error());
    }
    Ok(())
}

fn validate_cursor(cursor: Option<&str>) -> Result<(), RemoteWorkerError> {
    if cursor.is_some_and(|cursor| {
        cursor.is_empty()
            || cursor.len() > MAX_CURSOR_BYTES
            || !cursor
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    }) {
        return Err(query_error());
    }
    Ok(())
}

fn validate_artifact_path(path: &str) -> Result<(), RemoteWorkerError> {
    let valid = !path.is_empty()
        && path.len() <= 512
        && !path.starts_with('/')
        && !path.contains('\\')
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && component.len() <= 128
                && !component.chars().any(char::is_control)
        });
    if !valid {
        return Err(artifact_error(
            "test.worker.artifact.path_invalid",
            "artifact path must be a bounded portable relative path",
        ));
    }
    Ok(())
}

fn query_error() -> RemoteWorkerError {
    artifact_error(
        "test.worker.artifact.query_invalid",
        "report query is outside the canonical bounded form",
    )
}

fn artifact_error(code: &'static str, message: impl Into<String>) -> RemoteWorkerError {
    RemoteWorkerError::new(code, message, false)
}

pub(super) fn remote_artifact_protocol_field_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": REMOTE_ARTIFACT_PROTOCOL
    })
}
