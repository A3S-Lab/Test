use a3s_test_driver_tui::TuiCapabilities;
use a3s_test_worker::{
    remote_worker_protocol_schema, RemoteInputBundle, RemoteInputFile, RemoteJobSubmission,
    RemoteWorkerDescriptor, RemoteWorkerIdentity, RemoteWorkerLimits, WorkerCapabilityInventory,
    WorkerSurface, WorkerSurfaceCapability, REMOTE_WORKER_PROTOCOL,
};

const IMAGE_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn descriptor() -> RemoteWorkerDescriptor {
    let inventory = WorkerCapabilityInventory::local(
        4,
        vec![WorkerSurfaceCapability::Tui {
            terminal: TuiCapabilities::compiled().expect("compiled TUI capabilities"),
        }],
    )
    .expect("worker inventory");
    RemoteWorkerDescriptor::new(
        RemoteWorkerIdentity {
            instance_id: "runner-west-1".to_string(),
            image_digest: IMAGE_DIGEST.to_string(),
        },
        inventory,
        RemoteWorkerLimits::default(),
    )
    .expect("remote worker descriptor")
}

fn submission(now_ms: u64) -> RemoteJobSubmission {
    let descriptor = descriptor();
    RemoteJobSubmission {
        job_id: "job-001".to_string(),
        dispatch_id: "dispatch-001".to_string(),
        worker_instance: descriptor.identity.instance_id.clone(),
        required_image_digest: descriptor.identity.image_digest.clone(),
        required_inventory_digest: descriptor.inventory_digest.clone(),
        issued_at_ms: now_ms,
        deadline_ms: now_ms + 60_000,
        lease_expires_at_ms: now_ms + 30_000,
        max_parallel_scenarios: 2,
        required_surfaces: vec![WorkerSurface::Tui],
        scenario_ids: vec!["terminal".to_string()],
        input: RemoteInputBundle {
            manifest: "suite.acl".to_string(),
            files: vec![RemoteInputFile::from_bytes(
                "suite.acl",
                br#"suite "remote" {
    version = 1
    scenario "terminal" {
        surface = "tui"
        expect "ready" { text = "ready" }
    }
}
"#,
            )],
        },
    }
}

#[test]
fn remote_protocol_schema_is_strict_and_states_execution_boundaries() {
    let protocol = remote_worker_protocol_schema();
    assert_eq!(protocol.protocol, REMOTE_WORKER_PROTOCOL);
    assert!(protocol.invariants.transport_authentication_required);
    assert!(protocol.invariants.tls_termination_external);
    assert!(protocol.invariants.external_image_identity_required);
    assert!(protocol.invariants.request_cannot_select_executables);
    assert!(protocol.invariants.deadline_and_lease_required);
    assert!(protocol.invariants.idempotent_dispatch_required);
    assert!(protocol.invariants.scenario_selection_digest_bound);
    assert!(!protocol.invariants.transports_artifacts);

    let request_schema = serde_json::to_value(protocol.request_schema).expect("request schema");
    let descriptor_schema =
        serde_json::to_value(protocol.descriptor_schema).expect("descriptor schema");
    assert_eq!(request_schema["additionalProperties"], false);
    assert_eq!(
        request_schema["properties"]["protocol"]["const"],
        REMOTE_WORKER_PROTOCOL
    );
    assert_eq!(descriptor_schema["additionalProperties"], false);
    assert_eq!(
        descriptor_schema["properties"]["limits"]["$ref"],
        "#/$defs/RemoteWorkerLimits"
    );
    assert_eq!(
        request_schema["$defs"]["RemoteJobSubmission"]["properties"]["required_surfaces"]
            ["maxItems"],
        2
    );
}

