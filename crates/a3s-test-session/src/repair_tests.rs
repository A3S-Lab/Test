use super::*;
use crate::RepairWorkspace;
use a3s_test_core::{
    Evidence, PageContextRect, PageContextSnapshot, RepairDesignReference,
    RepairDesignReferenceImage, RepairDesignReferenceKind, RepairIntent, RepairLayoutCanvas,
    RepairLayoutIntent, RepairRelation, RepairSeverity, RepairTarget, RepairTargetKind,
};
use serde_json::json;

fn finding(id: &str) -> RepairFinding {
    RepairFinding {
        id: id.to_string(),
        batch_id: "batch-1".to_string(),
        instruction: format!("Fix {id}"),
        success_criteria: Some("The issue is visibly gone".to_string()),
        intent: RepairIntent::Fix,
        severity: RepairSeverity::Important,
        relations: Vec::new(),
        design_reference: None,
        target: RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: vec!["n1".to_string()],
            selected_text: None,
            region: None,
            drawing: None,
            layout: None,
        },
        created_at: "2026-08-12T00:00:00Z".to_string(),
        page_id: "checkout".to_string(),
        url: "http://127.0.0.1/checkout".to_string(),
        context_revision: 3,
        context: json!({ "untrusted": true }),
        status: RepairStatus::Queued,
        submitted_at: "2026-08-12T00:00:01Z".to_string(),
    }
}

fn transition(status: RepairStatus, request_id: &str) -> RepairTransition {
    RepairTransition {
        session: "repair-session".to_string(),
        finding_id: "finding-1".to_string(),
        request_id: request_id.to_string(),
        status,
        actor: RepairActor::Agent,
        attempt_id: Some("attempt-1".to_string()),
        lease_expires_at_ms: Some(10_000),
        summary: Some("working".to_string()),
        message: None,
        verification: None,
        changed_files: (status == RepairStatus::Verifying)
            .then(|| vec!["src/checkout.rs".to_string()]),
    }
}

fn evidence(revision: u64) -> RepairEvidenceBundle {
    RepairEvidenceBundle {
        captured_at_ms: 1,
        context_revision: revision,
        context_sha256: "a".repeat(64),
        context: ready_page_context(revision),
        console_errors: 0,
        page_errors: 0,
        screenshot: Evidence {
            name: "before".to_string(),
            path: "repairs/finding-1/submitted/before.png".to_string(),
            media_type: "image/png".to_string(),
        },
        screenshot_sha256: "b".repeat(64),
    }
}

