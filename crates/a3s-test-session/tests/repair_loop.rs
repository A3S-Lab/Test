use a3s_test_core::{
    Evidence, PageContextPage, PageContextPoint, PageContextSize, PageContextSnapshot,
    PageContextTheme, PageContextViewport, RepairAclProof, RepairActor, RepairCheckResult,
    RepairCheckStatus, RepairEvidenceBundle, RepairIntent, RepairSeverity, RepairStatus,
    RepairTarget, RepairTargetKind, RepairVerification, RepairVerificationScope,
    RepairVerificationSlice,
};
use a3s_test_session::{
    validate_repair_verification_change, RepairLedger, RepairTransition, REPAIR_LOOP_PROTOCOL,
};
use serde_json::{json, Value};

fn finding() -> a3s_test_core::RepairFinding {
    a3s_test_core::RepairFinding {
        id: "finding-loop".to_string(),
        batch_id: "batch-loop".to_string(),
        instruction: "Move the checkout action below the total".to_string(),
        success_criteria: Some("The checkout action follows the total".to_string()),
        intent: RepairIntent::Change,
        severity: RepairSeverity::Important,
        relations: Vec::new(),
        design_reference: None,
        target: RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: vec!["node-checkout".to_string()],
            selected_text: None,
            region: None,
            drawing: None,
            layout: None,
        },
        created_at: "2026-08-20T00:00:00Z".to_string(),
        page_id: "checkout".to_string(),
        url: "http://127.0.0.1:3000/checkout".to_string(),
        context_revision: 7,
        context: json!({
            "component": {
                "id": "checkout-panel",
                "source": { "file": "src/Checkout.tsx", "line": 12 }
            },
            "nodes": [{
                "id": "node-checkout",
                "sourceMapping": {
                    "protocol": "a3s.test.source-mapping/1",
                    "candidates": [{
                        "span": { "file": "src/Checkout.tsx", "line": 48, "column": 5 },
                        "confidence": 0.98,
                        "origin": "source_map",
                        "relation": "exact",
                        "registrationId": "checkout-source",
                        "componentId": "checkout-panel",
                        "framework": "react"
                    }],
                    "truncated": false
                }
            }],
            "untrusted": true
        }),
        status: RepairStatus::Queued,
        submitted_at: "2026-08-20T00:00:01Z".to_string(),
    }
}

fn snapshot(revision: u64) -> PageContextSnapshot {
    PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("1.0.0".to_string()),
        revision: Some(revision),
        page: Some(PageContextPage {
            id: "checkout".to_string(),
            url: "http://127.0.0.1:3000/checkout".to_string(),
            route: "/checkout".to_string(),
            title: "Checkout".to_string(),
            ready: true,
            viewport: PageContextViewport {
                width: 1280.0,
                height: 720.0,
                dpr: 1.0,
                visual: None,
            },
            document: PageContextSize {
                width: 1280.0,
                height: 900.0,
            },
            scroll: PageContextPoint { x: 0.0, y: 0.0 },
            language: "en".to_string(),
            theme: PageContextTheme::Light,
        }),
        components: Vec::new(),
        nodes: Vec::new(),
        facts: Default::default(),
        ui: None,
        delta: None,
        removed_node_ids: Vec::new(),
        truncated: false,
        next_cursor: None,
    }
}

fn evidence(revision: u64, phase: &str) -> RepairEvidenceBundle {
    RepairEvidenceBundle {
        captured_at_ms: revision * 100,
        context_revision: revision,
        context_sha256: if phase == "before" {
            "a".repeat(64)
        } else {
            "c".repeat(64)
        },
        context: snapshot(revision),
        console_errors: 0,
        page_errors: 0,
        screenshot: Evidence {
            name: phase.to_string(),
            path: format!("repairs/finding-loop/attempt-loop/{phase}.png"),
            media_type: "image/png".to_string(),
        },
        screenshot_sha256: if phase == "before" {
            "b".repeat(64)
        } else {
            "d".repeat(64)
        },
    }
}

