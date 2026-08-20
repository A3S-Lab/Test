use a3s_test_core::{
    RepairActor, RepairEvidenceBundle, RepairFinding, RepairStatus, RepairVerification,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairEventRecord {
    pub session: String,
    pub finding_id: String,
    pub request_id: String,
    pub sequence: u64,
    pub status: RepairStatus,
    pub actor: RepairActor,
    pub timestamp_ms: u64,
    pub attempt_id: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub summary: Option<String>,
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<RepairVerification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_files: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct RepairTransition {
    pub session: String,
    pub finding_id: String,
    pub request_id: String,
    pub status: RepairStatus,
    pub actor: RepairActor,
    pub attempt_id: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub verification: Option<RepairVerification>,
    pub changed_files: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum StoredLedgerEvent {
    Submitted {
        session: String,
        finding: Box<RepairFinding>,
        timestamp_ms: u64,
    },
    Transition {
        event: Box<RepairEventRecord>,
    },
    BeforeEvidence {
        session: String,
        finding_id: String,
        evidence: Box<RepairEvidenceBundle>,
    },
}
