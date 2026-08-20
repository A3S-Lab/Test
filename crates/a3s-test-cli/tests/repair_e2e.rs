mod support;

use std::path::PathBuf;

use a3s_test_core::{RepairLayoutCanvas, RepairLayoutIntent, RepairStatus, RepairTargetKind};
use a3s_test_session::RepairRecord;
use serde_json::{json, Value};
use support::repair_fixture::{
    admitted_browser, assert_process_success, json_output, start_fixture, submit_findings,
    submit_layout_findings_from_overlay, target_node_ids, RepairSession, Transition,
};

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn single_repair_restart_recovery_and_acl_promotion_are_end_to_end() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping repair lifecycle E2E");
        return;
    };
    let (_bundle_workspace, fixture) = start_fixture();
    let mut session = RepairSession::start(&browser, &fixture, "repair-single");
    write_verification_profile(&session);
    let target = target_node_ids(&session, &["repair-target"])
        .into_iter()
        .next()
        .expect("repair target node ID");
    let submitted = submit_findings(
        &session,
        &serde_json::to_string(&[finding(
            "finding-single",
            "Repair the broken action",
            Some("Repaired action"),
            &target,
            0,
        )])
        .expect("single finding JSON"),
    );
    assert_eq!(submitted.len(), 1);

    let watch = session.watch();
    let repair = &watch["repairs"][0];
    let before_revision = repair["before_evidence"]["contextRevision"]
        .as_u64()
        .expect("A3S-owned before revision");
    assert_eq!(repair["status"], "queued");
    assert_eq!(repair["finding"]["id"], "finding-single");
    assert_eq!(repair["finding"]["instruction"], "Repair the broken action");

    transition_attempt_to_verifying(&session, "finding-single", "attempt-single");
    fixture.set_repaired(true);
    assert_process_success(
        "apply the fixture hot repair",
        &session.browser(&["eval", "window.testkitFixture.repair(); true"]),
    );
    assert_process_success(
        "wait for a newer repaired page revision",
        &session.browser(&[
            "wait",
            "--fn",
            &format!(
                "document.querySelector('#sticky')?.textContent==='Repaired action'&&window[Symbol.for('a3s.test.page-context')].snapshot().revision>{before_revision}"
            ),
        ]),
    );

    let verify = verify_automatic(&session, "finding-single", "verify-single", true);
    let verification = &verify["repair"]["verification"];
    assert_eq!(verify["repair"]["status"], "review_ready", "{verify:#}");
    assert_eq!(verification["passed"], true);
    assert!(verification["afterRevision"]
        .as_u64()
        .is_some_and(|revision| revision > before_revision));
    assert_eq!(verification["aclProof"]["passed"], true);
    assert_eq!(verification["verificationSlice"]["scope"], "focused");
    assert_eq!(
        verification["verificationSlice"]["sourceFiles"],
        json!(["src/Fixture.tsx"])
    );
    assert_eq!(
        verification["verificationSlice"]["selectedChecks"],
        json!(["fixture"])
    );
    assert_eq!(verification["checks"][0]["status"], "passed");
    assert!(verification["aclCandidate"]
        .as_str()
        .is_some_and(|candidate| candidate.contains("testid(\"repair-target\")")));
    let acl_path = PathBuf::from(
        session.state()["artifacts_dir"]
            .as_str()
            .expect("repair artifact directory"),
    )
    .join(
        verification["aclProof"]["path"]
            .as_str()
            .expect("ACL proof path"),
    );
    assert!(
        acl_path.is_file(),
        "persisted ACL proof missing: {acl_path:?}"
    );

    submit_human_action(&session, "finding-single", "accept", None);
    assert_process_success("reload the repaired page", &session.browser(&["reload"]));
    assert_process_success(
        "wait for TestKit recovery after reload",
        &session.browser(&[
            "wait",
            "--fn",
            "window[Symbol.for('a3s.test.page-context')]?.probe?.().protocol==='a3s.test.page-context/1'&&window[Symbol.for('a3s.test.page-context')].snapshot().page.ready",
        ]),
    );

    let accepted = session.replay();
    assert_eq!(accepted["batches"][0]["status"], "resolved");
    assert_eq!(accepted["batches"][0]["results"][0]["status"], "resolved");
    let events_after_acceptance = ledger_line_count(&session);
    let replay = session.replay();
    assert!(replay["repairs"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(
        ledger_line_count(&session),
        events_after_acceptance,
        "a new CLI process must replay without duplicating authoritative events"
    );
    let records = session.current_repairs();
    assert_eq!(records[0]["status"], "resolved");
    assert_eq!(records[0]["verification"]["aclProof"]["passed"], true);

    let abort = session.abort();
    assert!(abort["cleanup_error"].is_null());
}

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn layout_overlay_batch_reaches_a3s_test_as_typed_non_mutating_intent() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping layout handoff E2E");
        return;
    };
    let (_bundle_workspace, fixture) = start_fixture();
    let mut session = RepairSession::start(&browser, &fixture, "repair-layout-overlay");

    let page_result = submit_layout_findings_from_overlay(&session);
    assert_eq!(
        page_result["layoutKinds"],
        json!(["placement", "rearrange"])
    );
    assert_eq!(page_result["sourceStyleBefore"], Value::Null);
    assert_eq!(page_result["sourceStyleAfter"], Value::Null);
    assert_eq!(page_result["batchIds"][0], page_result["batchIds"][1]);

    let watch = session.watch();
    let repairs: Vec<RepairRecord> = serde_json::from_value(watch["repairs"].clone())
        .expect("A3S Test repair-watch must return typed layout records");
    assert_eq!(repairs.len(), 2, "{watch:#}");
    assert_eq!(
        repairs[0].finding.batch_id, repairs[1].finding.batch_id,
        "the overlay batch order must survive A3S Test ingestion"
    );
    assert!(repairs.iter().all(|repair| {
        repair.status == RepairStatus::Queued && repair.before_evidence.is_some()
    }));

    let placement = &repairs[0].finding.target;
    assert_eq!(placement.kind, RepairTargetKind::Region);
    assert!(placement.node_ids.is_empty());
    let placement_region = placement.region.as_ref().expect("placement region");
    assert_eq!(
        (
            placement_region.x,
            placement_region.y,
            placement_region.width,
            placement_region.height,
        ),
        (700.0, 320.0, 300.0, 160.0)
    );
    match placement.layout.as_ref().expect("typed placement intent") {
        RepairLayoutIntent::Placement {
            component_type,
            canvas,
            purpose,
        } => {
            assert_eq!(component_type, "Pricing section");
            assert_eq!(*canvas, RepairLayoutCanvas::Wireframe);
            assert_eq!(purpose.as_deref(), Some("Developer tool landing page"));
        }
        other => panic!("expected placement intent, got {other:?}"),
    }

    let rearrange = &repairs[1].finding.target;
    assert_eq!(rearrange.kind, RepairTargetKind::Node);
    assert_eq!(rearrange.node_ids.len(), 1);
    let destination = rearrange.region.as_ref().expect("rearrange destination");
    assert_eq!(
        (
            destination.x,
            destination.y,
            destination.width,
            destination.height,
        ),
        (40.0, 420.0, 560.0, 180.0)
    );
    match rearrange.layout.as_ref().expect("typed rearrange intent") {
        RepairLayoutIntent::Rearrange {
            original_region,
            purpose,
        } => {
            assert_eq!(original_region.width, 560.0);
            assert_eq!(original_region.height, 180.0);
            assert_eq!(purpose.as_deref(), Some("Developer tool landing page"));
        }
        other => panic!("expected rearrange intent, got {other:?}"),
    }

    assert!(session.abort()["cleanup_error"].is_null());
}

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn ordered_batch_continues_after_an_isolated_failure() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping ordered batch E2E");
        return;
    };
    let (_bundle_workspace, fixture) = start_fixture();
    let mut session = RepairSession::start(&browser, &fixture, "repair-batch");
    let target_id = target_node_ids(&session, &["repair-target"])
        .into_iter()
        .next()
        .expect("batch target node ID");
    let findings = [
        finding(
            "finding-batch-1",
            "Repair the first independent target",
            None,
            &target_id,
            0,
        ),
        region_finding(
            "finding-batch-2",
            "Repair the second independent target",
            1100.0,
            600.0,
            1,
        ),
    ];
    submit_findings(
        &session,
        &serde_json::to_string(&findings).expect("batch finding JSON"),
    );
    let watch = session.watch();
    assert_eq!(watch["repairs"][0]["finding"]["id"], "finding-batch-1");
    assert_eq!(watch["repairs"][1]["finding"]["id"], "finding-batch-2");
    assert_eq!(
        watch["batches"][0]["findingIds"],
        json!(["finding-batch-1", "finding-batch-2"])
    );

    let failed = session.transition(Transition {
        command: "repair-fail",
        finding_id: "finding-batch-1",
        request_id: "fail-batch-1",
        attempt_id: None,
        summary: "The first repair failed independently",
        message: Some("The second repair remains actionable"),
        lease_ms: None,
    });
    assert_eq!(failed["repair"]["status"], "failed");
    let claimed = session.transition(Transition {
        command: "repair-claim",
        finding_id: "finding-batch-2",
        request_id: "claim-batch-2",
        attempt_id: Some("attempt-batch-2"),
        summary: "Claim the remaining repair",
        message: None,
        lease_ms: None,
    });
    assert_eq!(claimed["repair"]["status"], "claimed");
    assert_eq!(claimed["repair"]["finding"]["id"], "finding-batch-2");
    let cancelled = session.transition(Transition {
        command: "repair-cancel",
        finding_id: "finding-batch-2",
        request_id: "cancel-batch-2",
        attempt_id: Some("attempt-batch-2"),
        summary: "Bounded batch proof complete",
        message: None,
        lease_ms: None,
    });
    assert_eq!(cancelled["repair"]["status"], "cancelled");
    let replay = session.replay();
    assert_eq!(
        replay["batches"][0]["results"],
        json!([
            {"findingId": "finding-batch-1", "status": "failed"},
            {"findingId": "finding-batch-2", "status": "cancelled"}
        ])
    );
    assert_eq!(replay["batches"][0]["status"], "completed_with_failures");

    assert!(session.abort()["cleanup_error"].is_null());
}

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn clarification_round_trip_and_cancellation_are_end_to_end() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping clarification E2E");
        return;
    };
    let (_bundle_workspace, fixture) = start_fixture();
    let mut session = RepairSession::start(&browser, &fixture, "repair-clarify-cancel");
    let targets = target_node_ids(&session, &["repair-target"]);
    submit_findings(
        &session,
        &serde_json::to_string(&[
            finding(
                "finding-clarify",
                "Repair after human clarification",
                None,
                &targets[0],
                0,
            ),
            region_finding(
                "finding-cancel",
                "Cancel this queued repair",
                1100.0,
                600.0,
                1,
            ),
        ])
        .expect("clarification findings JSON"),
    );
    session.watch();
    session.transition(Transition {
        command: "repair-claim",
        finding_id: "finding-clarify",
        request_id: "claim-clarify",
        attempt_id: Some("attempt-clarify"),
        summary: "Claim clarification repair",
        message: None,
        lease_ms: None,
    });
    let question = session.transition(Transition {
        command: "repair-reply",
        finding_id: "finding-clarify",
        request_id: "question-clarify",
        attempt_id: Some("attempt-clarify"),
        summary: "Human clarification required",
        message: Some("Should the visible label remain unchanged?"),
        lease_ms: None,
    });
    assert_eq!(question["repair"]["status"], "needs_input");
    assert_eq!(
        question["repair"]["attempts"][0]["replies"][0]["actor"],
        "agent"
    );
    assert_eq!(
        question["repair"]["attempts"][0]["replies"][0]["message"],
        "Should the visible label remain unchanged?"
    );
    submit_human_action(
        &session,
        "finding-clarify",
        "reply",
        Some("Keep the visible label."),
    );
    let replied = session.replay();
    let clarified = replied["repairs"]
        .as_array()
        .and_then(|repairs| {
            repairs
                .iter()
                .find(|repair| repair["finding"]["id"] == "finding-clarify")
        })
        .expect("clarified repair returned to queue");
    assert_eq!(clarified["status"], "queued");
    assert_eq!(
        clarified["attempts"][0]["replies"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        clarified["attempts"][0]["replies"][1]["message"],
        "Keep the visible label."
    );

    let cancelled = session.transition(Transition {
        command: "repair-cancel",
        finding_id: "finding-cancel",
        request_id: "cancel-queued",
        attempt_id: None,
        summary: "Reviewer cancelled queued work",
        message: None,
        lease_ms: None,
    });
    assert_eq!(cancelled["repair"]["status"], "cancelled");
    let records = session.current_repairs();
    assert_eq!(records[0]["status"], "queued");
    assert_eq!(records[1]["status"], "cancelled");

    assert!(session.abort()["cleanup_error"].is_null());
}

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn disconnect_recovers_pre_edit_claim_and_quarantines_possible_edits() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping disconnect E2E");
        return;
    };
    let (_first_bundle, first_fixture) = start_fixture();
    let mut pre_edit = RepairSession::start(&browser, &first_fixture, "disconnect-pre-edit");
    let target = target_node_ids(&pre_edit, &["repair-target"])[0].clone();
    submit_findings(
        &pre_edit,
        &serde_json::to_string(&[finding(
            "finding-pre-edit",
            "Disconnect before editing",
            None,
            &target,
            0,
        )])
        .expect("pre-edit finding JSON"),
    );
    pre_edit.watch();
    pre_edit.transition(Transition {
        command: "repair-claim",
        finding_id: "finding-pre-edit",
        request_id: "claim-pre-edit",
        attempt_id: Some("attempt-pre-edit"),
        summary: "Claim before disconnect",
        message: None,
        lease_ms: None,
    });
    assert!(pre_edit.abort()["cleanup_error"].is_null());
    let pre_edit_records = pre_edit.current_repairs();
    assert_eq!(pre_edit_records[0]["status"], "queued");
    assert_eq!(
        pre_edit_records[0]["summary"],
        "Session closed before workspace editing began"
    );

    let (_editing_bundle, editing_fixture) = start_fixture();
    let mut editing = RepairSession::start(&browser, &editing_fixture, "disconnect-editing");
    let target = target_node_ids(&editing, &["repair-target"])[0].clone();
    submit_findings(
        &editing,
        &serde_json::to_string(&[finding(
            "finding-editing",
            "Disconnect after editing begins",
            None,
            &target,
            0,
        )])
        .expect("editing finding JSON"),
    );
    editing.watch();
    editing.transition(Transition {
        command: "repair-claim",
        finding_id: "finding-editing",
        request_id: "claim-editing",
        attempt_id: Some("attempt-editing"),
        summary: "Claim editing repair",
        message: None,
        lease_ms: None,
    });
    editing.transition(Transition {
        command: "repair-progress",
        finding_id: "finding-editing",
        request_id: "progress-editing",
        attempt_id: Some("attempt-editing"),
        summary: "Editing may have begun",
        message: None,
        lease_ms: None,
    });
    assert!(editing.abort()["cleanup_error"].is_null());
    let editing_records = editing.current_repairs();
    assert_eq!(editing_records[0]["status"], "needs_input");
    assert_eq!(
        editing_records[0]["summary"],
        "Session closed after workspace editing may have occurred"
    );
    assert_eq!(
        editing_records[0]["message"],
        "Review the possibly mutated workspace before assigning another attempt"
    );
}

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn hot_reload_expires_context_refs_and_fresh_observation_recovers() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping stale ref E2E");
        return;
    };
    let (_bundle_workspace, fixture) = start_fixture();
    let mut session = RepairSession::start(&browser, &fixture, "repair-stale-ref");
    let observed = inspect_context(&session, "inspect before hot reload");
    let observation_id = observed["observation_id"].as_u64().expect("observation ID");
    let context_ref = observed["output"]["page_context"]["snapshot"]["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["testId"] == "repair-target"))
        .and_then(|node| node["ref"].as_str())
        .expect("repair target context ref")
        .to_string();

    assert_process_success(
        "mutate page context after observation",
        &session.browser(&[
            "eval",
            "window.testkitFixture.virtualize(); window.testkitFixture.route(); true",
        ]),
    );
    assert_process_success(
        "wait for hot-reloaded context",
        &session.browser(&[
            "wait",
            "--fn",
            "document.querySelector('#virtual-row')?.textContent==='Virtual row 50'&&location.pathname==='/routed'",
        ]),
    );
    let stale = session.agent(&[
        "click",
        &context_ref,
        "--session",
        "repair-stale-ref",
        "--observation",
        &observation_id.to_string(),
        "--json",
    ]);
    assert_eq!(stale.status.code(), Some(1), "{stale:?}");
    let stale: Value = serde_json::from_slice(&stale.stdout).expect("stale ref error JSON");
    assert_eq!(stale["error"]["code"], "test.driver.web.page_context_stale");

    let fresh = inspect_context(&session, "inspect after hot reload");
    let fresh_observation = fresh["observation_id"]
        .as_u64()
        .expect("fresh observation ID");
    let fresh_ref = fresh["output"]["page_context"]["snapshot"]["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["testId"] == "repair-target"))
        .and_then(|node| node["ref"].as_str())
        .expect("fresh repair target context ref");
    let clicked = session.agent(&[
        "click",
        fresh_ref,
        "--session",
        "repair-stale-ref",
        "--observation",
        &fresh_observation.to_string(),
        "--json",
    ]);
    assert_process_success("click with fresh context ref", &clicked);

    assert!(session.abort()["cleanup_error"].is_null());
}

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn failed_verification_never_auto_resolves_and_can_be_reopened() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping verification failure E2E");
        return;
    };
    let (_bundle_workspace, fixture) = start_fixture();
    let mut session = RepairSession::start(&browser, &fixture, "repair-verify-failure");
    let target = target_node_ids(&session, &["repair-target"])[0].clone();
    submit_findings(
        &session,
        &serde_json::to_string(&[finding(
            "finding-verify-failure",
            "Attempt a repair that fails browser verification",
            Some("Repaired action"),
            &target,
            0,
        )])
        .expect("verification failure finding JSON"),
    );
    let watch = session.watch();
    let before_revision = watch["repairs"][0]["before_evidence"]["contextRevision"]
        .as_u64()
        .expect("before revision");
    transition_attempt_to_verifying(&session, "finding-verify-failure", "attempt-verify-failure");
    assert_process_success(
        "advance the page revision without satisfying the criterion",
        &session.browser(&["eval", "window.testkitFixture.virtualize(); true"]),
    );
    assert_process_success(
        "wait for newer unrepaired revision",
        &session.browser(&[
            "wait",
            "--fn",
            &format!(
                "document.querySelector('#virtual-row')?.textContent==='Virtual row 50'&&window[Symbol.for('a3s.test.page-context')].snapshot().revision>{before_revision}"
            ),
        ]),
    );
    let failed = verify(
        &session,
        "finding-verify-failure",
        "verify-failure",
        false,
        r#"[{"command":"npm test","status":"failed","summary":"Focused TestKit check exposed the defect"}]"#,
    );
    assert_eq!(failed["repair"]["status"], "verification_failed");
    assert_eq!(failed["repair"]["verification"]["passed"], false);
    assert!(failed["repair"]["verification"]["aclProof"].is_null());
    let records = session.current_repairs();
    assert_eq!(records[0]["status"], "verification_failed");
    assert_ne!(records[0]["status"], "resolved");

    submit_human_action(&session, "finding-verify-failure", "reopen", None);
    let reopened = session.replay();
    assert_eq!(reopened["repairs"][0]["status"], "queued");
    assert_eq!(
        reopened["repairs"][0]["summary"],
        "Human retried the failed verification"
    );

    assert!(session.abort()["cleanup_error"].is_null());
}

