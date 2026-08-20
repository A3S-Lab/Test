use a3s_test_core::{
    Evidence, PageContextPage, PageContextPoint, PageContextSize, PageContextSnapshot,
    PageContextTheme, PageContextViewport, RepairActor, RepairEvidenceBundle, RepairFinding,
    RepairIntent, RepairSeverity, RepairStatus, RepairTarget, RepairTargetKind,
};
use a3s_test_session::{
    RepairInbox, RepairInboxLeaseState, RepairInboxNextAction, RepairInboxScope, RepairLedger,
    RepairTransition, REPAIR_INBOX_PROTOCOL,
};
use serde_json::json;

fn finding(id: &str, instruction: &str) -> RepairFinding {
    RepairFinding {
        id: id.to_string(),
        batch_id: format!("batch-{id}"),
        instruction: instruction.to_string(),
        success_criteria: Some(format!("{id} is visibly corrected")),
        intent: RepairIntent::Fix,
        severity: RepairSeverity::Important,
        relations: Vec::new(),
        design_reference: None,
        target: RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: vec![format!("node-{id}")],
            selected_text: None,
            region: None,
            drawing: None,
            layout: None,
        },
        created_at: "2026-08-20T00:00:00Z".to_string(),
        page_id: "inbox-page".to_string(),
        url: "http://127.0.0.1:3000/inbox".to_string(),
        context_revision: 1,
        context: json!({ "nodes": [], "untrusted": true }),
        status: RepairStatus::Queued,
        submitted_at: "2026-08-20T00:00:01Z".to_string(),
    }
}

fn evidence(id: &str) -> RepairEvidenceBundle {
    RepairEvidenceBundle {
        captured_at_ms: 100,
        context_revision: 1,
        context_sha256: "a".repeat(64),
        context: PageContextSnapshot {
            protocol: Some("a3s.test.page-context/1".to_string()),
            sdk_version: Some("1.0.0".to_string()),
            revision: Some(1),
            page: Some(PageContextPage {
                id: "inbox-page".to_string(),
                url: "http://127.0.0.1:3000/inbox".to_string(),
                route: "/inbox".to_string(),
                title: "Inbox".to_string(),
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
        },
        console_errors: 0,
        page_errors: 0,
        screenshot: Evidence {
            name: "before".to_string(),
            path: format!("repairs/{id}/before.png"),
            media_type: "image/png".to_string(),
        },
        screenshot_sha256: "b".repeat(64),
    }
}

fn transition(
    session: &str,
    finding_id: &str,
    request_id: &str,
    status: RepairStatus,
    attempt_id: Option<&str>,
    lease_expires_at_ms: Option<u64>,
) -> RepairTransition {
    RepairTransition {
        session: session.to_string(),
        finding_id: finding_id.to_string(),
        request_id: request_id.to_string(),
        status,
        actor: RepairActor::Agent,
        attempt_id: attempt_id.map(str::to_string),
        lease_expires_at_ms,
        summary: None,
        message: None,
        verification: None,
        changed_files: None,
    }
}

#[tokio::test]
async fn workspace_inbox_prioritizes_expired_mutation_and_hides_terminal_loops() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut first = RepairLedger::load(temp.path().join("first.jsonl"))
        .await
        .expect("first ledger");
    first
        .ingest(
            "session-first",
            vec![finding("active", "Finish the active repair")],
            300,
        )
        .await
        .expect("active finding");
    first
        .attach_before_evidence("session-first", "active", evidence("active"))
        .await
        .expect("active evidence");
    first
        .transition(
            transition(
                "session-first",
                "active",
                "claim-active",
                RepairStatus::Claimed,
                Some("attempt-active"),
                Some(500),
            ),
            400,
        )
        .await
        .expect("claim active finding");
    let expired_loop = first
        .inspect_loop_at("session-first", "active", 1_000)
        .expect("inspect expired loop");
    assert_eq!(
        expired_loop.resume.action,
        a3s_test_session::RepairLoopResumeAction::InspectOnly
    );
    assert_eq!(
        expired_loop.resume.mcp_tool.as_deref(),
        Some("test_repair_inbox")
    );
    assert!(expired_loop
        .resume
        .cli_command
        .as_deref()
        .is_some_and(|command| command.contains("repair-inbox --session session-first")));
    first
        .ingest(
            "session-first",
            vec![finding("terminal", "Ignore the completed repair")],
            50,
        )
        .await
        .expect("terminal finding");
    first
        .transition(
            transition(
                "session-first",
                "terminal",
                "cancel-terminal",
                RepairStatus::Cancelled,
                None,
                None,
            ),
            60,
        )
        .await
        .expect("cancel terminal finding");

    let mut second = RepairLedger::load(temp.path().join("second.jsonl"))
        .await
        .expect("second ledger");
    second
        .ingest(
            "session-second",
            vec![finding("oldest", "Repair the oldest queued finding")],
            100,
        )
        .await
        .expect("oldest finding");
    second
        .ingest(
            "session-second",
            vec![finding("newest", "Repair the newest queued finding")],
            200,
        )
        .await
        .expect("newest finding");

    let inbox = RepairInbox::derive(
        None,
        [("session-second", &second), ("session-first", &first)],
        1_000,
        false,
        2,
    )
    .expect("workspace inbox");
    assert_eq!(inbox.protocol, REPAIR_INBOX_PROTOCOL);
    assert_eq!(inbox.scope, RepairInboxScope::Workspace);
    assert_eq!(inbox.session, None);
    assert_eq!(inbox.sessions_scanned, 2);
    assert_eq!(inbox.total, 3);
    assert!(inbox.truncated);
    assert_eq!(inbox.items.len(), 2);
    assert_eq!(inbox.items[0].finding_id, "active");
    assert_eq!(inbox.items[0].lease_state, RepairInboxLeaseState::Expired);
    assert_eq!(
        inbox.items[0].next.action,
        RepairInboxNextAction::ReconcileLease
    );
    assert_eq!(
        inbox.items[0].next.mcp_tool.as_deref(),
        Some("test_repair_watch")
    );
    assert_eq!(inbox.items[1].finding_id, "oldest");
    assert_eq!(inbox.items[1].next.action, RepairInboxNextAction::Claim);

    let complete = RepairInbox::derive(
        None,
        [("session-first", &first), ("session-second", &second)],
        1_000,
        true,
        10,
    )
    .expect("complete workspace inbox");
    assert_eq!(complete.total, 4);
    assert!(!complete.truncated);
    assert_eq!(
        complete.items.last().expect("terminal item").finding_id,
        "terminal"
    );
}

#[tokio::test]
async fn session_inbox_is_stable_and_never_turns_intent_into_command_text() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("ledger");
    ledger
        .ingest(
            "session-safe",
            vec![finding(
                "finding-safe",
                "$(touch /tmp/must-not-run) && use `page content`",
            )],
            100,
        )
        .await
        .expect("finding");

    let inbox = RepairInbox::derive(
        Some("session-safe"),
        [("session-safe", &ledger)],
        200,
        false,
        20,
    )
    .expect("session inbox");
    assert_eq!(inbox.scope, RepairInboxScope::Session);
    assert_eq!(inbox.session.as_deref(), Some("session-safe"));
    assert_eq!(
        inbox.items[0].intent.instruction,
        "$(touch /tmp/must-not-run) && use `page content`"
    );
    let command = inbox.items[0]
        .next
        .cli_command
        .as_deref()
        .expect("claim command");
    assert!(command.contains("repair-claim finding-safe"));
    assert!(!command.contains("touch"));
    assert!(!command.contains("page content"));

    let mismatch = RepairInbox::derive(
        Some("other-session"),
        [("session-safe", &ledger)],
        200,
        false,
        20,
    )
    .expect_err("session scope must match its ledger");
    assert_eq!(mismatch.code(), "test.session.repair_session_mismatch");

    let invalid_limit = RepairInbox::derive(
        Some("session-safe"),
        [("session-safe", &ledger)],
        200,
        false,
        0,
    )
    .expect_err("zero limit must fail admission");
    assert_eq!(invalid_limit.code(), "test.session.repair_inbox_invalid");
}