#[test]
fn valid_submission_is_digest_bound_and_decodes_a_canonical_bundle() {
    let now_ms = 1_800_000_000_000;
    let descriptor = descriptor();
    let admitted = submission(now_ms)
        .admit(now_ms, &descriptor)
        .expect("admitted remote job");

    assert_eq!(admitted.job_id(), "job-001");
    assert_eq!(admitted.dispatch_id(), "dispatch-001");
    assert_eq!(admitted.manifest(), "suite.acl");
    assert_eq!(admitted.files().len(), 1);
    assert_eq!(admitted.files()[0].path(), "suite.acl");
    assert!(admitted.files()[0].bytes().starts_with(b"suite"));
    assert!(admitted.request_digest().starts_with("sha256:"));
    assert_eq!(admitted.required_surfaces(), &[WorkerSurface::Tui]);
    assert_eq!(admitted.scenario_ids(), &["terminal"]);
}

#[test]
fn submission_rejects_identity_capability_and_time_binding_drift() {
    let now_ms = 1_800_000_000_000;
    let descriptor = descriptor();

    let mut unsafe_job_id = submission(now_ms);
    unsafe_job_id.job_id = "..".to_string();
    assert_eq!(
        unsafe_job_id
            .admit(now_ms, &descriptor)
            .expect_err("filesystem-unsafe job ID")
            .code(),
        "test.worker.remote.identifier_invalid"
    );

    let mut wrong_instance = submission(now_ms);
    wrong_instance.worker_instance = "runner-east-9".to_string();
    assert_eq!(
        wrong_instance
            .admit(now_ms, &descriptor)
            .expect_err("wrong instance")
            .code(),
        "test.worker.remote.instance_mismatch"
    );

    let mut wrong_image = submission(now_ms);
    wrong_image.required_image_digest =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    assert_eq!(
        wrong_image
            .admit(now_ms, &descriptor)
            .expect_err("wrong image")
            .code(),
        "test.worker.remote.image_mismatch"
    );

    let mut wrong_inventory = submission(now_ms);
    wrong_inventory.required_inventory_digest =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string();
    assert_eq!(
        wrong_inventory
            .admit(now_ms, &descriptor)
            .expect_err("wrong inventory")
            .code(),
        "test.worker.remote.inventory_mismatch"
    );

    let mut expired = submission(now_ms);
    expired.deadline_ms = now_ms;
    expired.lease_expires_at_ms = now_ms;
    assert_eq!(
        expired
            .admit(now_ms, &descriptor)
            .expect_err("expired job")
            .code(),
        "test.worker.remote.deadline_invalid"
    );

    let mut lease_after_deadline = submission(now_ms);
    lease_after_deadline.lease_expires_at_ms = lease_after_deadline.deadline_ms + 1;
    assert_eq!(
        lease_after_deadline
            .admit(now_ms, &descriptor)
            .expect_err("lease after deadline")
            .code(),
        "test.worker.remote.lease_invalid"
    );
}

#[test]
fn submission_rejects_unavailable_or_noncanonical_surface_requirements() {
    let now_ms = 1_800_000_000_000;
    let descriptor = descriptor();

    let mut unavailable = submission(now_ms);
    unavailable.required_surfaces = vec![WorkerSurface::Web];
    assert_eq!(
        unavailable
            .admit(now_ms, &descriptor)
            .expect_err("unavailable Web surface")
            .code(),
        "test.worker.remote.surface_unavailable"
    );

    let mut duplicate = submission(now_ms);
    duplicate.required_surfaces = vec![WorkerSurface::Tui, WorkerSurface::Tui];
    assert_eq!(
        duplicate
            .admit(now_ms, &descriptor)
            .expect_err("duplicate surface")
            .code(),
        "test.worker.remote.surface_order_invalid"
    );

    let mut too_parallel = submission(now_ms);
    too_parallel.max_parallel_scenarios = 5;
    assert_eq!(
        too_parallel
            .admit(now_ms, &descriptor)
            .expect_err("parallelism over inventory")
            .code(),
        "test.worker.remote.parallelism_unavailable"
    );
}