fn finding(
    id: &str,
    instruction: &str,
    success_criteria: Option<&str>,
    node_id: &str,
    created_offset: u32,
) -> Value {
    json!({
        "id": id,
        "instruction": instruction,
        "successCriteria": success_criteria,
        "intent": "fix",
        "severity": "important",
        "target": { "kind": "node", "nodeIds": [node_id] },
        "createdAt": format!("2026-08-13T00:00:{created_offset:02}Z"),
    })
}

fn region_finding(id: &str, instruction: &str, x: f64, y: f64, created_offset: u32) -> Value {
    json!({
        "id": id,
        "instruction": instruction,
        "intent": "fix",
        "severity": "important",
        "target": {
            "kind": "region",
            "nodeIds": [],
            "region": { "x": x, "y": y, "width": 40.0, "height": 40.0 }
        },
        "createdAt": format!("2026-08-13T00:00:{created_offset:02}Z"),
    })
}

fn transition_attempt_to_verifying(session: &RepairSession, finding_id: &str, attempt_id: &str) {
    for (command, request_id, expected, summary) in [
        ("repair-claim", "claim", "claimed", "Claim repair"),
        (
            "repair-progress",
            "progress",
            "repairing",
            "Editing fixture",
        ),
        (
            "repair-complete",
            "complete",
            "verifying",
            "Fixture edit complete",
        ),
    ] {
        let request_id = format!("{request_id}-{finding_id}");
        let transition = session.transition(Transition {
            command,
            finding_id,
            request_id: &request_id,
            attempt_id: Some(attempt_id),
            summary,
            message: None,
            lease_ms: None,
        });
        assert_eq!(transition["repair"]["status"], expected);
        assert_eq!(transition["repair"]["attempt_id"], attempt_id);
    }
}

