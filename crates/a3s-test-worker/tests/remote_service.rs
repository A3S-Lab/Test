use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use a3s_test_driver_tui::TuiCapabilities;
use a3s_test_worker::{
    RemoteExecutionJob, RemoteExecutionResult, RemoteInputBundle, RemoteInputFile,
    RemoteJobExecutor, RemoteJobState, RemoteJobSubmission, RemoteScenarioCounts,
    RemoteWorkerDescriptor, RemoteWorkerError, RemoteWorkerIdentity, RemoteWorkerLimits,
    RemoteWorkerService, RemoteWorkerServiceConfig, WorkerCapabilityInventory, WorkerSurface,
    WorkerSurfaceCapability,
};
use async_trait::async_trait;
use tempfile::TempDir;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

const IMAGE_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn descriptor(max_queued_jobs: u16) -> RemoteWorkerDescriptor {
    let inventory = WorkerCapabilityInventory::local(
        4,
        vec![WorkerSurfaceCapability::Tui {
            terminal: TuiCapabilities::compiled().expect("compiled TUI capabilities"),
        }],
    )
    .expect("worker inventory");
    let limits = RemoteWorkerLimits {
        max_queued_jobs,
        ..RemoteWorkerLimits::default()
    };
    RemoteWorkerDescriptor::new(
        RemoteWorkerIdentity {
            instance_id: "runner-west-1".to_string(),
            image_digest: IMAGE_DIGEST.to_string(),
        },
        inventory,
        limits,
    )
    .expect("remote worker descriptor")
}

fn submission(
    descriptor: &RemoteWorkerDescriptor,
    now_ms: u64,
    job_id: &str,
    dispatch_id: &str,
) -> RemoteJobSubmission {
    RemoteJobSubmission {
        job_id: job_id.to_string(),
        dispatch_id: dispatch_id.to_string(),
        worker_instance: descriptor.identity.instance_id.clone(),
        required_image_digest: descriptor.identity.image_digest.clone(),
        required_inventory_digest: descriptor.inventory_digest.clone(),
        issued_at_ms: now_ms,
        deadline_ms: now_ms + 60_000,
        lease_expires_at_ms: now_ms + 30_000,
        max_parallel_scenarios: 2,
        required_surfaces: vec![WorkerSurface::Tui],
        required_host_permission_digest: None,
        scenario_ids: vec!["terminal".to_string()],
        input: RemoteInputBundle {
            manifest: "suite.acl".to_string(),
            files: vec![RemoteInputFile::from_bytes(
                "suite.acl",
                format!(
                    "suite \"{job_id}\" {{\n  version = 1\n  scenario \"terminal\" {{\n    surface = \"tui\"\n    expect \"ready\" {{ text = \"ready\" }}\n  }}\n}}\n"
                ),
            )],
        },
    }
}

async fn wait_for_state(
    service: &RemoteWorkerService,
    job_id: &str,
    dispatch_id: &str,
    expected: impl Fn(RemoteJobState) -> bool,
) -> a3s_test_worker::RemoteJobSnapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = service
                .status(job_id, dispatch_id)
                .await
                .expect("job status");
            if expected(snapshot.state) {
                return snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("state transition timeout")
}

#[derive(Default)]
struct BlockingExecutor {
    started: Notify,
    executions: AtomicUsize,
}

#[async_trait]
impl RemoteJobExecutor for BlockingExecutor {
    async fn execute(
        &self,
        _job: RemoteExecutionJob,
        cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.started.notify_waiters();
        cancellation.cancelled().await;
        Err(RemoteWorkerError::new(
            "test.fixture.cancelled",
            "fixture execution observed cancellation",
            false,
        ))
    }
}

struct PassingExecutor {
    report: Vec<u8>,
}

struct StubbornExecutor;

#[async_trait]
impl RemoteJobExecutor for StubbornExecutor {
    async fn execute(
        &self,
        _job: RemoteExecutionJob,
        _cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        std::future::pending().await
    }
}

