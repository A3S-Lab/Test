use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use a3s_test_core::RepairStatus;
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::{RepairRecord, RepairTransition, SessionError};

const OWNER_SCHEMA_VERSION: u32 = 1;
const MAX_LEASE_MS: u64 = 15 * 60 * 1_000;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct RepairWorkspace {
    state_dir: PathBuf,
}

pub struct RepairWorkspaceLock {
    file: std::fs::File,
    owner_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum MutationPhase {
    Claimed,
    Repairing,
    Verifying,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct WorkspaceOwner {
    schema_version: u32,
    session: String,
    finding_id: String,
    attempt_id: String,
    phase: MutationPhase,
    lease_expires_at_ms: u64,
    updated_at_ms: u64,
}

impl RepairWorkspace {
    #[must_use]
    pub fn new(workspace: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: workspace.into().join(".a3s-test"),
        }
    }

    #[must_use]
    pub fn from_state_dir(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
        }
    }

    #[must_use]
    pub fn from_artifacts_root(artifacts_root: impl Into<PathBuf>) -> Self {
        let artifacts_root = artifacts_root.into();
        let state_dir = artifacts_root
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == ".a3s-test"))
            .map(Path::to_path_buf)
            .unwrap_or_else(|| artifacts_root.clone());
        Self { state_dir }
    }

    pub async fn acquire(&self) -> Result<RepairWorkspaceLock, SessionError> {
        tokio::fs::create_dir_all(&self.state_dir)
            .await
            .map_err(|error| workspace_storage_error(&self.state_dir, error))?;
        let lock_path = self.state_dir.join("repair-workspace.lock");
        let owner_path = self.state_dir.join("repair-workspace.json");
        let file = tokio::task::spawn_blocking(move || {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(&lock_path)
                .map_err(|error| workspace_storage_error(&lock_path, error))?;
            file.lock_exclusive()
                .map_err(|error| workspace_storage_error(&lock_path, error))?;
            Ok::<_, SessionError>(file)
        })
        .await
        .map_err(|error| {
            SessionError::new(
                "test.session.repair_workspace_lock_failed",
                format!("repair workspace lock task failed: {error}"),
            )
        })??;
        Ok(RepairWorkspaceLock { file, owner_path })
    }
}

impl RepairWorkspaceLock {
    pub async fn validate_attempt_owner(
        &self,
        session: &str,
        finding_id: &str,
        attempt_id: &str,
        status: RepairStatus,
        now_ms: u64,
    ) -> Result<(), SessionError> {
        let expected_phase = phase_for_status(status).ok_or_else(|| {
            SessionError::new(
                "test.session.repair_workspace_invalid",
                "repair workspace ownership can only be checked for a mutating state",
            )
        })?;
        let owner = self.read_owner().await?.ok_or_else(|| {
            SessionError::new(
                "test.session.repair_state_changed",
                "repair workspace ownership changed while work was in progress",
            )
            .with_retryable(true)
        })?;
        if owner.session != session
            || owner.finding_id != finding_id
            || owner.attempt_id != attempt_id
            || owner.phase != expected_phase
        {
            return Err(SessionError::new(
                "test.session.repair_state_changed",
                "repair workspace ownership changed while work was in progress",
            )
            .with_retryable(true));
        }
        if owner.lease_expires_at_ms <= now_ms {
            return Err(expired_owner(&owner));
        }
        Ok(())
    }

    pub(crate) async fn prepare_transition(
        &mut self,
        current: &RepairRecord,
        request: &RepairTransition,
        now_ms: u64,
    ) -> Result<Option<WorkspaceOwner>, SessionError> {
        if !is_mutating(request.status) {
            if is_mutating(current.status) {
                let attempt_id = current.attempt_id.as_deref().ok_or_else(|| {
                    SessionError::new(
                        "test.session.repair_attempt_invalid",
                        "workspace mutation is missing its active repair attempt id",
                    )
                })?;
                self.validate_attempt_owner(
                    &request.session,
                    &request.finding_id,
                    attempt_id,
                    current.status,
                    now_ms,
                )
                .await?;
            }
            return Ok(None);
        }
        let desired = owner_for_transition(current, request, now_ms)?;
        let previous = self.read_owner().await?;
        match previous.as_ref() {
            None => self.write_owner(&desired).await?,
            Some(owner) if same_attempt(owner, &desired) => {
                if owner.lease_expires_at_ms <= now_ms {
                    return Err(expired_owner(owner));
                }
                if phase_rank(desired.phase) < phase_rank(owner.phase) {
                    return Err(SessionError::new(
                        "test.session.repair_workspace_invalid",
                        "repair workspace phase cannot move backwards",
                    ));
                }
                self.write_owner(&desired).await?;
            }
            Some(owner)
                if owner.phase == MutationPhase::Claimed && owner.lease_expires_at_ms <= now_ms =>
            {
                self.write_owner(&desired).await?;
            }
            Some(owner) => return Err(workspace_busy(owner)),
        }
        Ok(previous)
    }

