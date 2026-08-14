use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    DistributedError, DistributedHistoryRun, DistributedPlanRequest, DistributedQuarantine,
    DistributedRunPlan, DistributedScenarioOutcome, DistributedScenarioSpec, DistributedShard,
    DistributedWorkerSpec, DISTRIBUTED_RUN_PROTOCOL, MAX_DISTRIBUTED_HISTORY_RUNS,
    MAX_DISTRIBUTED_SCENARIOS, MAX_DISTRIBUTED_WORKERS,
};

const ESTIMATE_HISTORY_WINDOW: usize = 20;

struct WorkerState {
    spec: DistributedWorkerSpec,
    lanes: Vec<u64>,
    scenario_ids: Vec<String>,
    surfaces: BTreeSet<crate::WorkerSurface>,
}

pub fn plan_distributed_run(
    mut request: DistributedPlanRequest,
) -> Result<DistributedRunPlan, DistributedError> {
    validate_request(&request)?;
    request
        .scenarios
        .sort_by(|left, right| left.id.cmp(&right.id));
    request
        .workers
        .sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
    request
        .history
        .sort_by_key(|run| (Reverse(run.finished_at_ms), run.run_id.clone()));
    for run in &mut request.history {
        run.scenarios.sort_by(|left, right| left.id.cmp(&right.id));
    }
    request
        .quarantines
        .sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));

    let estimates = duration_estimates(&request.scenarios, &request.history, &request.suite_digest);
    let mut eligible_counts = BTreeMap::new();
    for scenario in &request.scenarios {
        let count = request
            .workers
            .iter()
            .filter(|worker| worker.surfaces.contains(&scenario.surface))
            .count();
        if count == 0 {
            return Err(DistributedError::new(
                "test.distributed.worker_unavailable",
                format!(
                    "scenario '{}' has no worker with the required {:?} surface",
                    scenario.id, scenario.surface
                ),
            ));
        }
        eligible_counts.insert(scenario.id.clone(), count);
    }

    let mut ordered = request.scenarios.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        eligible_counts[&left.id]
            .cmp(&eligible_counts[&right.id])
            .then_with(|| estimates[&right.id].cmp(&estimates[&left.id]))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut workers = request
        .workers
        .into_iter()
        .map(|spec| WorkerState {
            lanes: vec![0; usize::from(spec.max_parallel_scenarios)],
            spec,
            scenario_ids: Vec::new(),
            surfaces: BTreeSet::new(),
        })
        .collect::<Vec<_>>();

    for scenario in ordered {
        let estimate = estimates[&scenario.id];
        let selected = workers
            .iter()
            .enumerate()
            .filter(|(_, worker)| worker.spec.surfaces.contains(&scenario.surface))
            .min_by_key(|(_, worker)| candidate_score(worker, estimate))
            .map(|(index, _)| index)
            .expect("eligible worker count was validated");
        assign(
            &mut workers[selected],
            &scenario.id,
            scenario.surface,
            estimate,
        );
    }

    let mut shards = Vec::new();
    for worker in workers
        .into_iter()
        .filter(|worker| !worker.scenario_ids.is_empty())
    {
        let index = u16::try_from(shards.len()).map_err(|_| {
            DistributedError::new(
                "test.distributed.worker_count_invalid",
                "distributed shard count exceeds its protocol bound",
            )
        })?;
        let mut scenario_ids = worker.scenario_ids;
        scenario_ids.sort();
        let required_surfaces = worker.surfaces.into_iter().collect::<Vec<_>>();
        let required_host_permission_digest = required_surfaces
            .contains(&crate::WorkerSurface::Gui)
            .then_some(worker.spec.host_permission_digest.clone())
            .flatten();
        shards.push(DistributedShard {
            index,
            worker_instance: worker.spec.instance_id,
            required_image_digest: worker.spec.image_digest,
            required_inventory_digest: worker.spec.inventory_digest,
            required_surfaces,
            required_host_permission_digest,
            max_parallel_scenarios: worker.spec.max_parallel_scenarios,
            predicted_duration_ms: worker.lanes.into_iter().max().unwrap_or_default(),
            scenario_ids,
        });
    }

    let mut plan = DistributedRunPlan {
        protocol: DISTRIBUTED_RUN_PROTOCOL.to_string(),
        plan_id: request.plan_id,
        plan_digest: String::new(),
        suite: request.suite,
        suite_digest: request.suite_digest,
        created_at_ms: request.created_at_ms,
        shards,
        quarantines: request.quarantines,
    };
    plan.plan_digest = digest(&plan)?;
    validate_plan(&plan)?;
    Ok(plan)
}