#[async_trait]
impl RemoteJobExecutor for PassingExecutor {
    async fn execute(
        &self,
        job: RemoteExecutionJob,
        _cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        assert!(job.manifest_path().ends_with("suite.acl"));
        assert!(tokio::fs::try_exists(job.manifest_path())
            .await
            .expect("manifest existence"));
        Ok(RemoteExecutionResult {
            run_id: format!("run-{}", job.job_id()),
            suite: job.job_id().to_string(),
            status: RemoteJobState::Passed,
            scenarios: RemoteScenarioCounts {
                passed: 1,
                failed: 0,
                timed_out: 0,
                cancelled: 0,
            },
            report: self.report.clone(),
            media_type: "application/json".to_string(),
        })
    }
}

async fn open_service(
    root: &Path,
    descriptor: RemoteWorkerDescriptor,
    executor: Arc<dyn RemoteJobExecutor>,
) -> RemoteWorkerService {
    RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(root.to_path_buf(), descriptor),
        executor,
    )
    .await
    .expect("open remote worker service")
}

#[tokio::test]
async fn duplicate_dispatch_is_idempotent_and_conflicts_fail_closed() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor(2);
    let executor = Arc::new(BlockingExecutor::default());
    let service = open_service(temp.path(), descriptor.clone(), executor.clone()).await;
    let now_ms = service.now_ms();
    let first = submission(&descriptor, now_ms, "job-1", "dispatch-1");

    let accepted = service.submit(first.clone()).await.expect("first submit");
    let duplicate = service
        .submit(first.clone())
        .await
        .expect("duplicate submit");
    assert_eq!(duplicate.request_digest, accepted.request_digest);
    assert_eq!(duplicate.job_id, accepted.job_id);

    let conflicting_job = submission(&descriptor, now_ms, "job-2", "dispatch-1");
    assert_eq!(
        service
            .submit(conflicting_job)
            .await
            .expect_err("dispatch conflict")
            .code(),
        "test.worker.remote.dispatch_conflict"
    );

    let conflicting_payload = submission(&descriptor, now_ms, "job-1", "dispatch-2");
    assert_eq!(
        service
            .submit(conflicting_payload)
            .await
            .expect_err("job conflict")
            .code(),
        "test.worker.remote.job_conflict"
    );

    wait_for_state(&service, "job-1", "dispatch-1", |state| {
        state == RemoteJobState::Running
    })
    .await;
    service
        .cancel("job-1", "dispatch-1", Some("test cleanup".to_string()))
        .await
        .expect("cancel first job");
    wait_for_state(&service, "job-1", "dispatch-1", RemoteJobState::terminal).await;
    assert_eq!(executor.executions.load(Ordering::SeqCst), 1);
    service.shutdown().await.expect("service shutdown");
}

#[tokio::test]
async fn queue_is_bounded_and_failed_admission_does_not_materialize_input() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor(1);
    let executor = Arc::new(BlockingExecutor::default());
    let service = open_service(temp.path(), descriptor.clone(), executor.clone()).await;
    let now_ms = service.now_ms();

    service
        .submit(submission(
            &descriptor,
            now_ms,
            "job-running",
            "dispatch-running",
        ))
        .await
        .expect("running job submit");
    wait_for_state(&service, "job-running", "dispatch-running", |state| {
        state == RemoteJobState::Running
    })
    .await;

    service
        .submit(submission(
            &descriptor,
            now_ms,
            "job-queued",
            "dispatch-queued",
        ))
        .await
        .expect("queued job submit");
    let full = service
        .submit(submission(&descriptor, now_ms, "job-full", "dispatch-full"))
        .await
        .expect_err("queue must be full");
    assert_eq!(full.code(), "test.worker.remote.queue_full");

    let mut invalid = submission(&descriptor, now_ms, "job-invalid", "dispatch-invalid");
    invalid.input.files[0].path = "../suite.acl".to_string();
    invalid.input.manifest = "../suite.acl".to_string();
    let invalid_error = service
        .submit(invalid)
        .await
        .expect_err("invalid input path");
    assert_eq!(
        invalid_error.code(),
        "test.worker.remote.input_path_invalid"
    );

    assert!(!temp.path().join("jobs/job-full").exists());
    assert!(!temp.path().join("jobs/job-invalid").exists());

    service
        .cancel(
            "job-running",
            "dispatch-running",
            Some("test cleanup".to_string()),
        )
        .await
        .expect("cancel running job");
    service
        .cancel(
            "job-queued",
            "dispatch-queued",
            Some("test cleanup".to_string()),
        )
        .await
        .expect("cancel queued job");
    service.shutdown().await.expect("service shutdown");
}

