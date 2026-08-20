use super::*;

#[tokio::test]
async fn persists_the_workspace_owner_across_sessions_until_a_terminal_transition() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = RepairWorkspace::new(temp.path());
    let mut first = RepairLedger::load(temp.path().join("first.jsonl"))
        .await
        .expect("first ledger");
    let mut second = RepairLedger::load(temp.path().join("second.jsonl"))
        .await
        .expect("second ledger");
    first
        .ingest("first-session", vec![finding("finding-1")], 1)
        .await
        .expect("first ingest");
    first
        .attach_before_evidence("first-session", "finding-1", evidence(3))
        .await
        .expect("first evidence");
    second
        .ingest("second-session", vec![finding("finding-2")], 1)
        .await
        .expect("second ingest");
    second
        .attach_before_evidence("second-session", "finding-2", evidence(3))
        .await
        .expect("second evidence");

    let mut first_claim = transition(RepairStatus::Claimed, "first-claim");
    first_claim.session = "first-session".to_string();
    first_claim.lease_expires_at_ms = Some(1_000);
    first
        .transition_in_workspace(
            first_claim,
            1,
            &mut workspace.acquire().await.expect("first lock"),
        )
        .await
        .expect("first claim");
    assert!(temp
        .path()
        .join(".a3s-test/repair-workspace.json")
        .is_file());

    let mut second_claim = transition(RepairStatus::Claimed, "second-claim");
    second_claim.session = "second-session".to_string();
    second_claim.finding_id = "finding-2".to_string();
    second_claim.attempt_id = Some("attempt-2".to_string());
    second_claim.lease_expires_at_ms = Some(1_000);
    let error = second
        .transition_in_workspace(
            second_claim.clone(),
            2,
            &mut workspace.acquire().await.expect("second lock"),
        )
        .await
        .expect_err("second session must be blocked");
    assert_eq!(error.code(), "test.session.repair_workspace_busy");

    let mut progress = transition(RepairStatus::Repairing, "first-progress");
    progress.session = "first-session".to_string();
    progress.lease_expires_at_ms = None;
    first
        .transition_in_workspace(
            progress,
            3,
            &mut workspace.acquire().await.expect("progress lock"),
        )
        .await
        .expect("first progress");
    let mut needs_input = transition(RepairStatus::NeedsInput, "first-needs-input");
    needs_input.session = "first-session".to_string();
    needs_input.actor = RepairActor::A3sTest;
    needs_input.lease_expires_at_ms = None;
    first
        .transition_in_workspace(
            needs_input,
            4,
            &mut workspace.acquire().await.expect("terminal lock"),
        )
        .await
        .expect("release first owner");
    assert!(!temp.path().join(".a3s-test/repair-workspace.json").exists());

    let claimed = second
        .transition_in_workspace(
            second_claim,
            5,
            &mut workspace.acquire().await.expect("second retry lock"),
        )
        .await
        .expect("second session claim after release")
        .0;
    assert_eq!(claimed.status, RepairStatus::Claimed);
}

#[tokio::test]
async fn non_owner_cannot_complete_an_active_workspace_attempt() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = RepairWorkspace::new(temp.path());
    let mut first = RepairLedger::load(temp.path().join("first.jsonl"))
        .await
        .expect("first ledger");
    first
        .ingest("first-session", vec![finding("finding-1")], 1)
        .await
        .expect("first ingest");
    first
        .attach_before_evidence("first-session", "finding-1", evidence(3))
        .await
        .expect("first evidence");
    let mut first_claim = transition(RepairStatus::Claimed, "first-claim");
    first_claim.session = "first-session".to_string();
    first_claim.lease_expires_at_ms = Some(1_000);
    first
        .transition_in_workspace(
            first_claim,
            1,
            &mut workspace.acquire().await.expect("first lock"),
        )
        .await
        .expect("first claim");

    let mut spoofed = transition(RepairStatus::NeedsInput, "spoofed-terminal");
    spoofed.session = "second-session".to_string();
    spoofed.actor = RepairActor::A3sTest;
    spoofed.lease_expires_at_ms = None;
    let error = first
        .transition_in_workspace(
            spoofed,
            2,
            &mut workspace.acquire().await.expect("spoofed lock"),
        )
        .await
        .expect_err("non-owner terminal transition must fail");
    assert_eq!(error.code(), "test.session.repair_state_changed");
    assert_eq!(
        first.get("finding-1").expect("repair").status,
        RepairStatus::Claimed
    );
}