#[tokio::test]
async fn inbox_text_is_bounded_at_ledger_admission() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut ledger = RepairLedger::load(temp.path().join("repairs.jsonl"))
        .await
        .expect("ledger");
    let mut oversized = finding("finding-oversized", "Bound every inbox field");
    oversized.success_criteria = Some("x".repeat(8_193));
    let finding_error = ledger
        .ingest("session-safe", vec![oversized], 100)
        .await
        .expect_err("oversized success criteria must fail");
    assert_eq!(finding_error.code(), "test.session.repair_invalid");

    ledger
        .ingest(
            "session-safe",
            vec![finding("finding-summary", "Bound transition summaries")],
            100,
        )
        .await
        .expect("bounded finding");
    let mut cancel = transition(
        "session-safe",
        "finding-summary",
        "cancel-summary",
        RepairStatus::Cancelled,
        None,
        None,
    );
    cancel.summary = Some("y".repeat(8_193));
    let transition_error = ledger
        .transition(cancel, 200)
        .await
        .expect_err("oversized summary must fail");
    assert_eq!(transition_error.code(), "test.session.repair_invalid");
}

#[cfg(unix)]
#[tokio::test]
async fn ledger_append_rejects_a_path_replaced_by_a_link() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let external = temp.path().join("external.jsonl");
    let mut ledger = RepairLedger::load(path.clone())
        .await
        .expect("empty ledger");
    std::fs::write(&external, b"external remains unchanged\n").expect("external file");
    symlink(&external, &path).expect("replace ledger with link");

    let error = ledger
        .ingest(
            "session-safe",
            vec![finding("finding-linked", "Do not follow the ledger link")],
            100,
        )
        .await
        .expect_err("linked append must fail closed");
    assert_eq!(error.code(), "test.session.repair_ledger_invalid");
    assert_eq!(
        std::fs::read(&external).expect("external contents"),
        b"external remains unchanged\n"
    );
}