#[tokio::test]
async fn lease_renewal_and_cancel_have_persisted_state_transitions() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor(2);
    let executor = Arc::new(BlockingExecutor::default());
    let service = open_service(temp.path(), descriptor.clone(), executor).await;
    let now_ms = service.now_ms();

    service
        .submit(submission(
            &descriptor,
            now_ms,
            "job-lease",
            "dispatch-lease",
        ))
        .await
        .expect("job submit");
    wait_for_state(&service, "job-lease", "dispatch-lease", |state| {
        state == RemoteJobState::Running
    })
    .await;

    let renewed_lease = now_ms + 50_000;
    let renewed = service
        .renew_lease("job-lease", "dispatch-lease", renewed_lease)
        .await
        .expect("renew lease");
    assert_eq!(renewed.lease_expires_at_ms, renewed_lease);

    let cancelling = service
        .cancel(
            "job-lease",
            "dispatch-lease",
            Some("operator requested".to_string()),
        )
        .await
        .expect("cancel job");
    assert!(matches!(
        cancelling.state,
        RemoteJobState::Cancelling | RemoteJobState::Cancelled
    ));
    let cancelled = wait_for_state(
        &service,
        "job-lease",
        "dispatch-lease",
        RemoteJobState::terminal,
    )
    .await;
    assert_eq!(cancelled.state, RemoteJobState::Cancelled);
    assert_eq!(
        cancelled.error.expect("cancellation reason").code(),
        "test.worker.remote.cancelled"
    );

    let mut events = tokio::fs::read_dir(temp.path().join("jobs/job-lease/events"))
        .await
        .expect("persisted events");
    let mut event_count = 0;
    while events
        .next_entry()
        .await
        .expect("persisted event entry")
        .is_some()
    {
        event_count += 1;
    }
    assert!(event_count >= 4);
    service.shutdown().await.expect("service shutdown");
}

#[tokio::test(start_paused = true)]
async fn lease_expiry_and_deadline_cancel_owned_execution() {
    let lease_temp = TempDir::new().expect("lease state root");
    let descriptor = descriptor(2);
    let lease_executor = Arc::new(BlockingExecutor::default());
    let lease_service = open_service(
        lease_temp.path(),
        descriptor.clone(),
        lease_executor.clone(),
    )
    .await;
    let lease_now = lease_service.now_ms();
    let mut lease_job = submission(
        &descriptor,
        lease_now,
        "job-lease-expiry",
        "dispatch-lease-expiry",
    );
    lease_job.lease_expires_at_ms = lease_now + 1_000;
    lease_job.deadline_ms = lease_now + 5_000;
    lease_service.submit(lease_job).await.expect("lease job");
    wait_for_state(
        &lease_service,
        "job-lease-expiry",
        "dispatch-lease-expiry",
        |state| state == RemoteJobState::Running,
    )
    .await;
    tokio::time::advance(Duration::from_millis(1_000)).await;
    let lease_terminal = wait_for_state(
        &lease_service,
        "job-lease-expiry",
        "dispatch-lease-expiry",
        RemoteJobState::terminal,
    )
    .await;
    assert_eq!(lease_terminal.state, RemoteJobState::Cancelled);
    assert_eq!(
        lease_terminal.error.expect("lease error").code(),
        "test.worker.remote.lease_expired"
    );
    assert_eq!(lease_executor.executions.load(Ordering::SeqCst), 1);
    lease_service.shutdown().await.expect("lease shutdown");

    let deadline_temp = TempDir::new().expect("deadline state root");
    let deadline_executor = Arc::new(BlockingExecutor::default());
    let deadline_service = open_service(
        deadline_temp.path(),
        descriptor.clone(),
        deadline_executor.clone(),
    )
    .await;
    let deadline_now = deadline_service.now_ms();
    let mut deadline_job = submission(
        &descriptor,
        deadline_now,
        "job-deadline",
        "dispatch-deadline",
    );
    deadline_job.deadline_ms = deadline_now + 1_000;
    deadline_job.lease_expires_at_ms = deadline_job.deadline_ms;
    deadline_service
        .submit(deadline_job)
        .await
        .expect("deadline job");
    wait_for_state(
        &deadline_service,
        "job-deadline",
        "dispatch-deadline",
        |state| state == RemoteJobState::Running,
    )
    .await;
    tokio::time::advance(Duration::from_millis(1_000)).await;
    let deadline_terminal = wait_for_state(
        &deadline_service,
        "job-deadline",
        "dispatch-deadline",
        RemoteJobState::terminal,
    )
    .await;
    assert_eq!(deadline_terminal.state, RemoteJobState::TimedOut);
    assert_eq!(
        deadline_terminal.error.expect("deadline error").code(),
        "test.worker.remote.deadline_exceeded"
    );
    assert_eq!(deadline_executor.executions.load(Ordering::SeqCst), 1);
    deadline_service
        .shutdown()
        .await
        .expect("deadline shutdown");
}