fn ready_page_context(revision: u64) -> PageContextSnapshot {
    use a3s_test_core::{
        PageContextPage, PageContextPoint, PageContextSize, PageContextTheme, PageContextViewport,
    };
    PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("0.1.0".to_string()),
        revision: Some(revision),
        page: Some(PageContextPage {
            id: "repair-page".to_string(),
            url: "http://127.0.0.1/checkout".to_string(),
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
                height: 720.0,
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

async fn attach_all_evidence(ledger: &mut RepairLedger) {
    let records = ledger.queued(50);
    for record in records {
        ledger
            .attach_before_evidence(
                "repair-session",
                &record.finding.id,
                evidence(record.finding.context_revision),
            )
            .await
            .expect("attach before evidence");
    }
}

#[tokio::test]
async fn persists_order_and_idempotent_state_transitions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
    ledger
        .ingest(
            "repair-session",
            vec![finding("finding-1"), finding("finding-2")],
            1,
        )
        .await
        .expect("ingest");
    attach_all_evidence(&mut ledger).await;
    assert_eq!(
        ledger
            .queued(10)
            .iter()
            .map(|record| record.finding.id.as_str())
            .collect::<Vec<_>>(),
        ["finding-1", "finding-2"]
    );
    assert_eq!(
        ledger
            .transition(transition(RepairStatus::Claimed, "request-1"), 2)
            .await
            .expect("claim")
            .0
            .sequence,
        1
    );
    assert_eq!(
        ledger
            .transition(transition(RepairStatus::Claimed, "request-1"), 3)
            .await
            .expect("idempotent")
            .0
            .sequence,
        1
    );
    assert!(ledger
        .transition(transition(RepairStatus::Resolved, "request-2"), 4)
        .await
        .is_err());
    let replayed = RepairLedger::load(path).await.expect("replay");
    assert_eq!(
        replayed
            .queued(10)
            .iter()
            .map(|record| record.finding.id.as_str())
            .collect::<Vec<_>>(),
        ["finding-2"]
    );
}

#[tokio::test]
async fn rejects_request_id_reuse_for_a_different_transition() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    ledger
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    attach_all_evidence(&mut ledger).await;
    let (claimed, event) = ledger
        .transition(transition(RepairStatus::Claimed, "request-1"), 2)
        .await
        .expect("claim");
    let (retried, retried_event) = ledger
        .transition(transition(RepairStatus::Claimed, "request-1"), 99)
        .await
        .expect("same request retry");
    assert_eq!(retried, claimed);
    assert_eq!(retried_event, event);

    let conflict = ledger
        .transition(transition(RepairStatus::Repairing, "request-1"), 3)
        .await
        .expect_err("conflicting idempotency key");
    assert_eq!(conflict.code(), "test.session.repair_idempotency_conflict");
}

#[tokio::test]
async fn rejects_corrupt_or_cross_session_replay_without_mutating_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
    ledger
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    let mismatch = ledger
        .ingest("another-session", vec![finding("finding-2")], 2)
        .await
        .expect_err("session mismatch");
    assert_eq!(mismatch.code(), "test.session.repair_session_mismatch");

    tokio::fs::write(&path, "{not-json}\n")
        .await
        .expect("corrupt ledger");
    let invalid = match RepairLedger::load(path).await {
        Ok(_) => panic!("corrupt replay was accepted"),
        Err(error) => error,
    };
    assert_eq!(invalid.code(), "test.session.repair_ledger_invalid");
}

#[tokio::test]
async fn recovers_pre_edit_and_possibly_mutated_expired_leases_differently() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
    ledger
        .ingest(
            "repair-session",
            vec![finding("finding-1"), finding("finding-2")],
            1,
        )
        .await
        .expect("ingest");
    attach_all_evidence(&mut ledger).await;
    ledger
        .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
        .await
        .expect("first claim");
    let pre_edit_recovery = ledger
        .recover_expired_leases("repair-session", 10_000)
        .await
        .expect("recover pre-edit claim");
    assert_eq!(pre_edit_recovery[0].0.status, RepairStatus::Queued);
    attach_all_evidence(&mut ledger).await;
    let mut second_claim = transition(RepairStatus::Claimed, "claim-2");
    second_claim.finding_id = "finding-2".to_string();
    second_claim.attempt_id = Some("attempt-2".to_string());
    second_claim.lease_expires_at_ms = Some(20_000);
    ledger
        .transition(second_claim, 10_001)
        .await
        .expect("second claim");
    let mut editing = transition(RepairStatus::Repairing, "editing-2");
    editing.finding_id = "finding-2".to_string();
    editing.attempt_id = Some("attempt-2".to_string());
    editing.lease_expires_at_ms = None;
    ledger.transition(editing, 10_002).await.expect("editing");

    let recovered = ledger
        .recover_expired_leases("repair-session", 20_000)
        .await
        .expect("recover leases");
    assert_eq!(
        recovered
            .iter()
            .map(|(record, _)| (record.finding.id.as_str(), record.status))
            .collect::<Vec<_>>(),
        [("finding-2", RepairStatus::NeedsInput)]
    );
    assert_eq!(ledger.queued(10)[0].finding.id, "finding-1");
    assert_eq!(ledger.current_events().len(), 2);

    let replayed = RepairLedger::load(path).await.expect("replay");
    assert_eq!(replayed.queued(10)[0].finding.id, "finding-1");
    assert_eq!(
        replayed.records["finding-2"].status,
        RepairStatus::NeedsInput
    );
}

