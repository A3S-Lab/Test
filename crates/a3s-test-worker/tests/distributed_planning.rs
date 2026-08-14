use a3s_test_worker::{
    analyze_distributed_run, distributed_run_protocol_schema, plan_distributed_run,
    DistributedAnalysisRequest, DistributedDisposition, DistributedHistoryRun,
    DistributedHistoryScenario, DistributedPlanRequest, DistributedQuarantine,
    DistributedRunStatus, DistributedScenarioObservation, DistributedScenarioOutcome,
    DistributedScenarioSpec, DistributedWorkerSpec, HistoricalChange, WorkerSurface,
    DISTRIBUTED_RUN_PROTOCOL,
};

const SUITE_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const IMAGE_DIGEST: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const INVENTORY_DIGEST: &str =
    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn worker(instance_id: &str, surfaces: Vec<WorkerSurface>) -> DistributedWorkerSpec {
    DistributedWorkerSpec {
        instance_id: instance_id.to_string(),
        image_digest: IMAGE_DIGEST.to_string(),
        inventory_digest: INVENTORY_DIGEST.to_string(),
        max_parallel_scenarios: 1,
        surfaces,
    }
}

fn scenario(
    id: &str,
    surface: WorkerSurface,
    fallback_duration_ms: u64,
) -> DistributedScenarioSpec {
    DistributedScenarioSpec {
        id: id.to_string(),
        surface,
        fallback_duration_ms,
    }
}

fn request() -> DistributedPlanRequest {
    DistributedPlanRequest {
        plan_id: "plan-001".to_string(),
        suite: "checkout".to_string(),
        suite_digest: SUITE_DIGEST.to_string(),
        created_at_ms: 1_800_000_000_000,
        scenarios: vec![
            scenario("fast-web", WorkerSurface::Web, 10_000),
            scenario("terminal", WorkerSurface::Tui, 20_000),
            scenario("slow-web", WorkerSurface::Web, 90_000),
        ],
        workers: vec![
            worker("multi", vec![WorkerSurface::Web, WorkerSurface::Tui]),
            worker("web-only", vec![WorkerSurface::Web]),
        ],
        history: Vec::new(),
        quarantines: Vec::new(),
    }
}

#[test]
fn distributed_protocol_schema_states_determinism_and_quarantine_boundaries() {
    let protocol = distributed_run_protocol_schema();
    assert_eq!(protocol.protocol, DISTRIBUTED_RUN_PROTOCOL);
    assert!(protocol.invariants.deterministic_sharding);
    assert!(protocol.invariants.exact_worker_identity_binding);
    assert!(protocol.invariants.exact_scenario_selection);
    assert!(protocol.invariants.digest_bound_plan);
    assert!(protocol.invariants.accountable_quarantine);
    assert!(
        protocol
            .invariants
            .infrastructure_failures_never_quarantined
    );
    let plan = serde_json::to_value(protocol.plan_schema).expect("plan schema");
    let analysis = serde_json::to_value(protocol.analysis_schema).expect("analysis schema");
    assert_eq!(plan["additionalProperties"], false);
    assert_eq!(analysis["additionalProperties"], false);
}

#[test]
fn planner_assigns_every_scenario_once_and_preserves_scarce_surface_capacity() {
    let plan = plan_distributed_run(request()).expect("distributed plan");

    assert_eq!(plan.protocol, DISTRIBUTED_RUN_PROTOCOL);
    assert_eq!(plan.shards.len(), 2);
    let multi = plan
        .shards
        .iter()
        .find(|shard| shard.worker_instance == "multi")
        .expect("multi-surface shard");
    let web = plan
        .shards
        .iter()
        .find(|shard| shard.worker_instance == "web-only")
        .expect("Web shard");
    assert_eq!(multi.scenario_ids, vec!["fast-web", "terminal"]);
    assert_eq!(multi.predicted_duration_ms, 30_000);
    assert_eq!(web.scenario_ids, vec!["slow-web"]);
    assert_eq!(web.predicted_duration_ms, 90_000);

    let mut assigned = plan
        .shards
        .iter()
        .flat_map(|shard| shard.scenario_ids.iter().cloned())
        .collect::<Vec<_>>();
    assigned.sort();
    assert_eq!(assigned, vec!["fast-web", "slow-web", "terminal"]);
}