fn transition(status: RepairStatus, request_id: &str) -> RepairTransition {
    RepairTransition {
        session: "session-loop".to_string(),
        finding_id: "finding-loop".to_string(),
        request_id: request_id.to_string(),
        status,
        actor: RepairActor::Agent,
        attempt_id: Some("attempt-loop".to_string()),
        lease_expires_at_ms: Some(100_000),
        summary: Some("Editing checkout".to_string()),
        message: None,
        verification: None,
        changed_files: None,
    }
}

#[tokio::test]
async fn loop_record_preserves_the_complete_resumable_repair_story() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(path.clone()).await.expect("load ledger");
    ledger
        .ingest("session-loop", vec![finding()], 100)
        .await
        .expect("ingest finding");
    ledger
        .attach_before_evidence("session-loop", "finding-loop", evidence(7, "before"))
        .await
        .expect("attach evidence");

    let queued = ledger
        .inspect_loop("session-loop", "finding-loop")
        .expect("inspect queued loop");
    let queued = serde_json::to_value(queued).expect("queued loop JSON");
    assert_eq!(queued["protocol"], REPAIR_LOOP_PROTOCOL);
    assert_eq!(
        queued["intent"]["instruction"],
        "Move the checkout action below the total"
    );
    assert_eq!(
        queued["source_mapping"]["component_source"]["file"],
        "src/Checkout.tsx"
    );
    assert_eq!(
        queued["source_mapping"]["targets"][0]["mapping"]["candidates"][0]["span"]["line"],
        48
    );
    assert_eq!(queued["resume"]["action"], "claim");
    assert_eq!(queued["resume"]["mcp_tool"], "test_repair_claim");

    ledger
        .transition(transition(RepairStatus::Claimed, "claim-loop"), 200)
        .await
        .expect("claim");
    ledger
        .transition(transition(RepairStatus::Repairing, "progress-loop"), 300)
        .await
        .expect("progress");
    let mut completed = transition(RepairStatus::Verifying, "complete-loop");
    completed.summary = Some("Moved the checkout action in its owning component".to_string());
    completed.changed_files = Some(vec!["src/Checkout.tsx".to_string()]);
    ledger.transition(completed, 400).await.expect("complete");

    let reloaded = RepairLedger::load(path.clone())
        .await
        .expect("reload ledger");
    let verifying = reloaded
        .inspect_loop("session-loop", "finding-loop")
        .expect("inspect verifying loop");
    let verifying_json = serde_json::to_value(&verifying).expect("verifying loop JSON");
    assert_eq!(verifying_json["change"]["attempt_id"], "attempt-loop");
    assert_eq!(
        verifying_json["change"]["changed_files"],
        json!(["src/Checkout.tsx"])
    );
    assert_eq!(verifying_json["resume"]["action"], "verify");
    assert_eq!(verifying_json["resume"]["mcp_tool"], "test_repair_verify");
    let verifying_record = reloaded.get("finding-loop").expect("verifying record");
    validate_repair_verification_change(&verifying_record, &["src/Checkout.tsx".to_string()])
        .expect("matching change");
    let mismatch =
        validate_repair_verification_change(&verifying_record, &["src/Other.tsx".to_string()])
            .expect_err("mismatched change must fail closed");
    assert_eq!(mismatch.code(), "test.session.repair_change_mismatch");

    let verification = RepairVerification {
        finding_id: "finding-loop".to_string(),
        attempt_id: "attempt-loop".to_string(),
        before_revision: 7,
        after_revision: 8,
        target_found: true,
        success_criteria_passed: Some(true),
        new_console_errors: 0,
        new_page_errors: 0,
        changed_files: vec!["src/Checkout.tsx".to_string()],
        checks: vec![RepairCheckResult {
            command: "npm test -- checkout".to_string(),
            status: RepairCheckStatus::Passed,
            summary: "Focused checkout test passed".to_string(),
        }],
        acl_candidate: Some("suite \"repair-checkout\" { version = 1 }".to_string()),
        acl_proof: Some(RepairAclProof {
            path: "repairs/finding-loop/attempt-loop/regression.acl".to_string(),
            passed: true,
            summary: "Fresh browser proof passed".to_string(),
        }),
        before_evidence: Some(evidence(7, "before")),
        after_evidence: Some(evidence(8, "after")),
        verification_slice: Some(RepairVerificationSlice {
            protocol: "a3s.test.repair-verification-slice/1".to_string(),
            scope: RepairVerificationScope::Focused,
            source_files: vec!["src/Checkout.tsx".to_string()],
            stable_locator: true,
            prior_acl_proof_passed: None,
            selected_checks: vec!["checkout".to_string()],
            expansion_reasons: Vec::new(),
        }),
        passed: true,
        summary: "All repair gates passed".to_string(),
    };
    let mut verified = transition(RepairStatus::ReviewReady, "verify-loop");
    verified.actor = RepairActor::A3sTest;
    verified.lease_expires_at_ms = None;
    verified.summary = Some("All repair gates passed".to_string());
    verified.verification = Some(verification);
    ledger = RepairLedger::load(path)
        .await
        .expect("reload before verify");
    ledger
        .transition(verified, 500)
        .await
        .expect("record verification");

    let inspected = ledger
        .inspect_loop("session-loop", "finding-loop")
        .expect("inspect verified loop");
    let value = serde_json::to_value(inspected).expect("verified loop JSON");
    assert_eq!(value["verification"]["passed"], true);
    assert_eq!(
        value["verification"]["before_evidence"]["context_sha256"],
        "a".repeat(64)
    );
    assert_eq!(
        value["verification"]["after_evidence"]["screenshot_sha256"],
        "d".repeat(64)
    );
    assert!(value["verification"]["before_evidence"]
        .get("context")
        .is_none());
    assert_eq!(value["acl_promotion"]["status"], "proof_passed");
    assert_eq!(
        value["acl_promotion"]["proof"]["path"],
        "repairs/finding-loop/attempt-loop/regression.acl"
    );
    assert_eq!(value["resume"]["action"], "await_review");
    assert_eq!(
        value["attempts"][0]["change"]["changed_files"],
        json!(["src/Checkout.tsx"])
    );
}