fn verify(
    session: &RepairSession,
    finding_id: &str,
    request_id: &str,
    success_criteria_passed: bool,
    checks_json: &str,
) -> Value {
    json_output(
        "verify repair",
        &session.agent(&[
            "repair-verify",
            finding_id,
            "--session",
            session.state()["session"]
                .as_str()
                .expect("repair session ID"),
            "--request-id",
            request_id,
            "--success-criteria-passed",
            if success_criteria_passed {
                "true"
            } else {
                "false"
            },
            "--changed-file",
            "src/Fixture.tsx",
            "--checks-json",
            checks_json,
            "--summary",
            "A3S Test completed browser-owned repair verification",
            "--json",
        ]),
    )
}

fn verify_automatic(
    session: &RepairSession,
    finding_id: &str,
    request_id: &str,
    success_criteria_passed: bool,
) -> Value {
    json_output(
        "verify repair with the configured deterministic slice",
        &session.agent(&[
            "repair-verify",
            finding_id,
            "--session",
            session.state()["session"]
                .as_str()
                .expect("repair session ID"),
            "--request-id",
            request_id,
            "--success-criteria-passed",
            if success_criteria_passed {
                "true"
            } else {
                "false"
            },
            "--changed-file",
            "src/Fixture.tsx",
            "--summary",
            "A3S Test ran the smallest configured verification slice",
            "--json",
        ]),
    )
}