#[test]
fn generated_plan_validates_and_rejects_content_tampering() {
    let mut plan = plan_distributed_run(request()).expect("distributed plan");
    plan.validate().expect("generated plan validates");

    plan.shards[0].predicted_duration_ms += 1;
    let error = plan.validate().expect_err("tampered plan digest");
    assert_eq!(error.code(), "test.distributed.plan_digest_mismatch");
}

#[test]
fn plan_validation_enforces_global_scenario_and_quarantine_order_bounds() {
    let mut oversized = plan_distributed_run(request()).expect("distributed plan");
    let scenario_ids = (0..=4_096)
        .map(|index| format!("scenario-{index:04}"))
        .collect::<Vec<_>>();
    oversized.shards[0].scenario_ids = scenario_ids[..2_048].to_vec();
    oversized.shards[1].scenario_ids = scenario_ids[2_048..].to_vec();
    let error = oversized
        .validate()
        .expect_err("global scenario count must be bounded");
    assert_eq!(error.code(), "test.distributed.plan_invalid");

    let mut quarantined = request();
    quarantined.quarantines = vec![
        DistributedQuarantine {
            scenario_id: "fast-web".to_string(),
            reason: "Known fast failure".to_string(),
            owner: "checkout-team".to_string(),
            issue: "https://issues.example.test/fast".to_string(),
            expires_at_ms: quarantined.created_at_ms + 60_000,
        },
        DistributedQuarantine {
            scenario_id: "slow-web".to_string(),
            reason: "Known slow failure".to_string(),
            owner: "checkout-team".to_string(),
            issue: "https://issues.example.test/slow".to_string(),
            expires_at_ms: quarantined.created_at_ms + 60_000,
        },
    ];
    let mut plan = plan_distributed_run(quarantined).expect("quarantined plan");
    plan.quarantines.reverse();
    let error = plan
        .validate()
        .expect_err("quarantines must remain canonical");
    assert_eq!(error.code(), "test.distributed.plan_invalid");
}

#[test]
fn planner_saturates_extreme_duration_scores_without_overflowing() {
    let request = DistributedPlanRequest {
        plan_id: "plan-extreme".to_string(),
        suite: "extreme".to_string(),
        suite_digest: SUITE_DIGEST.to_string(),
        created_at_ms: 1_800_000_000_000,
        scenarios: (0..4)
            .map(|index| scenario(&format!("scenario-{index}"), WorkerSurface::Web, u64::MAX))
            .collect(),
        workers: vec![
            DistributedWorkerSpec {
                max_parallel_scenarios: 2,
                ..worker("runner-a", vec![WorkerSurface::Web])
            },
            DistributedWorkerSpec {
                max_parallel_scenarios: 2,
                ..worker("runner-b", vec![WorkerSurface::Web])
            },
        ],
        history: Vec::new(),
        quarantines: Vec::new(),
    };
    let plan = plan_distributed_run(request).expect("extreme-duration plan");
    plan.validate().expect("extreme-duration plan validates");
    assert_eq!(plan.shards.len(), 2);
    assert!(plan
        .shards
        .iter()
        .all(|shard| shard.predicted_duration_ms == u64::MAX));
}

#[test]
fn planner_is_deterministic_across_input_order_and_uses_recent_duration_medians() {
    let mut first = request();
    first.history = vec![DistributedHistoryRun {
        run_id: "history-1".to_string(),
        suite_digest: SUITE_DIGEST.to_string(),
        finished_at_ms: 1_799_999_000_000,
        scenarios: vec![
            DistributedHistoryScenario {
                id: "fast-web".to_string(),
                outcome: DistributedScenarioOutcome::Passed,
                duration_ms: 70_000,
            },
            DistributedHistoryScenario {
                id: "slow-web".to_string(),
                outcome: DistributedScenarioOutcome::Passed,
                duration_ms: 5_000,
            },
            DistributedHistoryScenario {
                id: "terminal".to_string(),
                outcome: DistributedScenarioOutcome::Passed,
                duration_ms: 20_000,
            },
        ],
    }];
    let expected = plan_distributed_run(first.clone()).expect("first plan");

    first.scenarios.reverse();
    first.workers.reverse();
    first.history[0].scenarios.reverse();
    let reordered = plan_distributed_run(first).expect("reordered plan");

    assert_eq!(expected.plan_digest, reordered.plan_digest);
    assert_eq!(expected.shards, reordered.shards);
    let web = expected
        .shards
        .iter()
        .find(|shard| shard.worker_instance == "web-only")
        .expect("Web shard");
    assert_eq!(web.scenario_ids, vec!["fast-web"]);
    assert_eq!(web.predicted_duration_ms, 70_000);
}

