#![cfg(any(unix, windows))]

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use a3s_test_driver_tui::TuiCapabilities;
use a3s_test_worker::{
    RemoteExecutionJob, RemoteExecutionResult, RemoteInputBundle, RemoteInputFile,
    RemoteJobExecutor, RemoteJobState, RemoteJobSubmission, RemotePayloadState, RemoteReportQuery,
    RemoteScenarioCounts, RemoteWorkerDescriptor, RemoteWorkerError, RemoteWorkerIdentity,
    RemoteWorkerLimits, RemoteWorkerService, RemoteWorkerServiceConfig, WorkerCapabilityInventory,
    WorkerSurface, WorkerSurfaceCapability,
};
use async_trait::async_trait;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

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
            instance_id: "runner-safety".to_string(),
            image_digest: IMAGE_DIGEST.to_string(),
        },
        inventory,
        RemoteWorkerLimits::default(),
    )
    .expect("worker descriptor")
}

fn submission(descriptor: &RemoteWorkerDescriptor, now_ms: u64) -> RemoteJobSubmission {
    RemoteJobSubmission {
        job_id: "job-safety".to_string(),
        dispatch_id: "dispatch-safety".to_string(),
        worker_instance: descriptor.identity.instance_id.clone(),
        required_image_digest: descriptor.identity.image_digest.clone(),
        required_inventory_digest: descriptor.inventory_digest.clone(),
        issued_at_ms: now_ms,
        deadline_ms: now_ms + 60_000,
        lease_expires_at_ms: now_ms + 30_000,
        max_parallel_scenarios: 1,
        required_surfaces: vec![WorkerSurface::Tui],
        input: RemoteInputBundle {
            manifest: "suite.acl".to_string(),
            files: vec![RemoteInputFile::from_bytes(
                "suite.acl",
                b"suite \"safety\" { version = 1 }\n",
            )],
        },
    }
}

struct SafeExecutor;

#[async_trait]
impl RemoteJobExecutor for SafeExecutor {
    async fn execute(
        &self,
        job: RemoteExecutionJob,
        _cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        tokio::fs::create_dir_all(job.artifacts_root())
            .await
            .expect("artifact root");
        tokio::fs::write(
            job.artifacts_root().join("evidence.txt"),
            b"bounded evidence",
        )
        .await
        .expect("safe evidence");
        Ok(passed_result())
    }
}

struct LinkExecutor {
    outside: PathBuf,
}

#[async_trait]
impl RemoteJobExecutor for LinkExecutor {
    async fn execute(
        &self,
        job: RemoteExecutionJob,
        _cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        tokio::fs::create_dir_all(job.artifacts_root())
            .await
            .expect("artifact root");
        symlink_file(&self.outside, &job.artifacts_root().join("escape.txt"))
            .expect("unsafe evidence link");
        Ok(passed_result())
    }
}