    pub(crate) async fn rollback(
        &mut self,
        previous: Option<WorkspaceOwner>,
    ) -> Result<(), SessionError> {
        match previous {
            Some(owner) => self.write_owner(&owner).await,
            None => self.clear_owner().await,
        }
    }

    pub(crate) async fn reconcile_record(
        &mut self,
        session: &str,
        record: &RepairRecord,
        now_ms: u64,
    ) -> Result<(), SessionError> {
        let owner = self.read_owner().await?;
        if let Some(phase) = phase_for_status(record.status) {
            let Some(attempt_id) = record.attempt_id.as_deref() else {
                return Ok(());
            };
            let Some(lease_expires_at_ms) = record.lease_expires_at_ms else {
                return Ok(());
            };
            let desired = WorkspaceOwner {
                schema_version: OWNER_SCHEMA_VERSION,
                session: session.to_string(),
                finding_id: record.finding.id.clone(),
                attempt_id: attempt_id.to_string(),
                phase,
                lease_expires_at_ms,
                updated_at_ms: record.updated_at_ms,
            };
            match owner.as_ref() {
                None => self.write_owner(&desired).await,
                Some(existing) if same_attempt(existing, &desired) => {
                    self.write_owner(&desired).await
                }
                Some(existing)
                    if existing.phase == MutationPhase::Claimed
                        && existing.lease_expires_at_ms <= now_ms =>
                {
                    self.write_owner(&desired).await
                }
                Some(existing) => Err(workspace_busy(existing)),
            }
        } else if owner.as_ref().is_some_and(|owner| {
            owner.session == session
                && owner.finding_id == record.finding.id
                && record
                    .attempt_id
                    .as_deref()
                    .is_none_or(|attempt_id| owner.attempt_id == attempt_id)
        }) {
            self.clear_owner().await
        } else {
            Ok(())
        }
    }

    pub(crate) async fn finish_transition(
        &mut self,
        previous_status: RepairStatus,
        record: &RepairRecord,
        session: &str,
        now_ms: u64,
    ) -> Result<(), SessionError> {
        if is_mutating(previous_status) && !is_mutating(record.status) {
            self.reconcile_record(session, record, now_ms).await?;
        }
        Ok(())
    }

    async fn read_owner(&self) -> Result<Option<WorkspaceOwner>, SessionError> {
        let bytes = match tokio::fs::read(&self.owner_path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(workspace_storage_error(&self.owner_path, error)),
        };
        let owner: WorkspaceOwner = serde_json::from_slice(&bytes).map_err(|error| {
            SessionError::new(
                "test.session.repair_workspace_invalid",
                format!(
                    "invalid repair workspace owner {}: {error}",
                    self.owner_path.display()
                ),
            )
        })?;
        if owner.schema_version != OWNER_SCHEMA_VERSION
            || owner.session.is_empty()
            || owner.finding_id.is_empty()
            || owner.attempt_id.is_empty()
        {
            return Err(SessionError::new(
                "test.session.repair_workspace_invalid",
                format!(
                    "repair workspace owner {} has an unsupported or incomplete schema",
                    self.owner_path.display()
                ),
            ));
        }
        Ok(Some(owner))
    }