pub(super) fn validate_plan(plan: &DistributedRunPlan) -> Result<(), DistributedError> {
    if plan.protocol != DISTRIBUTED_RUN_PROTOCOL {
        return Err(DistributedError::new(
            "test.distributed.protocol_unsupported",
            format!("unsupported distributed run protocol {:?}", plan.protocol),
        ));
    }
    validate_identifier(&plan.plan_id, "plan ID")?;
    validate_digest(&plan.plan_digest, "plan digest")?;
    validate_text(&plan.suite, 256, "suite")?;
    validate_digest(&plan.suite_digest, "suite digest")?;
    if plan.created_at_ms == 0
        || plan.shards.is_empty()
        || plan.shards.len() > MAX_DISTRIBUTED_WORKERS
    {
        return Err(DistributedError::new(
            "test.distributed.plan_invalid",
            "distributed plan must have a positive creation time and a bounded non-empty shard set",
        ));
    }
    let mut worker_ids = BTreeSet::new();
    let mut scenario_ids = BTreeSet::new();
    for (expected_index, shard) in plan.shards.iter().enumerate() {
        let expected_index = u16::try_from(expected_index).map_err(|_| {
            DistributedError::new(
                "test.distributed.plan_invalid",
                "distributed shard index exceeds its protocol bound",
            )
        })?;
        validate_identifier(&shard.worker_instance, "shard worker instance")?;
        validate_digest(&shard.required_image_digest, "shard image digest")?;
        validate_digest(&shard.required_inventory_digest, "shard inventory digest")?;
        let gui_required = shard.required_surfaces.contains(&crate::WorkerSurface::Gui);
        if shard.required_surfaces.is_empty()
            || shard
                .required_surfaces
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || (gui_required && shard.required_host_permission_digest.is_none())
            || (!gui_required && shard.required_host_permission_digest.is_some())
            || (gui_required && shard.max_parallel_scenarios != 1)
        {
            return Err(DistributedError::new(
                "test.distributed.plan_invalid",
                "distributed shard surfaces and host permission binding are invalid",
            ));
        }
        if let Some(digest) = &shard.required_host_permission_digest {
            validate_digest(digest, "shard host permission digest")?;
        }
        if shard.index != expected_index
            || !worker_ids.insert(shard.worker_instance.as_str())
            || !(1..=64).contains(&shard.max_parallel_scenarios)
            || shard.predicted_duration_ms == 0
            || shard.scenario_ids.is_empty()
            || shard.scenario_ids.len() > MAX_DISTRIBUTED_SCENARIOS
            || shard.scenario_ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(DistributedError::new(
                "test.distributed.plan_invalid",
                "distributed shards must be canonical, unique, non-empty, and bounded",
            ));
        }
        for scenario_id in &shard.scenario_ids {
            validate_scenario_identifier(scenario_id, "planned scenario ID")?;
            if !scenario_ids.insert(scenario_id.as_str()) {
                return Err(DistributedError::new(
                    "test.distributed.plan_invalid",
                    format!("planned scenario '{scenario_id}' is assigned more than once"),
                ));
            }
        }
    }
    if scenario_ids.len() > MAX_DISTRIBUTED_SCENARIOS {
        return Err(DistributedError::new(
            "test.distributed.plan_invalid",
            "distributed plan exceeds the reviewed global scenario bound",
        ));
    }
    if plan
        .quarantines
        .windows(2)
        .any(|entries| entries[0].scenario_id >= entries[1].scenario_id)
    {
        return Err(DistributedError::new(
            "test.distributed.plan_invalid",
            "distributed plan quarantines must be unique and canonically ordered",
        ));
    }
    validate_quarantines(&plan.quarantines, &scenario_ids, plan.created_at_ms)?;
    let mut digest_input = plan.clone();
    digest_input.plan_digest.clear();
    if digest(&digest_input)? != plan.plan_digest {
        return Err(DistributedError::new(
            "test.distributed.plan_digest_mismatch",
            "distributed plan content does not match its digest",
        ));
    }
    Ok(())
}