#[tokio::test]
async fn requires_the_active_attempt_and_recovers_an_expired_claim() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    ledger
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    attach_all_evidence(&mut ledger).await;

    let mut claim = transition(RepairStatus::Claimed, "claim-1");
    claim.lease_expires_at_ms = Some(100);
    ledger.transition(claim, 10).await.expect("claim");

    let mut missing_attempt = transition(RepairStatus::Repairing, "progress-missing");
    missing_attempt.attempt_id = None;
    missing_attempt.lease_expires_at_ms = None;
    let error = ledger
        .transition(missing_attempt, 20)
        .await
        .expect_err("missing attempt must fail");
    assert_eq!(error.code(), "test.session.repair_attempt_invalid");

    let mut wrong_attempt = transition(RepairStatus::Repairing, "progress-wrong");
    wrong_attempt.attempt_id = Some("attempt-wrong".to_string());
    wrong_attempt.lease_expires_at_ms = None;
    let error = ledger
        .transition(wrong_attempt, 30)
        .await
        .expect_err("wrong attempt must fail");
    assert_eq!(error.code(), "test.session.repair_attempt_invalid");

    let mut expired_attempt = transition(RepairStatus::Repairing, "progress-expired");
    expired_attempt.lease_expires_at_ms = None;
    let error = ledger
        .transition(expired_attempt, 100)
        .await
        .expect_err("expired lease must fail");
    assert_eq!(error.code(), "test.session.repair_lease_expired");

    let recovered = ledger
        .recover_expired_leases("repair-session", 100)
        .await
        .expect("recover expired claim");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].0.status, RepairStatus::Queued);
    assert!(recovered[0].0.attempt_id.is_none());
    assert!(recovered[0].0.lease_expires_at_ms.is_none());
}

#[tokio::test]
async fn persists_human_reply_review_and_attempt_history() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
    ledger
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    attach_all_evidence(&mut ledger).await;
    ledger
        .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
        .await
        .expect("claim");
    let mut question = transition(RepairStatus::NeedsInput, "question-1");
    question.message = Some("Which state?".to_string());
    question.lease_expires_at_ms = None;
    ledger.transition(question, 3).await.expect("question");

    let replied = ledger
        .apply_human_action(
            "repair-session",
            RepairHumanAction {
                request_id: "human-reply-1".to_string(),
                finding_id: "finding-1".to_string(),
                action: RepairHumanActionKind::Reply,
                timestamp: "2026-08-13T00:00:00Z".to_string(),
                message: Some("Use the enabled state".to_string()),
            },
            4,
        )
        .await
        .expect("reply");
    assert_eq!(replied[0].0.status, RepairStatus::Queued);
    assert_eq!(replied[0].0.attempts.len(), 1);
    assert_eq!(replied[0].0.attempts[0].replies.len(), 2);
    attach_all_evidence(&mut ledger).await;

    let mut second_claim = transition(RepairStatus::Claimed, "claim-2");
    second_claim.attempt_id = Some("attempt-2".to_string());
    ledger
        .transition(second_claim, 5)
        .await
        .expect("second claim");
    let mut progress = transition(RepairStatus::Repairing, "progress-2");
    progress.attempt_id = Some("attempt-2".to_string());
    progress.lease_expires_at_ms = None;
    ledger.transition(progress, 6).await.expect("progress");
    let mut complete = transition(RepairStatus::Verifying, "complete-2");
    complete.attempt_id = Some("attempt-2".to_string());
    complete.lease_expires_at_ms = None;
    ledger.transition(complete, 7).await.expect("complete");
    let mut verified = transition(RepairStatus::ReviewReady, "verified-2");
    verified.actor = RepairActor::A3sTest;
    verified.attempt_id = Some("attempt-2".to_string());
    verified.lease_expires_at_ms = None;
    ledger.transition(verified, 8).await.expect("verified");
    let accepted = ledger
        .apply_human_action(
            "repair-session",
            RepairHumanAction {
                request_id: "human-accept-1".to_string(),
                finding_id: "finding-1".to_string(),
                action: RepairHumanActionKind::Accept,
                timestamp: "2026-08-13T00:00:01Z".to_string(),
                message: None,
            },
            9,
        )
        .await
        .expect("accept");
    assert_eq!(accepted[0].0.status, RepairStatus::Resolved);
    assert_eq!(accepted[0].0.attempts.len(), 2);

    let replayed = RepairLedger::load(path).await.expect("replay");
    let record = replayed.get("finding-1").expect("record");
    assert_eq!(record.status, RepairStatus::Resolved);
    assert_eq!(record.attempts.len(), 2);
    assert_eq!(record.attempts[0].replies.len(), 2);
    assert_eq!(replayed.batches()[0].status, RepairBatchStatus::Resolved);
}