    async fn write_owner(&self, owner: &WorkspaceOwner) -> Result<(), SessionError> {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary =
            self.owner_path
                .with_extension(format!("json.{}.{}.tmp", std::process::id(), sequence));
        let bytes = serde_json::to_vec_pretty(owner).map_err(|error| {
            SessionError::new(
                "test.session.repair_workspace_invalid",
                format!("failed to encode repair workspace owner: {error}"),
            )
        })?;
        tokio::fs::write(&temporary, bytes)
            .await
            .map_err(|error| workspace_storage_error(&temporary, error))?;
        #[cfg(windows)]
        if self.owner_path.exists() {
            tokio::fs::remove_file(&self.owner_path)
                .await
                .map_err(|error| workspace_storage_error(&self.owner_path, error))?;
        }
        tokio::fs::rename(&temporary, &self.owner_path)
            .await
            .map_err(|error| workspace_storage_error(&self.owner_path, error))
    }

    async fn clear_owner(&self) -> Result<(), SessionError> {
        match tokio::fs::remove_file(&self.owner_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(workspace_storage_error(&self.owner_path, error)),
        }
    }
}

impl Drop for RepairWorkspaceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn owner_for_transition(
    current: &RepairRecord,
    request: &RepairTransition,
    now_ms: u64,
) -> Result<WorkspaceOwner, SessionError> {
    let attempt_id = request
        .attempt_id
        .as_deref()
        .or(current.attempt_id.as_deref())
        .ok_or_else(|| {
            SessionError::new(
                "test.session.repair_attempt_invalid",
                "workspace mutation requires the active repair attempt id",
            )
        })?;
    let lease_expires_at_ms = request
        .lease_expires_at_ms
        .or(current.lease_expires_at_ms)
        .ok_or_else(|| {
            SessionError::new(
                "test.session.repair_lease_invalid",
                "workspace mutation requires an active repair lease",
            )
        })?;
    if lease_expires_at_ms <= now_ms || lease_expires_at_ms.saturating_sub(now_ms) > MAX_LEASE_MS {
        return Err(SessionError::new(
            "test.session.repair_lease_expired",
            "repair workspace lease is expired or exceeds the maximum duration",
        )
        .with_retryable(true));
    }
    let phase = phase_for_status(request.status).ok_or_else(|| {
        SessionError::new(
            "test.session.repair_workspace_invalid",
            "workspace mutation requires a mutating repair status",
        )
    })?;
    Ok(WorkspaceOwner {
        schema_version: OWNER_SCHEMA_VERSION,
        session: request.session.clone(),
        finding_id: request.finding_id.clone(),
        attempt_id: attempt_id.to_string(),
        phase,
        lease_expires_at_ms,
        updated_at_ms: now_ms,
    })
}

fn phase_for_status(status: RepairStatus) -> Option<MutationPhase> {
    match status {
        RepairStatus::Claimed => Some(MutationPhase::Claimed),
        RepairStatus::Repairing => Some(MutationPhase::Repairing),
        RepairStatus::Verifying => Some(MutationPhase::Verifying),
        _ => None,
    }
}

fn is_mutating(status: RepairStatus) -> bool {
    phase_for_status(status).is_some()
}

fn phase_rank(phase: MutationPhase) -> u8 {
    match phase {
        MutationPhase::Claimed => 0,
        MutationPhase::Repairing => 1,
        MutationPhase::Verifying => 2,
    }
}

fn same_attempt(left: &WorkspaceOwner, right: &WorkspaceOwner) -> bool {
    left.session == right.session
        && left.finding_id == right.finding_id
        && left.attempt_id == right.attempt_id
}

fn workspace_busy(owner: &WorkspaceOwner) -> SessionError {
    SessionError::new(
        "test.session.repair_workspace_busy",
        format!(
            "repair '{}' in session '{}' owns the workspace mutation slot in {:?} until {}",
            owner.finding_id, owner.session, owner.phase, owner.lease_expires_at_ms
        ),
    )
    .with_retryable(true)
}

fn expired_owner(owner: &WorkspaceOwner) -> SessionError {
    SessionError::new(
        "test.session.repair_lease_expired",
        format!(
            "repair '{}' in session '{}' has an expired workspace lease; watch that session to recover it",
            owner.finding_id, owner.session
        ),
    )
    .with_retryable(true)
}

fn workspace_storage_error(path: &Path, error: std::io::Error) -> SessionError {
    SessionError::new(
        "test.session.repair_workspace_storage_failed",
        format!(
            "failed to access repair workspace state {}: {error}",
            path.display()
        ),
    )
    .with_retryable(true)
}