#[tokio::test(start_paused = true)]
async fn cancellation_bounds_an_executor_that_ignores_its_token() {
    let temp = TempDir::new().expect("temporary state root");
    let mut descriptor = descriptor(2);
    descriptor.limits.cleanup_timeout_ms = 1_000;
    let service = open_service(temp.path(), descriptor.clone(), Arc::new(StubbornExecutor)).await;
    let now_ms = service.now_ms();
    service
        .submit(submission(
            &descriptor,
            now_ms,
            "job-stubborn",
            "dispatch-stubborn",
        ))
        .await
        .expect("stubborn job submit");
    wait_for_state(&service, "job-stubborn", "dispatch-stubborn", |state| {
        state == RemoteJobState::Running
    })
    .await;
    service
        .cancel(
            "job-stubborn",
            "dispatch-stubborn",
            Some("bounded cleanup test".to_string()),
        )
        .await
        .expect("cancel stubborn job");
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(750)).await;
    let terminal = wait_for_state(
        &service,
        "job-stubborn",
        "dispatch-stubborn",
        RemoteJobState::terminal,
    )
    .await;
    assert_eq!(terminal.state, RemoteJobState::Cancelled);
    assert!(terminal
        .error
        .expect("bounded cancellation error")
        .message
        .contains("executor cleanup exceeded 750 ms"));
    service.shutdown().await.expect("service shutdown");
}

#[tokio::test]
async fn report_digest_and_summary_survive_service_restart() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor(2);
    let report = br#"{"status":"passed","scenarios":1}"#.to_vec();
    let executor = Arc::new(PassingExecutor {
        report: report.clone(),
    });
    let service = open_service(temp.path(), descriptor.clone(), executor).await;
    let now_ms = service.now_ms();
    service
        .submit(submission(
            &descriptor,
            now_ms,
            "job-report",
            "dispatch-report",
        ))
        .await
        .expect("report job submit");
    let terminal = wait_for_state(
        &service,
        "job-report",
        "dispatch-report",
        RemoteJobState::terminal,
    )
    .await;
    assert_eq!(terminal.state, RemoteJobState::Passed);
    let summary = terminal.result.as_ref().expect("run summary");
    assert_eq!(summary.report.bytes, report.len() as u64);
    assert!(summary.report.sha256.starts_with("sha256:"));
    assert_eq!(
        tokio::fs::read(temp.path().join("jobs/job-report/report.bin"))
            .await
            .expect("persisted report"),
        report
    );
    service.shutdown().await.expect("first shutdown");
    drop(service);

    let reopened = open_service(
        temp.path(),
        descriptor,
        Arc::new(PassingExecutor { report: Vec::new() }),
    )
    .await;
    let recovered = reopened
        .status("job-report", "dispatch-report")
        .await
        .expect("recovered status");
    assert_eq!(recovered, terminal);
    reopened.shutdown().await.expect("reopened shutdown");
}

