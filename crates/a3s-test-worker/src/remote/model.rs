use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::{WorkerCapabilityInventory, WorkerSurface};

use super::{AdmittedRemoteJob, REMOTE_WORKER_PROTOCOL};

pub const DEFAULT_MAX_REQUEST_BYTES: u64 = 24 * 1024 * 1024;
pub const DEFAULT_MAX_FILES: u16 = 128;
pub const DEFAULT_MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
pub const DEFAULT_MAX_TOTAL_INPUT_BYTES: u64 = 16 * 1024 * 1024;
pub const DEFAULT_MAX_JOB_DURATION_MS: u64 = 60 * 60 * 1_000;
pub const DEFAULT_MAX_LEASE_MS: u64 = 5 * 60 * 1_000;
pub const DEFAULT_MAX_QUEUED_JOBS: u16 = 16;
pub const DEFAULT_MAX_REPORT_BYTES: u64 = 16 * 1024 * 1024;
pub const DEFAULT_CLEANUP_TIMEOUT_MS: u64 = 30_000;
pub const MIN_REMOTE_CLEANUP_TIMEOUT_MS: u64 = 1_000;
pub const MAX_REMOTE_CLEANUP_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWorkerLimits {
    #[schemars(range(min = 1024, max = 67108864))]
    pub max_request_bytes: u64,
    #[schemars(range(min = 1, max = 1024))]
    pub max_files: u16,
    #[schemars(range(min = 1, max = 16777216))]
    pub max_file_bytes: u64,
    #[schemars(range(min = 1, max = 33554432))]
    pub max_total_input_bytes: u64,
    #[schemars(range(min = 1000, max = 86400000))]
    pub max_job_duration_ms: u64,
    #[schemars(range(min = 1000, max = 3600000))]
    pub max_lease_ms: u64,
    #[schemars(range(min = 1, max = 1024))]
    pub max_queued_jobs: u16,
    #[schemars(range(min = 1024, max = 67108864))]
    pub max_report_bytes: u64,
    #[schemars(range(min = 1000, max = 300000))]
    pub cleanup_timeout_ms: u64,
}

