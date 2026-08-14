use std::collections::{BTreeMap, BTreeSet};

use super::planner::{
    validate_digest, validate_history, validate_identifier, validate_quarantines,
    validate_scenario_identifier,
};
use super::{
    DistributedAnalysisRequest, DistributedDisposition, DistributedError, DistributedFlakeSummary,
    DistributedHistoryRun, DistributedHistoryScenario, DistributedRunAnalysis,
    DistributedRunCounts, DistributedRunStatus, DistributedScenarioAnalysis,
    DistributedScenarioOutcome, HistoricalChange, DISTRIBUTED_RUN_PROTOCOL,
    MAX_DISTRIBUTED_HISTORY_RUNS, MAX_DISTRIBUTED_SCENARIOS, MAX_HISTORY_WINDOW,
};

pub fn analyze_distributed_run(
    mut request: DistributedAnalysisRequest,
) -> Result<DistributedRunAnalysis, DistributedError> {
    validate_request(&request)?;
    request
        .scenarios
        .sort_by(|left, right| left.id.cmp(&right.id));
    request
        .quarantines
        .sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    request
        .history
        .sort_by_key(|run| (std::cmp::Reverse(run.finished_at_ms), run.run_id.clone()));
    for run in &mut request.history {
        run.scenarios.sort_by(|left, right| left.id.cmp(&right.id));
    }
    request.shard_issues.sort_by_key(|issue| issue.shard_index);

    let baseline = request.history.first();
    let baseline_scenarios = baseline
        .map(|run| {
            run.scenarios
                .iter()
                .map(|scenario| (scenario.id.as_str(), scenario.outcome))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let quarantine_by_scenario = request
        .quarantines
        .iter()
        .map(|entry| (entry.scenario_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();

    let mut counts = DistributedRunCounts::default();
    let mut analyses = Vec::with_capacity(request.scenarios.len());
    for scenario in &request.scenarios {
        let disposition = match (
            scenario.outcome,
            quarantine_by_scenario.get(scenario.id.as_str()),
        ) {
            (DistributedScenarioOutcome::TestFailed, Some(_)) => {
                DistributedDisposition::QuarantinedFailure
            }
            (DistributedScenarioOutcome::Passed, Some(_)) => {
                DistributedDisposition::QuarantinedPass
            }
            _ => DistributedDisposition::Required,
        };
        update_counts(&mut counts, scenario.outcome, disposition);
        let change = historical_change(
            baseline_scenarios.get(scenario.id.as_str()).copied(),
            scenario.outcome,
        );
        analyses.push(DistributedScenarioAnalysis {
            id: scenario.id.clone(),
            outcome: scenario.outcome,
            disposition,
            duration_ms: scenario.duration_ms,
            failure_code: scenario.failure_code.clone(),
            change,
            flake: flake_summary(
                &scenario.id,
                scenario.outcome,
                &request.history,
                &request.suite_digest,
                request.history_window,
            ),
        });
    }
    let current_ids = request
        .scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect::<BTreeSet<_>>();
    let removed_scenarios = baseline
        .map(|run| {
            run.scenarios
                .iter()
                .filter(|scenario| !current_ids.contains(scenario.id.as_str()))
                .map(|scenario| scenario.id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let status = aggregate_status(&analyses);
    let history_record = DistributedHistoryRun {
        run_id: request.run_id.clone(),
        suite_digest: request.suite_digest.clone(),
        finished_at_ms: request.finished_at_ms,
        scenarios: request
            .scenarios
            .iter()
            .map(|scenario| DistributedHistoryScenario {
                id: scenario.id.clone(),
                outcome: scenario.outcome,
                duration_ms: scenario.duration_ms,
            })
            .collect(),
    };

    Ok(DistributedRunAnalysis {
        protocol: DISTRIBUTED_RUN_PROTOCOL.to_string(),
        plan_id: request.plan_id,
        plan_digest: request.plan_digest,
        run_id: request.run_id,
        suite: request.suite,
        suite_digest: request.suite_digest,
        started_at_ms: request.started_at_ms,
        finished_at_ms: request.finished_at_ms,
        status,
        baseline_run_id: baseline.map(|run| run.run_id.clone()),
        counts,
        scenarios: analyses,
        removed_scenarios,
        shard_issues: request.shard_issues,
        history_record,
    })
}

fn validate_request(request: &DistributedAnalysisRequest) -> Result<(), DistributedError> {
    validate_identifier(&request.plan_id, "distributed plan ID")?;
    validate_digest(&request.plan_digest, "distributed plan digest")?;
    validate_identifier(&request.run_id, "distributed run ID")?;
    if request.suite.trim().is_empty() || request.suite.len() > 256 {
        return Err(DistributedError::new(
            "test.distributed.suite_invalid",
            "distributed suite name must be bounded and non-empty",
        ));
    }
    validate_digest(&request.suite_digest, "suite digest")?;
    if request.started_at_ms == 0 || request.finished_at_ms < request.started_at_ms {
        return Err(DistributedError::new(
            "test.distributed.time_invalid",
            "distributed run timestamps must be positive and ordered",
        ));
    }
    if request.history_window == 0 || request.history_window > MAX_HISTORY_WINDOW {
        return Err(DistributedError::new(
            "test.distributed.history_window_invalid",
            "history window must be between 1 and 100 runs",
        ));
    }
    if request.scenarios.is_empty() || request.scenarios.len() > MAX_DISTRIBUTED_SCENARIOS {
        return Err(DistributedError::new(
            "test.distributed.scenario_count_invalid",
            "distributed scenario observation count is outside the reviewed bound",
        ));
    }
    if request.history.len() > MAX_DISTRIBUTED_HISTORY_RUNS {
        return Err(DistributedError::new(
            "test.distributed.history_count_invalid",
            "distributed history exceeds its reviewed run bound",
        ));
    }
    let mut scenarios = BTreeSet::new();
    for scenario in &request.scenarios {
        validate_scenario_identifier(&scenario.id, "scenario observation ID")?;
        if !scenarios.insert(scenario.id.as_str()) {
            return Err(DistributedError::new(
                "test.distributed.scenario_duplicate",
                format!("scenario observation '{}' is duplicated", scenario.id),
            ));
        }
        if scenario
            .failure_code
            .as_ref()
            .is_some_and(|code| code.trim().is_empty() || code.len() > 128)
        {
            return Err(DistributedError::new(
                "test.distributed.failure_code_invalid",
                format!("scenario '{}' has an invalid failure code", scenario.id),
            ));
        }
    }
    validate_history(&request.history)?;
    if request
        .history
        .iter()
        .any(|run| run.finished_at_ms >= request.finished_at_ms)
    {
        return Err(DistributedError::new(
            "test.distributed.history_invalid",
            "historical runs must precede the current completion time",
        ));
    }
    // Quarantine admission is frozen at run start and bound into the plan
    // digest. Expiry during an already-admitted run must not make analysis
    // nondeterministic.
    validate_quarantines(&request.quarantines, &scenarios, request.started_at_ms)?;
    let mut shard_indexes = BTreeSet::new();
    for issue in &request.shard_issues {
        validate_identifier(&issue.worker_instance, "shard issue worker instance")?;
        if issue.code.trim().is_empty()
            || issue.code.len() > 128
            || issue.message.trim().is_empty()
            || issue.message.len() > 2_048
            || !shard_indexes.insert(issue.shard_index)
        {
            return Err(DistributedError::new(
                "test.distributed.shard_issue_invalid",
                "shard issues must be bounded and unique by shard index",
            ));
        }
    }
    Ok(())
}

fn historical_change(
    previous: Option<DistributedScenarioOutcome>,
    current: DistributedScenarioOutcome,
) -> HistoricalChange {
    match (previous, current) {
        (None, _) => HistoricalChange::New,
        (Some(previous), current) if previous == current => HistoricalChange::Unchanged,
        (Some(DistributedScenarioOutcome::Passed), DistributedScenarioOutcome::TestFailed) => {
            HistoricalChange::Regression
        }
        (Some(DistributedScenarioOutcome::TestFailed), DistributedScenarioOutcome::Passed) => {
            HistoricalChange::Fixed
        }
        _ => HistoricalChange::InfrastructureChange,
    }
}

fn flake_summary(
    scenario_id: &str,
    current: DistributedScenarioOutcome,
    history: &[DistributedHistoryRun],
    suite_digest: &str,
    window: usize,
) -> DistributedFlakeSummary {
    let mut outcomes = vec![current];
    outcomes.extend(
        history
            .iter()
            .filter(|run| run.suite_digest == suite_digest)
            .filter_map(|run| {
                run.scenarios
                    .iter()
                    .find(|scenario| scenario.id == scenario_id)
                    .map(|scenario| scenario.outcome)
            })
            .take(window.saturating_sub(1)),
    );
    let mut summary = DistributedFlakeSummary {
        observations: u32::try_from(outcomes.len()).unwrap_or(u32::MAX),
        passed: 0,
        test_failed: 0,
        infrastructure_failed: 0,
        timed_out: 0,
        cancelled: 0,
        flaky: false,
    };
    for outcome in outcomes {
        match outcome {
            DistributedScenarioOutcome::Passed => summary.passed += 1,
            DistributedScenarioOutcome::TestFailed => summary.test_failed += 1,
            DistributedScenarioOutcome::InfrastructureFailed
            | DistributedScenarioOutcome::Interrupted => summary.infrastructure_failed += 1,
            DistributedScenarioOutcome::TimedOut => summary.timed_out += 1,
            DistributedScenarioOutcome::Cancelled => summary.cancelled += 1,
        }
    }
    summary.flaky = summary.passed > 0 && summary.test_failed > 0;
    summary
}

fn update_counts(
    counts: &mut DistributedRunCounts,
    outcome: DistributedScenarioOutcome,
    disposition: DistributedDisposition,
) {
    match (outcome, disposition) {
        (DistributedScenarioOutcome::Passed, DistributedDisposition::QuarantinedPass) => {
            counts.quarantined_passed += 1;
        }
        (DistributedScenarioOutcome::Passed, _) => counts.passed += 1,
        (DistributedScenarioOutcome::TestFailed, DistributedDisposition::QuarantinedFailure) => {
            counts.quarantined_failed += 1;
        }
        (DistributedScenarioOutcome::TestFailed, _) => counts.failed += 1,
        (
            DistributedScenarioOutcome::InfrastructureFailed
            | DistributedScenarioOutcome::Interrupted,
            _,
        ) => counts.infrastructure_failed += 1,
        (DistributedScenarioOutcome::TimedOut, _) => counts.timed_out += 1,
        (DistributedScenarioOutcome::Cancelled, _) => counts.cancelled += 1,
    }
}

fn aggregate_status(scenarios: &[DistributedScenarioAnalysis]) -> DistributedRunStatus {
    let mut status = DistributedRunStatus::Passed;
    for scenario in scenarios {
        let candidate = match (scenario.outcome, scenario.disposition) {
            (DistributedScenarioOutcome::Cancelled, _) => DistributedRunStatus::Cancelled,
            (DistributedScenarioOutcome::TimedOut, _) => DistributedRunStatus::TimedOut,
            (
                DistributedScenarioOutcome::InfrastructureFailed
                | DistributedScenarioOutcome::Interrupted,
                _,
            ) => DistributedRunStatus::InfrastructureFailed,
            (DistributedScenarioOutcome::TestFailed, DistributedDisposition::Required) => {
                DistributedRunStatus::Failed
            }
            _ => DistributedRunStatus::Passed,
        };
        if status_priority(candidate) > status_priority(status) {
            status = candidate;
        }
    }
    status
}

fn status_priority(status: DistributedRunStatus) -> u8 {
    match status {
        DistributedRunStatus::Passed => 0,
        DistributedRunStatus::Failed => 1,
        DistributedRunStatus::InfrastructureFailed => 2,
        DistributedRunStatus::TimedOut => 3,
        DistributedRunStatus::Cancelled => 4,
    }
}
