use std::collections::BTreeSet;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use sha2::{Digest, Sha256};

use crate::WorkerSurface;

use super::{
    RemoteInputFile, RemoteJobSubmission, RemoteWorkerDescriptor, RemoteWorkerError,
    RemoteWorkerLimits, MAX_REMOTE_CLEANUP_TIMEOUT_MS, MIN_REMOTE_CLEANUP_TIMEOUT_MS,
    REMOTE_WORKER_PROTOCOL,
};

const MAX_CLOCK_SKEW_MS: u64 = 30_000;

#[derive(Debug)]
pub struct AdmittedRemoteFile {
    path: String,
    bytes: Vec<u8>,
}

impl AdmittedRemoteFile {
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug)]
pub struct AdmittedRemoteJob {
    job_id: String,
    dispatch_id: String,
    request_digest: String,
    manifest: String,
    files: Vec<AdmittedRemoteFile>,
    deadline_ms: u64,
    lease_expires_at_ms: u64,
    max_parallel_scenarios: u16,
    required_surfaces: Vec<WorkerSurface>,
    scenario_ids: Vec<String>,
}

impl AdmittedRemoteJob {
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
    pub fn manifest(&self) -> &str {
        &self.manifest
    }

    #[must_use]
    pub fn files(&self) -> &[AdmittedRemoteFile] {
        &self.files
    }

    #[must_use]
    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }

    #[must_use]
    pub fn lease_expires_at_ms(&self) -> u64 {
        self.lease_expires_at_ms
    }

    #[must_use]
    pub fn max_parallel_scenarios(&self) -> u16 {
        self.max_parallel_scenarios
    }

    #[must_use]
    pub fn required_surfaces(&self) -> &[WorkerSurface] {
        &self.required_surfaces
    }

    #[must_use]
    pub fn scenario_ids(&self) -> &[String] {
        &self.scenario_ids
    }
}

impl RemoteInputFile {
    #[must_use]
    pub fn from_bytes(path: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        let bytes = bytes.as_ref();
        Self {
            path: path.into(),
            sha256: sha256(bytes),
            contents_base64: STANDARD.encode(bytes),
        }
    }
}

impl RemoteWorkerLimits {
    pub fn validate(&self) -> Result<(), RemoteWorkerError> {
        let valid = (1_024..=64 * 1024 * 1024).contains(&self.max_request_bytes)
            && (1..=1_024).contains(&self.max_files)
            && (1..=16 * 1024 * 1024).contains(&self.max_file_bytes)
            && self.max_file_bytes <= self.max_total_input_bytes
            && self.max_total_input_bytes <= 32 * 1024 * 1024
            && self.max_total_input_bytes <= self.max_request_bytes
            && (1_000..=24 * 60 * 60 * 1_000).contains(&self.max_job_duration_ms)
            && (1_000..=60 * 60 * 1_000).contains(&self.max_lease_ms)
            && self.max_lease_ms <= self.max_job_duration_ms
            && (1..=1_024).contains(&self.max_queued_jobs)
            && (1_024..=64 * 1024 * 1024).contains(&self.max_report_bytes)
            && (MIN_REMOTE_CLEANUP_TIMEOUT_MS..=MAX_REMOTE_CLEANUP_TIMEOUT_MS)
                .contains(&self.cleanup_timeout_ms);
        if !valid {
            return Err(remote_error(
                "test.worker.remote.limits_invalid",
                "remote worker limits are outside the reviewed safety bounds",
            ));
        }
        Ok(())
    }
}

