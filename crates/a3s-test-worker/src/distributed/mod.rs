mod analysis;
mod model;
mod planner;

pub use analysis::analyze_distributed_run;
pub use model::*;
pub use planner::plan_distributed_run;

use schemars::Schema;
use serde::Serialize;

pub const DISTRIBUTED_RUN_PROTOCOL: &str = "a3s.test.distributed-run/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct DistributedRunProtocolInvariants {
    pub deterministic_sharding: bool,
    pub exact_worker_identity_binding: bool,
    pub exact_scenario_selection: bool,
    pub digest_bound_plan: bool,
    pub accountable_quarantine: bool,
    pub infrastructure_failures_never_quarantined: bool,
    pub bounded_history: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DistributedRunProtocolSchema {
    pub protocol: &'static str,
    pub invariants: DistributedRunProtocolInvariants,
    pub plan_request_schema: Schema,
    pub plan_schema: Schema,
    pub analysis_request_schema: Schema,
    pub analysis_schema: Schema,
}

#[must_use]
pub fn distributed_run_protocol_schema() -> DistributedRunProtocolSchema {
    DistributedRunProtocolSchema {
        protocol: DISTRIBUTED_RUN_PROTOCOL,
        invariants: DistributedRunProtocolInvariants {
            deterministic_sharding: true,
            exact_worker_identity_binding: true,
            exact_scenario_selection: true,
            digest_bound_plan: true,
            accountable_quarantine: true,
            infrastructure_failures_never_quarantined: true,
            bounded_history: true,
        },
        plan_request_schema: schemars::schema_for!(DistributedPlanRequest),
        plan_schema: schemars::schema_for!(DistributedRunPlan),
        analysis_request_schema: schemars::schema_for!(DistributedAnalysisRequest),
        analysis_schema: schemars::schema_for!(DistributedRunAnalysis),
    }
}