#[test]
fn submission_rejects_empty_duplicate_or_noncanonical_scenario_selection() {
    let now_ms = 1_800_000_000_000;
    let descriptor = descriptor();

    let mut empty = submission(now_ms);
    empty.scenario_ids.clear();
    assert_eq!(
        empty
            .admit(now_ms, &descriptor)
            .expect_err("empty scenario selection")
            .code(),
        "test.worker.remote.scenario_selection_invalid"
    );

    let mut duplicate = submission(now_ms);
    duplicate.scenario_ids = vec!["terminal".to_string(), "terminal".to_string()];
    assert_eq!(
        duplicate
            .admit(now_ms, &descriptor)
            .expect_err("duplicate scenario selection")
            .code(),
        "test.worker.remote.scenario_selection_invalid"
    );

    let mut unsorted = submission(now_ms);
    unsorted.scenario_ids = vec!["zeta".to_string(), "alpha".to_string()];
    assert_eq!(
        unsorted
            .admit(now_ms, &descriptor)
            .expect_err("non-canonical scenario selection")
            .code(),
        "test.worker.remote.scenario_selection_invalid"
    );

    let mut unsafe_id = submission(now_ms);
    unsafe_id.scenario_ids = vec!["../outside".to_string()];
    assert_eq!(
        unsafe_id
            .admit(now_ms, &descriptor)
            .expect_err("unsafe scenario identifier")
            .code(),
        "test.worker.remote.scenario_selection_invalid"
    );
}

#[test]
fn bundle_rejects_traversal_duplicates_digest_drift_and_size_overclaims() {
    let now_ms = 1_800_000_000_000;
    let descriptor = descriptor();

    let mut traversal = submission(now_ms);
    traversal.input.files[0].path = "../suite.acl".to_string();
    traversal.input.manifest = "../suite.acl".to_string();
    assert_eq!(
        traversal
            .admit(now_ms, &descriptor)
            .expect_err("path traversal")
            .code(),
        "test.worker.remote.input_path_invalid"
    );

    let mut reserved = submission(now_ms);
    reserved.input.files[0].path = "CON.acl".to_string();
    reserved.input.manifest = "CON.acl".to_string();
    assert_eq!(
        reserved
            .admit(now_ms, &descriptor)
            .expect_err("Windows reserved input path")
            .code(),
        "test.worker.remote.input_path_invalid"
    );

    let mut case_collision = submission(now_ms);
    case_collision.input.files.insert(
        0,
        RemoteInputFile::from_bytes("Suite.acl", b"case-collision"),
    );
    assert_eq!(
        case_collision
            .admit(now_ms, &descriptor)
            .expect_err("case-insensitive input collision")
            .code(),
        "test.worker.remote.input_path_collision"
    );

    let mut duplicate = submission(now_ms);
    duplicate.input.files.push(duplicate.input.files[0].clone());
    assert_eq!(
        duplicate
            .admit(now_ms, &descriptor)
            .expect_err("duplicate input")
            .code(),
        "test.worker.remote.input_order_invalid"
    );

    let mut digest_drift = submission(now_ms);
    digest_drift.input.files[0].sha256 =
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string();
    assert_eq!(
        digest_drift
            .admit(now_ms, &descriptor)
            .expect_err("digest drift")
            .code(),
        "test.worker.remote.input_digest_mismatch"
    );

    let mut noncanonical_base64 = submission(now_ms);
    noncanonical_base64.input.files[0] = RemoteInputFile::from_bytes("suite.acl", b"d");
    noncanonical_base64.input.files[0].contents_base64 = "ZE==".to_string();
    assert_eq!(
        noncanonical_base64
            .admit(now_ms, &descriptor)
            .expect_err("noncanonical Base64")
            .code(),
        "test.worker.remote.input_encoding_invalid"
    );

    let mut oversized = submission(now_ms);
    oversized.input.files[0] = RemoteInputFile::from_bytes(
        "suite.acl",
        vec![b'x'; descriptor.limits.max_file_bytes as usize + 1],
    );
    assert_eq!(
        oversized
            .admit(now_ms, &descriptor)
            .expect_err("oversized input")
            .code(),
        "test.worker.remote.input_file_too_large"
    );
}

#[test]
fn wire_types_reject_unknown_fields() {
    let value = serde_json::json!({
        "protocol": REMOTE_WORKER_PROTOCOL,
        "request_id": "request-1",
        "command": { "operation": "inspect" },
        "trusted": true
    });
    serde_json::from_value::<a3s_test_worker::RemoteWorkerRequest>(value)
        .expect_err("unknown request field must fail");
}