impl RemoteWorkerDescriptor {
    pub fn new(
        identity: super::RemoteWorkerIdentity,
        inventory: crate::WorkerCapabilityInventory,
        limits: RemoteWorkerLimits,
    ) -> Result<Self, RemoteWorkerError> {
        validate_token(&identity.instance_id, "worker instance ID")?;
        validate_digest(&identity.image_digest, "worker image digest")?;
        inventory.validate().map_err(|error| {
            remote_error(
                "test.worker.remote.inventory_invalid",
                format!("worker capability inventory is invalid: {error}"),
            )
        })?;
        limits.validate()?;
        let encoded = serde_json::to_vec(&inventory).map_err(|error| {
            remote_error(
                "test.worker.remote.inventory_invalid",
                format!("failed to encode worker capability inventory: {error}"),
            )
        })?;
        Ok(Self {
            protocol: REMOTE_WORKER_PROTOCOL.to_string(),
            identity,
            inventory,
            inventory_digest: sha256(&encoded),
            limits,
        })
    }

    pub fn validate(&self) -> Result<(), RemoteWorkerError> {
        if self.protocol != REMOTE_WORKER_PROTOCOL {
            return Err(remote_error(
                "test.worker.remote.protocol_unsupported",
                format!("unsupported remote worker protocol {:?}", self.protocol),
            ));
        }
        let rebuilt = Self::new(
            self.identity.clone(),
            self.inventory.clone(),
            self.limits.clone(),
        )?;
        if rebuilt.inventory_digest != self.inventory_digest {
            return Err(remote_error(
                "test.worker.remote.inventory_digest_mismatch",
                "worker inventory does not match its declared digest",
            ));
        }
        Ok(())
    }
}

pub(super) fn admit_submission(
    submission: &RemoteJobSubmission,
    now_ms: u64,
    descriptor: &RemoteWorkerDescriptor,
) -> Result<AdmittedRemoteJob, RemoteWorkerError> {
    descriptor.validate()?;
    validate_storage_key(&submission.job_id, "job ID")?;
    validate_storage_key(&submission.dispatch_id, "dispatch ID")?;
    validate_token(&submission.worker_instance, "worker instance ID")?;
    validate_digest(&submission.required_image_digest, "required image digest")?;
    validate_digest(
        &submission.required_inventory_digest,
        "required inventory digest",
    )?;
    if submission.worker_instance != descriptor.identity.instance_id {
        return Err(remote_error(
            "test.worker.remote.instance_mismatch",
            "job was dispatched to a different worker instance",
        ));
    }
    if submission.required_image_digest != descriptor.identity.image_digest {
        return Err(remote_error(
            "test.worker.remote.image_mismatch",
            "job requires a different externally bound worker image",
        ));
    }
    if submission.required_inventory_digest != descriptor.inventory_digest {
        return Err(remote_error(
            "test.worker.remote.inventory_mismatch",
            "job requires a different worker capability inventory",
        ));
    }
    validate_times(submission, now_ms, &descriptor.limits)?;
    validate_requirements(submission, descriptor)?;

    let encoded = encode_submission(submission)?;
    if encoded.len() as u64 > descriptor.limits.max_request_bytes {
        return Err(remote_error(
            "test.worker.remote.request_too_large",
            "remote job submission exceeds the worker request limit",
        ));
    }
    let files = admit_bundle(submission, &descriptor.limits)?;
    Ok(AdmittedRemoteJob {
        job_id: submission.job_id.clone(),
        dispatch_id: submission.dispatch_id.clone(),
        request_digest: sha256(&encoded),
        manifest: submission.input.manifest.clone(),
        files,
        deadline_ms: submission.deadline_ms,
        lease_expires_at_ms: submission.lease_expires_at_ms,
        max_parallel_scenarios: submission.max_parallel_scenarios,
        required_surfaces: submission.required_surfaces.clone(),
        scenario_ids: submission.scenario_ids.clone(),
    })
}

pub(super) fn submission_request_digest(
    submission: &RemoteJobSubmission,
    limits: &RemoteWorkerLimits,
) -> Result<String, RemoteWorkerError> {
    validate_storage_key(&submission.job_id, "job ID")?;
    validate_storage_key(&submission.dispatch_id, "dispatch ID")?;
    let encoded = encode_submission(submission)?;
    if encoded.len() as u64 > limits.max_request_bytes {
        return Err(remote_error(
            "test.worker.remote.request_too_large",
            "remote job submission exceeds the worker request limit",
        ));
    }
    Ok(sha256(&encoded))
}