#[test]
fn planner_rejects_unrunnable_scenarios_and_stale_or_unknown_quarantines() {
    let mut unrunnable = request();
    unrunnable.workers = vec![worker("web-only", vec![WorkerSurface::Web])];
    let error = plan_distributed_run(unrunnable).expect_err("no eligible workers");
    assert_eq!(error.code(), "test.distributed.worker_unavailable");

    let mut stale = request();
    stale.quarantines.push(DistributedQuarantine {
        scenario_id: "fast-web".to_string(),
        reason: "Known product race".to_string(),
        owner: "checkout-team".to_string(),
        issue: "https://issues.example.test/123".to_string(),
        expires_at_ms: stale.created_at_ms,
    });
    let error = plan_distributed_run(stale).expect_err("expired quarantine");
    assert_eq!(error.code(), "test.distributed.quarantine_expired");

    let mut unknown = request();
    unknown.quarantines.push(DistributedQuarantine {
        scenario_id: "missing".to_string(),
        reason: "No such scenario".to_string(),
        owner: "checkout-team".to_string(),
        issue: "https://issues.example.test/456".to_string(),
        expires_at_ms: unknown.created_at_ms + 60_000,
    });
    let error = plan_distributed_run(unknown).expect_err("unknown quarantine target");
    assert_eq!(error.code(), "test.distributed.quarantine_target_missing");

    let mut oversized_id = request();
    oversized_id.scenarios[0].id = "s".repeat(65);
    let error = plan_distributed_run(oversized_id).expect_err("oversized scenario identifier");
    assert_eq!(error.code(), "test.distributed.identifier_invalid");
}

#[test]
fn analysis_reports_flakes_and_history_changes_without_hiding_infrastructure_failures() {
    let now_ms = 1_800_000_000_000;
    let history = vec![DistributedHistoryRun {
        run_id: "baseline".to_string(),
        suite_digest: SUITE_DIGEST.to_string(),
        finished_at_ms: now_ms - 10_000,
        scenarios: vec![
            DistributedHistoryScenario {
                id: "checkout".to_string(),
                outcome: DistributedScenarioOutcome::Passed,
                duration_ms: 1_000,
            },
            DistributedHistoryScenario {
                id: "search".to_string(),
                outcome: DistributedScenarioOutcome::TestFailed,
                duration_ms: 2_000,
            },
            DistributedHistoryScenario {
                id: "worker-health".to_string(),
                outcome: DistributedScenarioOutcome::Passed,
                duration_ms: 500,
            },
        ],
    }];
    let analysis = analyze_distributed_run(DistributedAnalysisRequest {
        plan_id: "plan-current".to_string(),
        plan_digest: SUITE_DIGEST.to_string(),
        run_id: "current".to_string(),
        suite: "checkout".to_string(),
        suite_digest: SUITE_DIGEST.to_string(),
        started_at_ms: now_ms,
        finished_at_ms: now_ms + 5_000,
        history_window: 20,
        scenarios: vec![
            DistributedScenarioObservation {
                id: "checkout".to_string(),
                outcome: DistributedScenarioOutcome::TestFailed,
                duration_ms: 1_500,
                failure_code: Some("test.assert.visible".to_string()),
            },
            DistributedScenarioObservation {
                id: "search".to_string(),
                outcome: DistributedScenarioOutcome::Passed,
                duration_ms: 1_800,
                failure_code: None,
            },
            DistributedScenarioObservation {
                id: "worker-health".to_string(),
                outcome: DistributedScenarioOutcome::InfrastructureFailed,
                duration_ms: 100,
                failure_code: Some("test.driver.web.unavailable".to_string()),
            },
        ],
        quarantines: vec![
            DistributedQuarantine {
                scenario_id: "checkout".to_string(),
                reason: "Known UI race".to_string(),
                owner: "checkout-team".to_string(),
                issue: "https://issues.example.test/123".to_string(),
                expires_at_ms: now_ms + 60_000,
            },
            DistributedQuarantine {
                scenario_id: "worker-health".to_string(),
                reason: "Must not hide infrastructure".to_string(),
                owner: "platform-team".to_string(),
                issue: "https://issues.example.test/789".to_string(),
                expires_at_ms: now_ms + 60_000,
            },
        ],
        history,
        shard_issues: Vec::new(),
    })
    .expect("distributed analysis");

    assert_eq!(analysis.status, DistributedRunStatus::InfrastructureFailed);
    let checkout = analysis
        .scenarios
        .iter()
        .find(|scenario| scenario.id == "checkout")
        .expect("checkout analysis");
    assert_eq!(
        checkout.disposition,
        DistributedDisposition::QuarantinedFailure
    );
    assert!(checkout.flake.flaky);
    assert_eq!(checkout.change, HistoricalChange::Regression);

    let search = analysis
        .scenarios
        .iter()
        .find(|scenario| scenario.id == "search")
        .expect("search analysis");
    assert_eq!(search.change, HistoricalChange::Fixed);

    let infrastructure = analysis
        .scenarios
        .iter()
        .find(|scenario| scenario.id == "worker-health")
        .expect("infrastructure analysis");
    assert_eq!(infrastructure.disposition, DistributedDisposition::Required);
    assert_eq!(
        infrastructure.change,
        HistoricalChange::InfrastructureChange
    );
    assert_eq!(analysis.counts.quarantined_failed, 1);
    assert_eq!(analysis.counts.infrastructure_failed, 1);
}

