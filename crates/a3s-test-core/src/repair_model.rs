use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairFinding {
    pub id: String,
    #[serde(rename = "batchId")]
    pub batch_id: String,
    pub instruction: String,
    #[serde(rename = "successCriteria")]
    pub success_criteria: Option<String>,
    pub intent: RepairIntent,
    pub severity: RepairSeverity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RepairRelation>,
    #[serde(
        rename = "designReference",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub design_reference: Option<RepairDesignReference>,
    pub target: RepairTarget,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "pageId")]
    pub page_id: String,
    pub url: String,
    #[serde(rename = "contextRevision")]
    pub context_revision: u64,
    pub context: Value,
    pub status: RepairStatus,
    #[serde(rename = "submittedAt")]
    pub submitted_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepairRelation {
    ConflictsWith {
        #[serde(rename = "findingId")]
        finding_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairDesignReference {
    pub kind: RepairDesignReferenceKind,
    pub width: u32,
    pub height: u32,
    pub image: RepairDesignReferenceImage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairDesignReferenceKind {
    Sketch,
    Screenshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepairDesignReferenceImage {
    Inline {
        #[serde(rename = "mediaType")]
        media_type: String,
        #[serde(rename = "dataUrl")]
        data_url: String,
    },
    Artifact {
        evidence: Evidence,
        sha256: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairIntent {
    Fix,
    Change,
    Question,
    Approve,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairSeverity {
    Blocking,
    Important,
    Suggestion,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairTarget {
    pub kind: RepairTargetKind,
    #[serde(rename = "nodeIds")]
    pub node_ids: Vec<String>,
    #[serde(rename = "selectedText")]
    pub selected_text: Option<String>,
    pub region: Option<PageContextRect>,
    pub drawing: Option<Vec<PageContextPoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<RepairLayoutIntent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepairLayoutIntent {
    Placement {
        #[serde(rename = "componentType")]
        component_type: String,
        canvas: RepairLayoutCanvas,
        purpose: Option<String>,
    },
    Rearrange {
        #[serde(rename = "originalRegion")]
        original_region: PageContextRect,
        purpose: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairLayoutCanvas {
    Page,
    Wireframe,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairTargetKind {
    Node,
    Text,
    Region,
    Drawing,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairStatus {
    Draft,
    Queued,
    Claimed,
    Repairing,
    Verifying,
    NeedsInput,
    VerificationFailed,
    ReviewReady,
    Resolved,
    Dismissed,
    Cancelled,
    Failed,
    Reopened,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairActor {
    Human,
    Agent,
    #[serde(rename = "a3s-test", alias = "a3s_test")]
    A3sTest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairStatusEvent {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "findingId")]
    pub finding_id: String,
    pub sequence: u64,
    pub status: RepairStatus,
    pub actor: RepairActor,
    pub timestamp: String,
    pub summary: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairHumanActionKind {
    Reply,
    Accept,
    Dismiss,
    Reopen,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairHumanAction {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "findingId")]
    pub finding_id: String,
    pub action: RepairHumanActionKind,
    pub timestamp: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairCheckResult {
    pub command: String,
    pub status: RepairCheckStatus,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairEvidencePhase {
    Before,
    After,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairEvidenceRequest {
    #[serde(rename = "findingId")]
    pub finding_id: String,
    #[serde(rename = "attemptId")]
    pub attempt_id: Option<String>,
    pub phase: RepairEvidencePhase,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairEvidenceBundle {
    #[serde(rename = "capturedAtMs")]
    pub captured_at_ms: u64,
    #[serde(rename = "contextRevision")]
    pub context_revision: u64,
    #[serde(rename = "contextSha256")]
    pub context_sha256: String,
    pub context: PageContextSnapshot,
    #[serde(rename = "consoleErrors")]
    pub console_errors: u32,
    #[serde(rename = "pageErrors")]
    pub page_errors: u32,
    pub screenshot: Evidence,
    #[serde(rename = "screenshotSha256")]
    pub screenshot_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairAclProof {
    pub path: String,
    pub passed: bool,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairCheckStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairVerificationScope {
    Focused,
    Expanded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairVerificationExpansionReason {
    SourceMappingUnavailable,
    StableLocatorUnavailable,
    ChangedFilesUnavailable,
    ChangedFileOutsideSourceMapping,
    ProjectCheckCoverageMissing,
    NewBrowserErrors,
    PriorProofFailed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairVerificationSlice {
    pub protocol: String,
    pub scope: RepairVerificationScope,
    #[serde(rename = "sourceFiles")]
    pub source_files: Vec<String>,
    #[serde(rename = "stableLocator")]
    pub stable_locator: bool,
    #[serde(rename = "priorAclProofPassed")]
    pub prior_acl_proof_passed: Option<bool>,
    #[serde(rename = "selectedChecks")]
    pub selected_checks: Vec<String>,
    #[serde(rename = "expansionReasons")]
    pub expansion_reasons: Vec<RepairVerificationExpansionReason>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairVerification {
    #[serde(rename = "findingId")]
    pub finding_id: String,
    #[serde(rename = "attemptId")]
    pub attempt_id: String,
    #[serde(rename = "beforeRevision")]
    pub before_revision: u64,
    #[serde(rename = "afterRevision")]
    pub after_revision: u64,
    #[serde(rename = "targetFound")]
    pub target_found: bool,
    #[serde(rename = "successCriteriaPassed")]
    #[serde(default)]
    pub success_criteria_passed: Option<bool>,
    #[serde(rename = "newConsoleErrors")]
    pub new_console_errors: u32,
    #[serde(rename = "newPageErrors")]
    pub new_page_errors: u32,
    #[serde(rename = "changedFiles")]
    pub changed_files: Vec<String>,
    pub checks: Vec<RepairCheckResult>,
    #[serde(rename = "aclCandidate")]
    #[serde(default)]
    pub acl_candidate: Option<String>,
    #[serde(rename = "aclProof")]
    #[serde(default)]
    pub acl_proof: Option<RepairAclProof>,
    #[serde(rename = "beforeEvidence")]
    #[serde(default)]
    pub before_evidence: Option<RepairEvidenceBundle>,
    #[serde(rename = "afterEvidence")]
    #[serde(default)]
    pub after_evidence: Option<RepairEvidenceBundle>,
    #[serde(
        rename = "verificationSlice",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub verification_slice: Option<RepairVerificationSlice>,
    pub passed: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairChange {
    #[serde(rename = "attemptId")]
    pub attempt_id: String,
    #[serde(rename = "reportedAtMs")]
    pub reported_at_ms: u64,
    #[serde(rename = "changedFiles")]
    pub changed_files: Vec<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairAttempt {
    pub id: String,
    #[serde(rename = "startedAtMs")]
    pub started_at_ms: u64,
    #[serde(rename = "finishedAtMs")]
    pub finished_at_ms: Option<u64>,
    pub status: RepairStatus,
    pub replies: Vec<RepairThreadMessage>,
    pub verification: Option<RepairVerification>,
    #[serde(rename = "beforeEvidence")]
    pub before_evidence: Option<RepairEvidenceBundle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<RepairChange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairThreadMessage {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub actor: RepairActor,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: u64,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairBatch {
    pub id: String,
    #[serde(rename = "findingIds")]
    pub finding_ids: Vec<String>,
    pub status: RepairBatchStatus,
    pub results: Vec<RepairBatchItemResult>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairBatchStatus {
    Queued,
    InProgress,
    NeedsInput,
    ReviewReady,
    Resolved,
    CompletedWithFailures,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairBatchItemResult {
    #[serde(rename = "findingId")]
    pub finding_id: String,
    pub status: RepairStatus,
}