fn validate_request(request: &DistributedPlanRequest) -> Result<(), DistributedError> {
    validate_identifier(&request.plan_id, "plan ID")?;
    validate_text(&request.suite, 256, "suite")?;
    validate_digest(&request.suite_digest, "suite digest")?;
    if request.created_at_ms == 0 {
        return Err(DistributedError::new(
            "test.distributed.time_invalid",
            "distributed plan creation time must be positive",
        ));
    }
    if request.scenarios.is_empty() || request.scenarios.len() > MAX_DISTRIBUTED_SCENARIOS {
        return Err(DistributedError::new(
            "test.distributed.scenario_count_invalid",
            "distributed scenario count is outside the reviewed bound",
        ));
    }
    if request.workers.is_empty() || request.workers.len() > MAX_DISTRIBUTED_WORKERS {
        return Err(DistributedError::new(
            "test.distributed.worker_count_invalid",
            "distributed worker count is outside the reviewed bound",
        ));
    }
    if request.history.len() > MAX_DISTRIBUTED_HISTORY_RUNS {
        return Err(DistributedError::new(
            "test.distributed.history_count_invalid",
            "distributed history exceeds its reviewed run bound",
        ));
    }

    let mut scenario_ids = BTreeSet::new();
    for scenario in &request.scenarios {
        validate_scenario_identifier(&scenario.id, "scenario ID")?;
        if scenario.fallback_duration_ms == 0 {
            return Err(DistributedError::new(
                "test.distributed.duration_invalid",
                format!("scenario '{}' has a zero fallback duration", scenario.id),
            ));
        }
        if !scenario_ids.insert(scenario.id.as_str()) {
            return Err(DistributedError::new(
                "test.distributed.scenario_duplicate",
                format!("scenario '{}' is duplicated", scenario.id),
            ));
        }
    }

    let mut worker_ids = BTreeSet::new();
    for worker in &request.workers {
        validate_identifier(&worker.instance_id, "worker instance ID")?;
        validate_digest(&worker.image_digest, "worker image digest")?;
        validate_digest(&worker.inventory_digest, "worker inventory digest")?;
        if !worker_ids.insert(worker.instance_id.as_str()) {
            return Err(DistributedError::new(
                "test.distributed.worker_duplicate",
                format!("worker '{}' is duplicated", worker.instance_id),
            ));
        }
        if !(1..=64).contains(&worker.max_parallel_scenarios)
            || worker.surfaces.is_empty()
            || worker.surfaces.len() > 3
            || worker.surfaces.windows(2).any(|pair| pair[0] >= pair[1])
            || (worker.surfaces.contains(&crate::WorkerSurface::Gui)
                && (worker.max_parallel_scenarios != 1 || worker.host_permission_digest.is_none()))
            || (!worker.surfaces.contains(&crate::WorkerSurface::Gui)
                && worker.host_permission_digest.is_some())
        {
            return Err(DistributedError::new(
                "test.distributed.worker_invalid",
                format!(
                    "worker '{}' has invalid capacity or surfaces",
                    worker.instance_id
                ),
            ));
        }
        if let Some(digest) = &worker.host_permission_digest {
            validate_digest(digest, "worker host permission digest")?;
        }
    }
    validate_history(&request.history)?;
    validate_quarantines(&request.quarantines, &scenario_ids, request.created_at_ms)
}