#[test]
fn analysis_compares_the_latest_suite_revision_and_freezes_quarantine_at_start() {
    let now_ms = 1_800_000_000_000;
    let previous_digest = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let analysis = analyze_distributed_run(DistributedAnalysisRequest {
        plan_id: "plan-revision".to_string(),
        plan_digest: SUITE_DIGEST.to_string(),
        run_id: "revision".to_string(),
        suite: "checkout".to_string(),
        suite_digest: SUITE_DIGEST.to_string(),
        started_at_ms: now_ms,
        finished_at_ms: now_ms + 5_000,
        history_window: 20,
        scenarios: vec![DistributedScenarioObservation {
            id: "checkout".to_string(),
            outcome: DistributedScenarioOutcome::TestFailed,
            duration_ms: 1_500,
            failure_code: Some("test.assert.visible".to_string()),
        }],
        quarantines: vec![DistributedQuarantine {
            scenario_id: "checkout".to_string(),
            reason: "Admitted before expiry".to_string(),
            owner: "checkout-team".to_string(),
            issue: "https://issues.example.test/123".to_string(),
            expires_at_ms: now_ms + 1,
        }],
        history: vec![DistributedHistoryRun {
            run_id: "previous-revision".to_string(),
            suite_digest: previous_digest.to_string(),
            finished_at_ms: now_ms - 1_000,
            scenarios: vec![
                DistributedHistoryScenario {
                    id: "checkout".to_string(),
                    outcome: DistributedScenarioOutcome::Passed,
                    duration_ms: 1_000,
                },
                DistributedHistoryScenario {
                    id: "removed".to_string(),
                    outcome: DistributedScenarioOutcome::Passed,
                    duration_ms: 500,
                },
            ],
        }],
        shard_issues: Vec::new(),
    })
    .expect("cross-revision analysis");

    assert_eq!(analysis.status, DistributedRunStatus::Passed);
    assert_eq!(
        analysis.baseline_run_id.as_deref(),
        Some("previous-revision")
    );
    assert_eq!(analysis.removed_scenarios, vec!["removed"]);
    assert_eq!(analysis.scenarios[0].change, HistoricalChange::Regression);
    assert_eq!(
        analysis.scenarios[0].disposition,
        DistributedDisposition::QuarantinedFailure
    );
    assert_eq!(analysis.scenarios[0].flake.observations, 1);
}