fn passed_result() -> RemoteExecutionResult {
    RemoteExecutionResult {
        run_id: "run-safety".to_string(),
        suite: "suite-safety".to_string(),
        status: RemoteJobState::Passed,
        scenarios: RemoteScenarioCounts {
            passed: 1,
            failed: 0,
            timed_out: 0,
            cancelled: 0,
        },
        report: br#"{"status":"passed"}"#.to_vec(),
        media_type: "application/json".to_string(),
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
    .expect("open safety service")
}

async fn run_job(
    service: &RemoteWorkerService,
    descriptor: &RemoteWorkerDescriptor,
) -> a3s_test_worker::RemoteJobSnapshot {
    service
        .submit(submission(descriptor, service.now_ms()))
        .await
        .expect("submit safety job");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = service
                .status("job-safety", "dispatch-safety")
                .await
                .expect("safety job status");
            if snapshot.state.terminal() {
                return snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("safety job completion")
}

fn report_query(state: RemoteJobState) -> RemoteReportQuery {
    RemoteReportQuery {
        states: vec![state],
        suite: None,
        run_id: None,
        finished_after_ms: None,
        finished_before_ms: None,
        limit: 10,
        cursor: None,
    }
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

fn unavailable_without_host_privilege(error: &std::io::Error) -> bool {
    cfg!(windows)
        && matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        )
}

#[tokio::test]
async fn link_like_evidence_turns_a_success_result_into_a_durable_failure() {
    let temp = TempDir::new().expect("temporary state root");
    let outside = temp.path().join("outside.txt");
    tokio::fs::write(&outside, b"must remain untouched")
        .await
        .expect("outside fixture");
    let probe = temp.path().join("link-probe.txt");
    if let Err(error) = symlink_file(&outside, &probe) {
        if unavailable_without_host_privilege(&error) {
            return;
        }
        panic!("failed to create artifact link probe: {error}");
    }
    tokio::fs::remove_file(probe)
        .await
        .expect("remove artifact link probe");
    let descriptor = descriptor();
    let service = open_service(
        &temp.path().join("state"),
        descriptor.clone(),
        Arc::new(LinkExecutor {
            outside: outside.clone(),
        }),
    )
    .await;
    let terminal = run_job(&service, &descriptor).await;
    assert_eq!(terminal.state, RemoteJobState::Failed);
    assert_eq!(
        terminal.error.expect("artifact failure").code(),
        "test.worker.artifact.path_invalid"
    );
    let indexed = service
        .list_reports(report_query(RemoteJobState::Failed))
        .await
        .expect("failed report index");
    assert_eq!(indexed.reports.len(), 1);
    assert_eq!(indexed.reports[0].payload_state, RemotePayloadState::Pruned);
    assert_eq!(indexed.reports[0].artifact_count, 0);
    assert_eq!(
        tokio::fs::read(&outside).await.expect("outside fixture"),
        b"must remain untouched"
    );
    service.shutdown().await.expect("first shutdown");
    drop(service);

    let reopened = open_service(
        &temp.path().join("state"),
        descriptor,
        Arc::new(SafeExecutor),
    )
    .await;
    let recovered = reopened
        .status("job-safety", "dispatch-safety")
        .await
        .expect("recovered safety failure");
    assert_eq!(recovered.state, RemoteJobState::Failed);
    reopened.shutdown().await.expect("reopened shutdown");
}

#[tokio::test]
async fn restart_finishes_a_crash_interrupted_payload_prune() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let service = open_service(temp.path(), descriptor.clone(), Arc::new(SafeExecutor)).await;
    let terminal = run_job(&service, &descriptor).await;
    assert_eq!(terminal.state, RemoteJobState::Passed);
    service.shutdown().await.expect("first shutdown");
    drop(service);

    let index_path = temp.path().join("jobs/job-safety/artifact-index.json");
    let mut index: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(&index_path)
            .await
            .expect("persisted artifact index"),
    )
    .expect("artifact index JSON");
    index["state"] = serde_json::Value::String("pruning".to_string());
    tokio::fs::write(
        &index_path,
        serde_json::to_vec(&index).expect("updated artifact index"),
    )
    .await
    .expect("simulate interrupted prune");

    let reopened = open_service(temp.path(), descriptor, Arc::new(SafeExecutor)).await;
    let reports = reopened
        .list_reports(report_query(RemoteJobState::Passed))
        .await
        .expect("recovered report index");
    assert_eq!(reports.reports.len(), 1);
    assert_eq!(reports.reports[0].payload_state, RemotePayloadState::Pruned);
    assert!(!temp.path().join("jobs/job-safety/input").exists());
    assert!(!temp.path().join("jobs/job-safety/report.bin").exists());
    assert!(!temp.path().join("jobs/job-safety/artifacts").exists());
    reopened.shutdown().await.expect("reopened shutdown");
}

#[tokio::test]
async fn restart_rejects_a_corrupted_pruned_artifact_index() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let service = open_service(temp.path(), descriptor.clone(), Arc::new(SafeExecutor)).await;
    let terminal = run_job(&service, &descriptor).await;
    assert_eq!(terminal.state, RemoteJobState::Passed);
    service.shutdown().await.expect("first shutdown");
    drop(service);

    let index_path = temp.path().join("jobs/job-safety/artifact-index.json");
    let mut index: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(&index_path)
            .await
            .expect("persisted artifact index"),
    )
    .expect("artifact index JSON");
    index["state"] = serde_json::Value::String("pruned".to_string());
    index["retained_bytes"] = serde_json::Value::Number(0_u64.into());
    index["files"][0]["bytes"] = serde_json::Value::Number(u64::MAX.into());
    tokio::fs::write(
        &index_path,
        serde_json::to_vec(&index).expect("corrupted artifact index"),
    )
    .await
    .expect("persist corrupted artifact index");

    let error = match RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(temp.path().to_path_buf(), descriptor),
        Arc::new(SafeExecutor),
    )
    .await
    {
        Ok(service) => {
            service
                .shutdown()
                .await
                .expect("unexpected service shutdown");
            panic!("corrupted artifact index must fail closed");
        }
        Err(error) => error,
    };
    assert_eq!(error.code(), "test.worker.artifact.index_invalid");
}
