use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::WorkerSurface;

use super::DISTRIBUTED_RUN_PROTOCOL;

pub const MAX_DISTRIBUTED_SCENARIOS: usize = 4_096;
pub const MAX_DISTRIBUTED_WORKERS: usize = 64;
pub const MAX_DISTRIBUTED_HISTORY_RUNS: usize = 200;
pub const MAX_HISTORY_WINDOW: usize = 100;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedScenarioSpec {
    pub id: String,
    pub surface: WorkerSurface,
    pub fallback_duration_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedWorkerSpec {
    pub instance_id: String,
    pub image_digest: String,
    pub inventory_digest: String,
    pub max_parallel_scenarios: u16,
    pub surfaces: Vec<WorkerSurface>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedQuarantine {
    pub scenario_id: String,
    pub reason: String,
    pub owner: String,
    pub issue: String,
    pub expires_at_ms: u64,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DistributedScenarioOutcome {
    Passed,
    TestFailed,
    InfrastructureFailed,
    TimedOut,
    Cancelled,
    Interrupted,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedHistoryScenario {
    pub id: String,
    pub outcome: DistributedScenarioOutcome,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedHistoryRun {
    pub run_id: String,
    pub suite_digest: String,
    pub finished_at_ms: u64,
    pub scenarios: Vec<DistributedHistoryScenario>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedPlanRequest {
    pub plan_id: String,
    pub suite: String,
    pub suite_digest: String,
    pub created_at_ms: u64,
    pub scenarios: Vec<DistributedScenarioSpec>,
    pub workers: Vec<DistributedWorkerSpec>,
    pub history: Vec<DistributedHistoryRun>,
    pub quarantines: Vec<DistributedQuarantine>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedShard {
    pub index: u16,
    pub worker_instance: String,
    pub required_image_digest: String,
    pub required_inventory_digest: String,
    pub max_parallel_scenarios: u16,
    pub predicted_duration_ms: u64,
    pub scenario_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedRunPlan {
    pub protocol: String,
    pub plan_id: String,
    pub plan_digest: String,
    pub suite: String,
    pub suite_digest: String,
    pub created_at_ms: u64,
    pub shards: Vec<DistributedShard>,
    pub quarantines: Vec<DistributedQuarantine>,
}

impl DistributedRunPlan {
    #[must_use]
    pub fn protocol() -> &'static str {
        DISTRIBUTED_RUN_PROTOCOL
    }

    pub fn validate(&self) -> Result<(), DistributedError> {
        super::planner::validate_plan(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedScenarioObservation {
    pub id: String,
    pub outcome: DistributedScenarioOutcome,
    pub duration_ms: u64,
    pub failure_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedAnalysisRequest {
    pub plan_id: String,
    pub plan_digest: String,
    pub run_id: String,
    pub suite: String,
    pub suite_digest: String,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub history_window: usize,
    pub scenarios: Vec<DistributedScenarioObservation>,
    pub quarantines: Vec<DistributedQuarantine>,
    pub history: Vec<DistributedHistoryRun>,
    pub shard_issues: Vec<DistributedShardIssue>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedShardIssue {
    pub shard_index: u16,
    pub worker_instance: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedDisposition {
    Required,
    QuarantinedFailure,
    QuarantinedPass,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalChange {
    New,
    Unchanged,
    Regression,
    Fixed,
    InfrastructureChange,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedRunStatus {
    Passed,
    Failed,
    InfrastructureFailed,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedFlakeSummary {
    pub observations: u32,
    pub passed: u32,
    pub test_failed: u32,
    pub infrastructure_failed: u32,
    pub timed_out: u32,
    pub cancelled: u32,
    pub flaky: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedScenarioAnalysis {
    pub id: String,
    pub outcome: DistributedScenarioOutcome,
    pub disposition: DistributedDisposition,
    pub duration_ms: u64,
    pub failure_code: Option<String>,
    pub change: HistoricalChange,
    pub flake: DistributedFlakeSummary,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedRunCounts {
    pub passed: u32,
    pub failed: u32,
    pub quarantined_failed: u32,
    pub quarantined_passed: u32,
    pub infrastructure_failed: u32,
    pub timed_out: u32,
    pub cancelled: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedRunAnalysis {
    pub protocol: String,
    pub plan_id: String,
    pub plan_digest: String,
    pub run_id: String,
    pub suite: String,
    pub suite_digest: String,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub status: DistributedRunStatus,
    pub baseline_run_id: Option<String>,
    pub counts: DistributedRunCounts,
    pub scenarios: Vec<DistributedScenarioAnalysis>,
    pub removed_scenarios: Vec<String>,
    pub shard_issues: Vec<DistributedShardIssue>,
    pub history_record: DistributedHistoryRun,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct DistributedError {
    code: &'static str,
    message: String,
}

impl DistributedError {
    pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}