#[tokio::test]
async fn reload_rejects_a_stale_verification_attempt() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let workspace = RepairWorkspace::new(temp.path());
    let mut verifier = RepairLedger::load(path.clone()).await.expect("verifier");
    verifier
        .ingest("repair-session", vec![finding("finding-1")], 1)
        .await
        .expect("ingest");
    attach_all_evidence(&mut verifier).await;
    verifier
        .transition_in_workspace(
            transition(RepairStatus::Claimed, "claim-1"),
            2,
            &mut workspace.acquire().await.expect("claim lock"),
        )
        .await
        .expect("claim");
    let mut repairing = transition(RepairStatus::Repairing, "repairing-1");
    repairing.lease_expires_at_ms = None;
    verifier
        .transition_in_workspace(
            repairing,
            3,
            &mut workspace.acquire().await.expect("repairing lock"),
        )
        .await
        .expect("repairing");
    let mut verifying = transition(RepairStatus::Verifying, "verifying-1");
    verifying.lease_expires_at_ms = None;
    verifier
        .transition_in_workspace(
            verifying,
            4,
            &mut workspace.acquire().await.expect("verifying lock"),
        )
        .await
        .expect("verifying");

    let mut concurrent = RepairLedger::load(path).await.expect("concurrent ledger");
    let mut interrupted = transition(RepairStatus::NeedsInput, "interrupted-1");
    interrupted.actor = RepairActor::A3sTest;
    interrupted.lease_expires_at_ms = None;
    concurrent
        .transition_in_workspace(
            interrupted,
            5,
            &mut workspace.acquire().await.expect("interrupt lock"),
        )
        .await
        .expect("interrupt active attempt");

    verifier.reload().await.expect("reload verifier");
    let error = verifier
        .require_attempt_state("finding-1", RepairStatus::Verifying, "attempt-1")
        .expect_err("stale verification must not commit");
    assert_eq!(error.code(), "test.session.repair_state_changed");
    assert!(error.retryable());
}

#[tokio::test]
async fn workspace_mutations_reload_before_appending_concurrent_events() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("repairs.jsonl");
    let workspace = RepairWorkspace::new(temp.path());
    let mut first = RepairLedger::load(path.clone())
        .await
        .expect("first ledger");
    let mut stale = RepairLedger::load(path.clone())
        .await
        .expect("stale ledger");

    first
        .ingest_in_workspace(
            "repair-session",
            vec![finding("finding-1")],
            1,
            &mut workspace.acquire().await.expect("first ingest lock"),
        )
        .await
        .expect("first ingest");
    stale
        .ingest_in_workspace(
            "repair-session",
            vec![finding("finding-2")],
            2,
            &mut workspace.acquire().await.expect("second ingest lock"),
        )
        .await
        .expect("stale ingest reloads");

    let reloaded = RepairLedger::load(path).await.expect("reloaded ledger");
    assert_eq!(
        reloaded
            .queued(10)
            .iter()
            .map(|record| record.finding.id.as_str())
            .collect::<Vec<_>>(),
        ["finding-1", "finding-2"]
    );
}