fn encode_submission(submission: &RemoteJobSubmission) -> Result<Vec<u8>, RemoteWorkerError> {
    serde_json::to_vec(submission).map_err(|error| {
        remote_error(
            "test.worker.remote.request_invalid",
            format!("failed to encode remote job submission: {error}"),
        )
    })
}

fn validate_times(
    submission: &RemoteJobSubmission,
    now_ms: u64,
    limits: &RemoteWorkerLimits,
) -> Result<(), RemoteWorkerError> {
    if submission.issued_at_ms > now_ms.saturating_add(MAX_CLOCK_SKEW_MS)
        || now_ms.saturating_sub(submission.issued_at_ms) > limits.max_lease_ms
    {
        return Err(remote_error(
            "test.worker.remote.issued_at_invalid",
            "job issue time is outside the admitted clock window",
        ));
    }
    let duration = submission
        .deadline_ms
        .checked_sub(submission.issued_at_ms)
        .filter(|duration| *duration > 0 && *duration <= limits.max_job_duration_ms);
    if submission.deadline_ms <= now_ms || duration.is_none() {
        return Err(remote_error(
            "test.worker.remote.deadline_invalid",
            "job deadline must be future, ordered, and within the worker limit",
        ));
    }
    if submission.lease_expires_at_ms <= now_ms
        || submission.lease_expires_at_ms > submission.deadline_ms
        || submission.lease_expires_at_ms.saturating_sub(now_ms) > limits.max_lease_ms
    {
        return Err(remote_error(
            "test.worker.remote.lease_invalid",
            "job lease must be future, bounded, and no later than the deadline",
        ));
    }
    Ok(())
}

