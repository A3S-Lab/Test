use a3s_test_driver_tui::TuiCapabilities;
use a3s_test_worker::{
    remote_artifact_protocol_schema, RemoteArtifactCommand, RemoteArtifactRequest, RemoteJobState,
    RemoteReportQuery, RemoteRetentionPolicy, RemoteWorkerDescriptor, RemoteWorkerIdentity,
    RemoteWorkerLimits, WorkerCapabilityInventory, WorkerSurfaceCapability,
    REMOTE_ARTIFACT_PROTOCOL,
};

const IMAGE_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn descriptor() -> RemoteWorkerDescriptor {
    let inventory = WorkerCapabilityInventory::local(
        1,
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
    .expect("worker descriptor")
}

#[test]
fn artifact_protocol_is_separate_authenticated_and_bounded() {
    let protocol = remote_artifact_protocol_schema();
    assert_eq!(protocol.protocol, REMOTE_ARTIFACT_PROTOCOL);
    assert!(protocol.invariants.transport_authentication_required);
    assert!(protocol.invariants.deployment_owned_retention);
    assert!(protocol.invariants.digest_bound_reads);
    assert!(protocol.invariants.bounded_pagination);
    assert!(protocol.invariants.bounded_chunks);
    assert!(protocol.invariants.no_arbitrary_paths);
    assert!(protocol.invariants.transports_artifacts);

    let request_schema = serde_json::to_value(protocol.request_schema).expect("request schema");
    let descriptor_schema =
        serde_json::to_value(protocol.descriptor_schema).expect("descriptor schema");
    assert_eq!(request_schema["additionalProperties"], false);
    assert_eq!(
        request_schema["properties"]["protocol"]["const"],
        REMOTE_ARTIFACT_PROTOCOL
    );
    assert_eq!(descriptor_schema["additionalProperties"], false);
    assert_eq!(
        descriptor_schema["properties"]["retention"]["$ref"],
        "#/$defs/RemoteRetentionPolicy"
    );
}

#[test]
fn retention_policy_requires_payload_and_index_bounds_to_be_ordered() {
    let policy = RemoteRetentionPolicy::default();
    policy.validate().expect("default retention policy");

    let invalid_count = RemoteRetentionPolicy {
        max_indexed_jobs: policy.max_retained_jobs - 1,
        ..policy.clone()
    };
    assert_eq!(
        invalid_count
            .validate()
            .expect_err("index count below payload count")
            .code(),
        "test.worker.artifact.retention_invalid"
    );

    let invalid_age = RemoteRetentionPolicy {
        max_index_age_ms: policy.max_retention_age_ms - 1,
        ..policy
    };
    assert_eq!(
        invalid_age
            .validate()
            .expect_err("index age below payload age")
            .code(),
        "test.worker.artifact.retention_invalid"
    );
}

#[test]
fn artifact_requests_are_strict_and_report_queries_are_explicit() {
    let request = RemoteArtifactRequest {
        protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
        request_id: "reports-1".to_string(),
        command: RemoteArtifactCommand::ListReports {
            query: RemoteReportQuery {
                states: vec![RemoteJobState::Passed],
                suite: Some("checkout".to_string()),
                run_id: None,
                finished_after_ms: Some(1_800_000_000_000),
                finished_before_ms: None,
                limit: 25,
                cursor: None,
            },
        },
    };
    request.validate().expect("valid artifact request");

    let mut value = serde_json::to_value(request).expect("request JSON");
    value
        .as_object_mut()
        .expect("request object")
        .insert("trusted".to_string(), serde_json::Value::Bool(true));
    serde_json::from_value::<RemoteArtifactRequest>(value)
        .expect_err("unknown request field must fail");

    let mut unsorted = RemoteReportQuery {
        states: vec![RemoteJobState::Failed, RemoteJobState::Passed],
        suite: None,
        run_id: None,
        finished_after_ms: None,
        finished_before_ms: None,
        limit: 25,
        cursor: None,
    };
    assert_eq!(
        unsorted.validate().expect_err("noncanonical states").code(),
        "test.worker.artifact.query_invalid"
    );
    unsorted.states.sort();
    unsorted.validate().expect("canonical states");

    let artifact_descriptor = descriptor().artifact_descriptor(RemoteRetentionPolicy::default());
    assert_eq!(artifact_descriptor.protocol, REMOTE_ARTIFACT_PROTOCOL);
    assert_eq!(artifact_descriptor.worker, descriptor().identity);
}