pub(super) fn validate_history(history: &[DistributedHistoryRun]) -> Result<(), DistributedError> {
    let mut run_ids = BTreeSet::new();
    for run in history {
        validate_identifier(&run.run_id, "history run ID")?;
        validate_digest(&run.suite_digest, "history suite digest")?;
        if run.finished_at_ms == 0
            || run.scenarios.is_empty()
            || run.scenarios.len() > MAX_DISTRIBUTED_SCENARIOS
            || !run_ids.insert(run.run_id.as_str())
        {
            return Err(DistributedError::new(
                "test.distributed.history_invalid",
                "history runs must have unique IDs, positive completion times, and bounded non-empty scenario sets",
            ));
        }
        let mut scenarios = BTreeSet::new();
        for scenario in &run.scenarios {
            validate_scenario_identifier(&scenario.id, "history scenario ID")?;
            if !scenarios.insert(scenario.id.as_str()) {
                return Err(DistributedError::new(
                    "test.distributed.history_invalid",
                    format!(
                        "history run '{}' contains duplicate scenario '{}'",
                        run.run_id, scenario.id
                    ),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_quarantines(
    quarantines: &[DistributedQuarantine],
    scenarios: &BTreeSet<&str>,
    at_ms: u64,
) -> Result<(), DistributedError> {
    let mut quarantined = BTreeSet::new();
    for quarantine in quarantines {
        validate_scenario_identifier(&quarantine.scenario_id, "quarantine scenario ID")?;
        validate_text(&quarantine.reason, 1_024, "quarantine reason")?;
        validate_text(&quarantine.owner, 256, "quarantine owner")?;
        validate_text(&quarantine.issue, 1_024, "quarantine issue")?;
        if !scenarios.contains(quarantine.scenario_id.as_str()) {
            return Err(DistributedError::new(
                "test.distributed.quarantine_target_missing",
                format!(
                    "quarantine target '{}' is absent from the suite",
                    quarantine.scenario_id
                ),
            ));
        }
        if quarantine.expires_at_ms <= at_ms {
            return Err(DistributedError::new(
                "test.distributed.quarantine_expired",
                format!("quarantine for '{}' has expired", quarantine.scenario_id),
            ));
        }
        if !quarantined.insert(quarantine.scenario_id.as_str()) {
            return Err(DistributedError::new(
                "test.distributed.quarantine_duplicate",
                format!(
                    "scenario '{}' is quarantined more than once",
                    quarantine.scenario_id
                ),
            ));
        }
    }
    Ok(())
}

fn duration_estimates(
    scenarios: &[DistributedScenarioSpec],
    history: &[DistributedHistoryRun],
    suite_digest: &str,
) -> BTreeMap<String, u64> {
    scenarios
        .iter()
        .map(|scenario| {
            let mut samples = history
                .iter()
                .filter(|run| run.suite_digest == suite_digest)
                .flat_map(|run| &run.scenarios)
                .filter(|sample| {
                    sample.id == scenario.id
                        && sample.duration_ms > 0
                        && matches!(
                            sample.outcome,
                            DistributedScenarioOutcome::Passed
                                | DistributedScenarioOutcome::TestFailed
                        )
                })
                .take(ESTIMATE_HISTORY_WINDOW)
                .map(|sample| sample.duration_ms)
                .collect::<Vec<_>>();
            samples.sort_unstable();
            let estimate = median(&samples).unwrap_or(scenario.fallback_duration_ms);
            (scenario.id.clone(), estimate)
        })
        .collect()
}

fn median(values: &[u64]) -> Option<u64> {
    let middle = values.len() / 2;
    match values.len() {
        0 => None,
        length if length % 2 == 1 => Some(values[middle]),
        _ => Some(values[middle - 1] + (values[middle] - values[middle - 1]) / 2),
    }
}

fn candidate_score(worker: &WorkerState, estimate: u64) -> (u64, u64, &str) {
    let mut lanes = worker.lanes.clone();
    let lane = lanes
        .iter()
        .enumerate()
        .min_by_key(|(index, duration)| (**duration, *index))
        .map(|(index, _)| index)
        .expect("workers have at least one lane");
    lanes[lane] = lanes[lane].saturating_add(estimate);
    (
        lanes.into_iter().max().unwrap_or_default(),
        worker
            .lanes
            .iter()
            .copied()
            .fold(0_u64, u64::saturating_add),
        worker.spec.instance_id.as_str(),
    )
}

fn assign(
    worker: &mut WorkerState,
    scenario_id: &str,
    surface: crate::WorkerSurface,
    estimate: u64,
) {
    let lane = worker
        .lanes
        .iter()
        .enumerate()
        .min_by_key(|(index, duration)| (**duration, *index))
        .map(|(index, _)| index)
        .expect("workers have at least one lane");
    worker.lanes[lane] = worker.lanes[lane].saturating_add(estimate);
    worker.scenario_ids.push(scenario_id.to_string());
    worker.surfaces.insert(surface);
}

fn digest(value: &impl Serialize) -> Result<String, DistributedError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        DistributedError::new(
            "test.distributed.plan_encode_failed",
            format!("failed to encode distributed plan: {error}"),
        )
    })?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

pub(super) fn validate_identifier(value: &str, label: &str) -> Result<(), DistributedError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(DistributedError::new(
            "test.distributed.identifier_invalid",
            format!("{label} must be a bounded portable identifier"),
        ));
    }
    Ok(())
}

pub(super) fn validate_scenario_identifier(
    value: &str,
    label: &str,
) -> Result<(), DistributedError> {
    validate_identifier(value, label)?;
    if value.len() > 64 {
        return Err(DistributedError::new(
            "test.distributed.identifier_invalid",
            format!("{label} must fit the A3S Test scenario identifier bound"),
        ));
    }
    Ok(())
}

pub(super) fn validate_digest(value: &str, label: &str) -> Result<(), DistributedError> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if !valid {
        return Err(DistributedError::new(
            "test.distributed.digest_invalid",
            format!("{label} must be a canonical lowercase SHA-256 digest"),
        ));
    }
    Ok(())
}

fn validate_text(value: &str, max_bytes: usize, label: &str) -> Result<(), DistributedError> {
    if value.trim().is_empty() || value.len() > max_bytes {
        return Err(DistributedError::new(
            "test.distributed.text_invalid",
            format!("{label} must be bounded and non-empty"),
        ));
    }
    Ok(())
}