#[tokio::test]
async fn restart_marks_the_last_durable_nonterminal_state_interrupted() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor(2);
    let service = open_service(
        temp.path(),
        descriptor.clone(),
        Arc::new(BlockingExecutor::default()),
    )
    .await;
    let now_ms = service.now_ms();
    service
        .submit(submission(
            &descriptor,
            now_ms,
            "job-restart",
            "dispatch-restart",
        ))
        .await
        .expect("restart job submit");
    wait_for_state(&service, "job-restart", "dispatch-restart", |state| {
        state == RemoteJobState::Running
    })
    .await;
    service.shutdown().await.expect("first shutdown");
    drop(service);

    let events_root = temp.path().join("jobs/job-restart/events");
    let mut directory = tokio::fs::read_dir(&events_root)
        .await
        .expect("job events directory");
    let mut event_paths = Vec::new();
    while let Some(entry) = directory.next_entry().await.expect("job event entry") {
        event_paths.push(entry.path());
    }
    event_paths.sort();
    assert!(event_paths.len() >= 3);
    for terminal_event in event_paths.into_iter().skip(2) {
        tokio::fs::remove_file(terminal_event)
            .await
            .expect("remove simulated post-crash event");
    }

    let reopened = open_service(
        temp.path(),
        descriptor,
        Arc::new(BlockingExecutor::default()),
    )
    .await;
    let recovered = reopened
        .status("job-restart", "dispatch-restart")
        .await
        .expect("recovered interrupted job");
    assert_eq!(recovered.state, RemoteJobState::Interrupted);
    assert_eq!(
        recovered.error.expect("restart interruption").code(),
        "test.worker.remote.restart_interrupted"
    );
    reopened.shutdown().await.expect("reopened shutdown");
}

#[tokio::test]
async fn state_root_is_exclusive_and_bound_to_the_exact_worker_descriptor() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor(2);
    let first = open_service(
        temp.path(),
        descriptor.clone(),
        Arc::new(BlockingExecutor::default()),
    )
    .await;
    let invalid_request = first
        .handle(a3s_test_worker::RemoteWorkerRequest {
            protocol: a3s_test_worker::REMOTE_WORKER_PROTOCOL.to_string(),
            request_id: "x".repeat(1_000),
            command: a3s_test_worker::RemoteWorkerCommand::Inspect,
        })
        .await;
    assert_eq!(invalid_request.request_id, "invalid-request");
    let a3s_test_worker::RemoteWorkerOutcome::Error { error } = invalid_request.outcome else {
        panic!("invalid request ID must return a protocol error");
    };
    assert_eq!(error.code(), "test.worker.remote.identifier_invalid");

    let concurrent = RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(temp.path().to_path_buf(), descriptor.clone()),
        Arc::new(BlockingExecutor::default()),
    )
    .await
    .err()
    .expect("concurrent state root must fail");
    assert_eq!(concurrent.code(), "test.worker.remote.state_locked");
    first.shutdown().await.expect("first shutdown");
    drop(first);

    let mut different_descriptor = descriptor;
    different_descriptor.identity.image_digest =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    let mismatch = RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(temp.path().to_path_buf(), different_descriptor),
        Arc::new(BlockingExecutor::default()),
    )
    .await
    .err()
    .expect("descriptor mismatch must fail");
    assert_eq!(mismatch.code(), "test.worker.remote.state_worker_mismatch");
}

#[tokio::test]
async fn dropping_the_last_service_handle_releases_the_state_root() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor(2);
    let first = open_service(
        temp.path(),
        descriptor.clone(),
        Arc::new(BlockingExecutor::default()),
    )
    .await;
    let last_handle = first.clone();
    drop(first);

    let still_locked = RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(temp.path().to_path_buf(), descriptor.clone()),
        Arc::new(BlockingExecutor::default()),
    )
    .await
    .err()
    .expect("a surviving service handle must retain the state lock");
    assert_eq!(still_locked.code(), "test.worker.remote.state_locked");

    drop(last_handle);
    let reopened = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match RemoteWorkerService::open(
                RemoteWorkerServiceConfig::new(temp.path().to_path_buf(), descriptor.clone()),
                Arc::new(BlockingExecutor::default()),
            )
            .await
            {
                Ok(service) => break service,
                Err(error) if error.code() == "test.worker.remote.state_locked" => {
                    tokio::task::yield_now().await;
                }
                Err(error) => panic!("unexpected reopen failure: {error}"),
            }
        }
    })
    .await
    .expect("dropped service did not release its state root");
    reopened.shutdown().await.expect("reopened shutdown");
}