impl Default for RemoteWorkerLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_files: DEFAULT_MAX_FILES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_total_input_bytes: DEFAULT_MAX_TOTAL_INPUT_BYTES,
            max_job_duration_ms: DEFAULT_MAX_JOB_DURATION_MS,
            max_lease_ms: DEFAULT_MAX_LEASE_MS,
            max_queued_jobs: DEFAULT_MAX_QUEUED_JOBS,
            max_report_bytes: DEFAULT_MAX_REPORT_BYTES,
            cleanup_timeout_ms: DEFAULT_CLEANUP_TIMEOUT_MS,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWorkerIdentity {
    #[schemars(length(min = 1, max = 128))]
    pub instance_id: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub image_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWorkerDescriptor {
    #[schemars(schema_with = "remote_worker_protocol_field_schema")]
    pub protocol: String,
    pub identity: RemoteWorkerIdentity,
    pub inventory: WorkerCapabilityInventory,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub inventory_digest: String,
    pub limits: RemoteWorkerLimits,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteInputFile {
    #[schemars(length(min = 1, max = 256))]
    pub path: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub sha256: String,
    #[schemars(length(min = 1, max = 22369624))]
    pub contents_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteInputBundle {
    #[schemars(length(min = 1, max = 256))]
    pub manifest: String,
    #[schemars(length(min = 1, max = 1024))]
    pub files: Vec<RemoteInputFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteJobSubmission {
    #[schemars(length(min = 1, max = 128))]
    #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
    pub job_id: String,
    #[schemars(length(min = 1, max = 128))]
    #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
    pub dispatch_id: String,
    #[schemars(length(min = 1, max = 128))]
    pub worker_instance: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub required_image_digest: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub required_inventory_digest: String,
    pub issued_at_ms: u64,
    pub deadline_ms: u64,
    pub lease_expires_at_ms: u64,
    #[schemars(range(min = 1, max = 64))]
    pub max_parallel_scenarios: u16,
    #[schemars(length(min = 1, max = 2))]
    pub required_surfaces: Vec<WorkerSurface>,
    #[schemars(length(min = 1, max = 4096))]
    pub scenario_ids: Vec<String>,
    pub input: RemoteInputBundle,
}

impl RemoteJobSubmission {
    pub fn admit(
        &self,
        now_ms: u64,
        descriptor: &RemoteWorkerDescriptor,
    ) -> Result<AdmittedRemoteJob, RemoteWorkerError> {
        super::admission::admit_submission(self, now_ms, descriptor)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWorkerRequest {
    #[schemars(schema_with = "remote_worker_protocol_field_schema")]
    pub protocol: String,
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub command: RemoteWorkerCommand,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RemoteWorkerCommand {
    Inspect,
    Submit {
        job: RemoteJobSubmission,
    },
    Status {
        #[schemars(length(min = 1, max = 128))]
        #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
        job_id: String,
        #[schemars(length(min = 1, max = 128))]
        #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
        dispatch_id: String,
    },
    RenewLease {
        #[schemars(length(min = 1, max = 128))]
        #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
        job_id: String,
        #[schemars(length(min = 1, max = 128))]
        #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
        dispatch_id: String,
        lease_expires_at_ms: u64,
    },
    Cancel {
        #[schemars(length(min = 1, max = 128))]
        #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
        job_id: String,
        #[schemars(length(min = 1, max = 128))]
        #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
        dispatch_id: String,
        #[schemars(length(min = 1, max = 1024))]
        reason: Option<String>,
    },
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RemoteJobState {
    Queued,
    Running,
    Cancelling,
    Passed,
    Failed,
    TimedOut,
    Cancelled,
    Interrupted,
}

impl RemoteJobState {
    #[must_use]
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Passed | Self::Failed | Self::TimedOut | Self::Cancelled | Self::Interrupted
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWorkerError {
    #[schemars(length(min = 1, max = 128))]
    pub code: String,
    #[schemars(length(min = 1, max = 2048))]
    pub message: String,
    pub retryable: bool,
}

impl RemoteWorkerError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }
}

impl std::fmt::Display for RemoteWorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RemoteWorkerError {}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReportDescriptor {
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub sha256: String,
    #[schemars(range(min = 1, max = 67108864))]
    pub bytes: u64,
    #[schemars(length(min = 1, max = 128))]
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteScenarioCounts {
    pub passed: u32,
    pub failed: u32,
    pub timed_out: u32,
    pub cancelled: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRunSummary {
    #[schemars(length(min = 1, max = 128))]
    pub run_id: String,
    #[schemars(length(min = 1, max = 256))]
    pub suite: String,
    pub status: RemoteJobState,
    pub scenarios: RemoteScenarioCounts,
    pub report: RemoteReportDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteJobSnapshot {
    #[schemars(length(min = 1, max = 128))]
    #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
    pub job_id: String,
    #[schemars(length(min = 1, max = 128))]
    #[schemars(regex(pattern = r"^[A-Za-z0-9_-](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_-])?$"))]
    pub dispatch_id: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    pub request_digest: String,
    pub state: RemoteJobState,
    pub submitted_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub finished_at_ms: Option<u64>,
    pub deadline_ms: u64,
    pub lease_expires_at_ms: u64,
    pub result: Option<RemoteRunSummary>,
    pub error: Option<RemoteWorkerError>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWorkerResponse {
    #[schemars(schema_with = "remote_worker_protocol_field_schema")]
    pub protocol: String,
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub outcome: RemoteWorkerOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RemoteWorkerOutcome {
    Descriptor { worker: RemoteWorkerDescriptor },
    Job { job: RemoteJobSnapshot },
    Error { error: RemoteWorkerError },
}

impl RemoteWorkerRequest {
    pub fn validate(&self) -> Result<(), RemoteWorkerError> {
        if self.protocol != REMOTE_WORKER_PROTOCOL {
            return Err(RemoteWorkerError::new(
                "test.worker.remote.protocol_unsupported",
                format!("unsupported remote worker protocol {:?}", self.protocol),
                false,
            ));
        }
        super::admission::validate_token(&self.request_id, "request ID")
    }
}

impl RemoteWorkerResponse {
    #[must_use]
    pub fn new(request_id: impl Into<String>, outcome: RemoteWorkerOutcome) -> Self {
        Self {
            protocol: REMOTE_WORKER_PROTOCOL.to_string(),
            request_id: request_id.into(),
            outcome,
        }
    }
}

pub(super) fn remote_worker_protocol_field_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": REMOTE_WORKER_PROTOCOL
    })
}