fn write_verification_profile(session: &RepairSession) {
    let profile = r#"project "repair-fixture" {
  version = 1
  root = ".."

  dev_server {
    executable = "rustc"
    args = ["--version"]
    working_directory = "."
    url = "http://127.0.0.1:5173/"
  }

  browser {
    driver = "standalone"
    session = "dev"
    headed = true
  }

  verification {
    check "fixture" {
      tier = "focused"
      executable = "rustc"
      args = ["--version"]
      working_directory = "."
      file_prefixes = ["src"]
    }

    check "workspace" {
      tier = "regression"
      executable = "rustc"
      args = ["--version"]
      working_directory = "."
      file_prefixes = []
    }
  }

  testkit {
    required = true
  }
}
"#;
    std::fs::write(session.workspace().join(".a3s-test/project.acl"), profile)
        .expect("verification profile");
}

fn submit_human_action(
    session: &RepairSession,
    finding_id: &str,
    action: &str,
    message: Option<&str>,
) {
    let payload = json!({
        "findingId": finding_id,
        "action": action,
        "message": message,
    });
    let script = format!(
        "Boolean(window[Symbol.for('a3s.test.page-context')].submitRepairAction({payload}))"
    );
    let output = session.browser(&["eval", &script]);
    assert_process_success("submit human repair action", &output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("true"));
}

fn ledger_line_count(session: &RepairSession) -> usize {
    std::fs::read_to_string(session.ledger_path())
        .expect("repair ledger")
        .lines()
        .count()
}

fn inspect_context(session: &RepairSession, context: &str) -> Value {
    json_output(
        context,
        &session.agent(&[
            "inspect",
            "--session",
            session.state()["session"]
                .as_str()
                .expect("repair session ID"),
            "--detail",
            "forensic",
            "--limit",
            "500",
            "--json",
        ]),
    )
}