fn validate_requirements(
    submission: &RemoteJobSubmission,
    descriptor: &RemoteWorkerDescriptor,
) -> Result<(), RemoteWorkerError> {
    if submission.max_parallel_scenarios == 0
        || submission.max_parallel_scenarios > descriptor.inventory.max_parallel_scenarios
    {
        return Err(remote_error(
            "test.worker.remote.parallelism_unavailable",
            "job parallelism exceeds the admitted worker inventory",
        ));
    }
    if submission.required_surfaces.is_empty()
        || submission
            .required_surfaces
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(remote_error(
            "test.worker.remote.surface_order_invalid",
            "required surfaces must be non-empty, unique, and canonically ordered",
        ));
    }
    let available = descriptor
        .inventory
        .surfaces
        .iter()
        .map(crate::WorkerSurfaceCapability::surface)
        .collect::<BTreeSet<_>>();
    if submission
        .required_surfaces
        .iter()
        .any(|surface| !available.contains(surface))
    {
        return Err(remote_error(
            "test.worker.remote.surface_unavailable",
            "job requires a surface absent from the admitted worker inventory",
        ));
    }
    if submission.scenario_ids.is_empty()
        || submission.scenario_ids.len() > 4_096
        || submission
            .scenario_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || submission.scenario_ids.iter().any(|scenario_id| {
            scenario_id.len() > 64
                || scenario_id.is_empty()
                || !scenario_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
    {
        return Err(remote_error(
            "test.worker.remote.scenario_selection_invalid",
            "scenario IDs must be non-empty, unique, canonically ordered, and portable",
        ));
    }
    Ok(())
}

fn admit_bundle(
    submission: &RemoteJobSubmission,
    limits: &RemoteWorkerLimits,
) -> Result<Vec<AdmittedRemoteFile>, RemoteWorkerError> {
    validate_relative_path(&submission.input.manifest)?;
    if submission.input.files.is_empty() || submission.input.files.len() > limits.max_files as usize
    {
        return Err(remote_error(
            "test.worker.remote.input_count_invalid",
            "input bundle file count is outside the worker limit",
        ));
    }
    if submission
        .input
        .files
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(remote_error(
            "test.worker.remote.input_order_invalid",
            "input files must be unique and sorted by canonical path",
        ));
    }

    let mut total = 0_u64;
    let mut found_manifest = false;
    let mut admitted = Vec::with_capacity(submission.input.files.len());
    let mut portable_paths = BTreeSet::new();
    for file in &submission.input.files {
        validate_relative_path(&file.path)?;
        if !portable_paths.insert(file.path.to_ascii_lowercase()) {
            return Err(remote_error(
                "test.worker.remote.input_path_collision",
                "input paths must remain unique on case-insensitive filesystems",
            ));
        }
        validate_digest(&file.sha256, "input file digest")?;
        let bytes = STANDARD.decode(&file.contents_base64).map_err(|_| {
            remote_error(
                "test.worker.remote.input_encoding_invalid",
                "input file contents are not canonical Base64",
            )
        })?;
        if STANDARD.encode(&bytes) != file.contents_base64 {
            return Err(remote_error(
                "test.worker.remote.input_encoding_invalid",
                "input file contents are not canonical Base64",
            ));
        }
        if bytes.is_empty() {
            return Err(remote_error(
                "test.worker.remote.input_file_empty",
                "input files must not be empty",
            ));
        }
        if bytes.len() as u64 > limits.max_file_bytes {
            return Err(remote_error(
                "test.worker.remote.input_file_too_large",
                "an input file exceeds the worker file limit",
            ));
        }
        total = total.checked_add(bytes.len() as u64).ok_or_else(|| {
            remote_error(
                "test.worker.remote.input_too_large",
                "input bundle size overflowed",
            )
        })?;
        if total > limits.max_total_input_bytes {
            return Err(remote_error(
                "test.worker.remote.input_too_large",
                "input bundle exceeds the worker total input limit",
            ));
        }
        if sha256(&bytes) != file.sha256 {
            return Err(remote_error(
                "test.worker.remote.input_digest_mismatch",
                format!(
                    "input file {:?} does not match its SHA-256 digest",
                    file.path
                ),
            ));
        }
        found_manifest |= file.path == submission.input.manifest;
        admitted.push(AdmittedRemoteFile {
            path: file.path.clone(),
            bytes,
        });
    }
    if !found_manifest {
        return Err(remote_error(
            "test.worker.remote.manifest_missing",
            "input bundle does not contain its declared manifest",
        ));
    }
    Ok(admitted)
}

pub(super) fn validate_token(value: &str, label: &str) -> Result<(), RemoteWorkerError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'));
    if !valid {
        return Err(remote_error(
            "test.worker.remote.identifier_invalid",
            format!("{label} must be a bounded portable identifier"),
        ));
    }
    Ok(())
}

pub(super) fn validate_storage_key(value: &str, label: &str) -> Result<(), RemoteWorkerError> {
    validate_token(value, label)?;
    if value.starts_with('.')
        || value.ends_with('.')
        || value.contains(':')
        || is_windows_reserved_component(value)
    {
        return Err(remote_error(
            "test.worker.remote.identifier_invalid",
            format!("{label} must be a portable filesystem-safe identifier"),
        ));
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), RemoteWorkerError> {
    let valid = !path.is_empty()
        && path.len() <= 256
        && !path.starts_with('/')
        && !path.contains('\\')
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.ends_with('.')
                && !is_windows_reserved_component(component)
                && component.len() <= 64
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        });
    if !valid {
        return Err(remote_error(
            "test.worker.remote.input_path_invalid",
            "input paths must be bounded portable relative paths",
        ));
    }
    Ok(())
}

fn is_windows_reserved_component(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

pub(super) fn validate_digest(value: &str, label: &str) -> Result<(), RemoteWorkerError> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if !valid {
        return Err(remote_error(
            "test.worker.remote.digest_invalid",
            format!("{label} must be a lowercase SHA-256 digest"),
        ));
    }
    Ok(())
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(super) fn remote_error(code: &'static str, message: impl Into<String>) -> RemoteWorkerError {
    RemoteWorkerError::new(code, message, false)
}