#[tokio::test]
async fn replays_page_human_actions_idempotently_after_the_page_reload() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let mut ledger = RepairLedger::load(path.clone()).await.expect("load");
    ledger
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    attach_all_evidence(&mut ledger).await;
    ledger
        .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
        .await
        .expect("claim");
    let mut question = transition(RepairStatus::NeedsInput, "question-1");
    question.message = Some("Which state?".to_string());
    question.lease_expires_at_ms = None;
    ledger.transition(question, 3).await.expect("question");
    let action = RepairHumanAction {
        request_id: "human-reply-replayed".to_string(),
        finding_id: "finding-1".to_string(),
        action: RepairHumanActionKind::Reply,
        timestamp: "2026-08-13T00:00:00Z".to_string(),
        message: Some("Use the enabled state".to_string()),
    };
    ledger
        .apply_human_action("repair-session", action.clone(), 4)
        .await
        .expect("first reply");
    let replayed = ledger
        .apply_human_action("repair-session", action.clone(), 99)
        .await
        .expect("same live action is idempotent");
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].0.sequence, 3);

    let mut reloaded = RepairLedger::load(path).await.expect("reload");
    let replayed = reloaded
        .apply_human_action("repair-session", action, 100)
        .await
        .expect("page reload action is idempotent");
    assert_eq!(replayed[0].0.status, RepairStatus::Queued);
    assert_eq!(replayed[0].0.attempts[0].status, RepairStatus::NeedsInput);
    assert_eq!(replayed[0].0.attempts[0].finished_at_ms, Some(3));
}

#[tokio::test]
async fn admits_a_newer_ready_before_evidence_baseline() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    ledger
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    let attached = ledger
        .attach_before_evidence("repair-session", "finding-1", evidence(5))
        .await
        .expect("newer evidence");
    assert_eq!(attached.before_evidence.unwrap().context_revision, 5);

    let mut stale = evidence(2);
    stale.context.page.as_mut().expect("page").ready = true;
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    ledger
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    assert_eq!(
        ledger
            .attach_before_evidence("repair-session", "finding-1", stale)
            .await
            .expect_err("stale evidence")
            .code(),
        "test.session.repair_evidence_invalid"
    );
}

#[tokio::test]
async fn independent_batch_failures_retain_order_and_per_item_results() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    ledger
        .ingest(
            "repair-session",
            vec![finding("finding-1"), finding("finding-2")],
            1,
        )
        .await
        .expect("ingest");
    let mut failed = transition(RepairStatus::Failed, "failed-1");
    failed.actor = RepairActor::A3sTest;
    failed.attempt_id = None;
    failed.lease_expires_at_ms = None;
    ledger.transition(failed, 2).await.expect("fail first");
    assert_eq!(ledger.queued(10)[0].finding.id, "finding-2");
    let batch = &ledger.batches()[0];
    assert_eq!(batch.finding_ids, ["finding-1", "finding-2"]);
    assert_eq!(batch.results[0].status, RepairStatus::Failed);
    assert_eq!(batch.results[1].status, RepairStatus::Queued);
    assert_eq!(batch.status, RepairBatchStatus::InProgress);
}