#[tokio::test]
async fn only_expired_pre_edit_ownership_can_be_taken_over_directly() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = RepairWorkspace::new(temp.path());
    let mut first = RepairLedger::load(temp.path().join("first.jsonl"))
        .await
        .expect("first ledger");
    let mut second = RepairLedger::load(temp.path().join("second.jsonl"))
        .await
        .expect("second ledger");
    first
        .ingest("first-session", vec![finding("finding-1")], 1)
        .await
        .expect("first ingest");
    first
        .attach_before_evidence("first-session", "finding-1", evidence(3))
        .await
        .expect("first evidence");
    second
        .ingest("second-session", vec![finding("finding-2")], 1)
        .await
        .expect("second ingest");
    second
        .attach_before_evidence("second-session", "finding-2", evidence(3))
        .await
        .expect("second evidence");

    let mut first_claim = transition(RepairStatus::Claimed, "first-claim");
    first_claim.session = "first-session".to_string();
    first_claim.lease_expires_at_ms = Some(100);
    first
        .transition_in_workspace(
            first_claim,
            1,
            &mut workspace.acquire().await.expect("first lock"),
        )
        .await
        .expect("first claim");
    let mut second_claim = transition(RepairStatus::Claimed, "second-claim");
    second_claim.session = "second-session".to_string();
    second_claim.finding_id = "finding-2".to_string();
    second_claim.attempt_id = Some("attempt-2".to_string());
    second_claim.lease_expires_at_ms = Some(200);
    second
        .transition_in_workspace(
            second_claim,
            100,
            &mut workspace.acquire().await.expect("takeover lock"),
        )
        .await
        .expect("expired pre-edit takeover");

    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = RepairWorkspace::new(temp.path());
    let mut first = RepairLedger::load(temp.path().join("first.jsonl"))
        .await
        .expect("first ledger");
    let mut second = RepairLedger::load(temp.path().join("second.jsonl"))
        .await
        .expect("second ledger");
    first
        .ingest("first-session", vec![finding("finding-1")], 1)
        .await
        .expect("first ingest");
    first
        .attach_before_evidence("first-session", "finding-1", evidence(3))
        .await
        .expect("first evidence");
    second
        .ingest("second-session", vec![finding("finding-2")], 1)
        .await
        .expect("second ingest");
    second
        .attach_before_evidence("second-session", "finding-2", evidence(3))
        .await
        .expect("second evidence");
    let mut first_claim = transition(RepairStatus::Claimed, "first-claim");
    first_claim.session = "first-session".to_string();
    first_claim.lease_expires_at_ms = Some(100);
    first
        .transition_in_workspace(
            first_claim,
            1,
            &mut workspace.acquire().await.expect("first lock"),
        )
        .await
        .expect("first claim");
    let mut progress = transition(RepairStatus::Repairing, "first-progress");
    progress.session = "first-session".to_string();
    progress.lease_expires_at_ms = None;
    first
        .transition_in_workspace(
            progress,
            2,
            &mut workspace.acquire().await.expect("progress lock"),
        )
        .await
        .expect("editing started");
    let mut second_claim = transition(RepairStatus::Claimed, "second-claim");
    second_claim.session = "second-session".to_string();
    second_claim.finding_id = "finding-2".to_string();
    second_claim.attempt_id = Some("attempt-2".to_string());
    second_claim.lease_expires_at_ms = Some(200);
    assert_eq!(
        second
            .transition_in_workspace(
                second_claim.clone(),
                100,
                &mut workspace.acquire().await.expect("blocked lock"),
            )
            .await
            .expect_err("possibly mutated workspace must not be stolen")
            .code(),
        "test.session.repair_workspace_busy"
    );
    let recovered = first
        .recover_expired_leases_in_workspace(
            "first-session",
            100,
            &mut workspace.acquire().await.expect("recovery lock"),
        )
        .await
        .expect("recover possibly mutated owner");
    assert_eq!(recovered[0].0.status, RepairStatus::NeedsInput);
    second
        .transition_in_workspace(
            second_claim,
            101,
            &mut workspace.acquire().await.expect("claim after recovery"),
        )
        .await
        .expect("second claim after explicit recovery");
}