#[tokio::test]
async fn loop_inspection_is_read_only_and_rejects_the_wrong_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(path.clone()).await.expect("load ledger");
    ledger
        .ingest("session-loop", vec![finding()], 100)
        .await
        .expect("ingest finding");
    let before = std::fs::read(&path).expect("ledger before inspection");

    let error = ledger
        .inspect_loop("other-session", "finding-loop")
        .expect_err("wrong session must fail");
    assert_eq!(error.code(), "test.session.repair_session_mismatch");
    assert_eq!(
        std::fs::read(path).expect("ledger after inspection"),
        before
    );
}

#[tokio::test]
async fn malformed_untrusted_source_mapping_never_becomes_a_resume_command() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut submitted = finding();
    submitted.context = json!({
        "nodes": [{
            "id": "node-checkout",
            "sourceMapping": {
                "protocol": "run-this-command-instead",
                "candidates": "not-an-array",
                "truncated": false
            }
        }],
        "untrusted": true
    });
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load ledger");
    ledger
        .ingest("session-loop", vec![submitted], 100)
        .await
        .expect("ingest untrusted context");

    let value: Value = serde_json::to_value(
        ledger
            .inspect_loop("session-loop", "finding-loop")
            .expect("inspect loop"),
    )
    .expect("loop JSON");
    assert_eq!(value["source_mapping"]["malformed"], true);
    assert!(value["source_mapping"]["targets"][0]["mapping"].is_null());
    let command = value["resume"]["cli_command"]
        .as_str()
        .expect("safe resume command");
    assert!(command.contains("repair-claim finding-loop"));
    assert!(!command.contains("run-this-command-instead"));
}