#[tokio::test]
async fn serializes_workspace_mutations_and_moves_overlaps_to_needs_input() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    let mut first = finding("finding-1");
    first.target.node_ids = vec!["shared-node".to_string()];
    let mut second = finding("finding-2");
    second.target.node_ids = vec!["shared-node".to_string()];
    ledger
        .ingest("repair-session", vec![first, second], 1)
        .await
        .expect("ingest");
    let conflicts = ledger
        .resolve_conflicts("repair-session", 2)
        .await
        .expect("resolve conflicts");
    assert_eq!(conflicts.len(), 2);
    assert!(conflicts
        .iter()
        .all(|(record, _)| record.status == RepairStatus::NeedsInput));

    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    let first = finding("finding-1");
    let mut second = finding("finding-2");
    second.target.node_ids = vec!["n2".to_string()];
    ledger
        .ingest("repair-session", vec![first, second], 1)
        .await
        .expect("ingest");
    attach_all_evidence(&mut ledger).await;
    ledger
        .transition(transition(RepairStatus::Claimed, "claim-1"), 2)
        .await
        .expect("first claim");
    let mut second_claim = transition(RepairStatus::Claimed, "claim-2");
    second_claim.finding_id = "finding-2".to_string();
    second_claim.attempt_id = Some("attempt-2".to_string());
    let error = ledger
        .transition(second_claim, 3)
        .await
        .expect_err("concurrent workspace claim");
    assert_eq!(error.code(), "test.session.repair_workspace_busy");
    assert!(error.retryable());
}

#[tokio::test]
async fn detects_explicit_semantic_conflicts_without_reading_instruction_text() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    let mut first = finding("finding-1");
    first.target.node_ids = vec!["header-title".to_string()];
    first.instruction = "Use the compact presentation".to_string();
    first.relations = vec![RepairRelation::ConflictsWith {
        finding_id: "finding-2".to_string(),
    }];
    let mut second = finding("finding-2");
    second.target.node_ids = vec!["footer-summary".to_string()];
    second.instruction = "Use the expanded presentation".to_string();
    second.batch_id = "batch-2".to_string();

    ledger
        .ingest("repair-session", vec![first, second], 1)
        .await
        .expect("ingest disjoint findings");
    let conflicts = ledger
        .resolve_conflicts("repair-session", 2)
        .await
        .expect("resolve explicit conflicts");
    assert_eq!(conflicts.len(), 2);
    assert!(conflicts
        .iter()
        .all(|(record, _)| record.status == RepairStatus::NeedsInput));

    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("load");
    let mut first = finding("finding-1");
    first.target.node_ids = vec!["header-title".to_string()];
    first.instruction = "Make this black and never white".to_string();
    let mut second = finding("finding-2");
    second.target.node_ids = vec!["footer-summary".to_string()];
    second.instruction = "Make this white and never black".to_string();

    ledger
        .ingest("repair-session", vec![first, second], 1)
        .await
        .expect("ingest undeclared requests");
    assert!(ledger
        .resolve_conflicts("repair-session", 2)
        .await
        .expect("resolve undeclared requests")
        .is_empty());
}

#[tokio::test]
async fn rejects_self_referential_duplicate_and_unbounded_conflict_relations() {
    let invalid_relations = [
        vec![RepairRelation::ConflictsWith {
            finding_id: "finding-1".to_string(),
        }],
        vec![
            RepairRelation::ConflictsWith {
                finding_id: "finding-2".to_string(),
            },
            RepairRelation::ConflictsWith {
                finding_id: "finding-2".to_string(),
            },
        ],
        (0..101)
            .map(|index| RepairRelation::ConflictsWith {
                finding_id: format!("related-{index}"),
            })
            .collect(),
    ];

    for relations in invalid_relations {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        let mut submitted = finding("finding-1");
        submitted.relations = relations;
        let error = ledger
            .ingest("repair-session", vec![submitted], 1)
            .await
            .expect_err("invalid relations");
        assert_eq!(error.code(), "test.session.repair_invalid");
    }
}

#[tokio::test]
async fn rejects_inconsistent_or_unbounded_layout_intents() {
    let invalid_targets = [
        RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: Vec::new(),
            selected_text: None,
            region: Some(PageContextRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            }),
            drawing: None,
            layout: Some(RepairLayoutIntent::Placement {
                component_type: "Hero".to_string(),
                canvas: RepairLayoutCanvas::Page,
                purpose: None,
            }),
        },
        RepairTarget {
            kind: RepairTargetKind::Region,
            node_ids: Vec::new(),
            selected_text: None,
            region: Some(PageContextRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            }),
            drawing: None,
            layout: Some(RepairLayoutIntent::Placement {
                component_type: " ".to_string(),
                canvas: RepairLayoutCanvas::Wireframe,
                purpose: None,
            }),
        },
        RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: Vec::new(),
            selected_text: None,
            region: Some(PageContextRect {
                x: 0.0,
                y: 100.0,
                width: 100.0,
                height: 100.0,
            }),
            drawing: None,
            layout: Some(RepairLayoutIntent::Rearrange {
                original_region: PageContextRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                purpose: None,
            }),
        },
        RepairTarget {
            kind: RepairTargetKind::Region,
            node_ids: Vec::new(),
            selected_text: None,
            region: Some(PageContextRect {
                x: 0.0,
                y: 0.0,
                width: f64::INFINITY,
                height: 100.0,
            }),
            drawing: None,
            layout: Some(RepairLayoutIntent::Placement {
                component_type: "Hero".to_string(),
                canvas: RepairLayoutCanvas::Page,
                purpose: Some("x".repeat(2_049)),
            }),
        },
    ];

    for (index, target) in invalid_targets.into_iter().enumerate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
            .await
            .expect("load");
        let mut submitted = finding(&format!("layout-{index}"));
        submitted.target = target;
        let error = ledger
            .ingest("repair-session", vec![submitted], 1)
            .await
            .expect_err("invalid layout target");
        assert_eq!(error.code(), "test.session.repair_invalid");
    }
}

#[tokio::test]
async fn admits_bounded_design_references_and_rejects_oversized_inline_images() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut valid = finding("finding-design-reference");
    valid.design_reference = Some(RepairDesignReference {
        kind: RepairDesignReferenceKind::Sketch,
        width: 960,
        height: 600,
        image: RepairDesignReferenceImage::Inline {
            media_type: "image/png".to_string(),
            data_url: "data:image/png;base64,AAAA".to_string(),
        },
    });
    let mut ledger = RepairLedger::load(temp.path().join("valid.jsonl"))
        .await
        .expect("load valid ledger");
    ledger
        .ingest("repair-session", vec![valid], 1)
        .await
        .expect("bounded design reference is admitted");

    let mut oversized = finding("finding-oversized-reference");
    oversized.design_reference = Some(RepairDesignReference {
        kind: RepairDesignReferenceKind::Screenshot,
        width: 960,
        height: 600,
        image: RepairDesignReferenceImage::Inline {
            media_type: "image/jpeg".to_string(),
            data_url: format!("data:image/jpeg;base64,{}", "A".repeat(384 * 1_024)),
        },
    });
    let mut ledger = RepairLedger::load(temp.path().join("oversized.jsonl"))
        .await
        .expect("load oversized ledger");
    let error = ledger
        .ingest("repair-session", vec![oversized], 1)
        .await
        .expect_err("oversized inline image is rejected");
    assert_eq!(error.code(), "test.session.repair_invalid");
}

#[path = "repair_workspace_tests.rs"]
mod workspace;
