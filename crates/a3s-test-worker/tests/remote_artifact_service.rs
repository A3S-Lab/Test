use std::{path::Path, sync::Arc, time::Duration};

use a3s_test_driver_tui::TuiCapabilities;
use a3s_test_worker::{
    RemoteArtifactKind, RemoteArtifactSelector, RemoteExecutionJob, RemoteExecutionResult,
    RemoteInputBundle, RemoteInputFile, RemoteJobExecutor, RemoteJobState, RemoteJobSubmission,
    RemotePayloadState, RemoteReportQuery, RemoteRetentionPolicy, RemoteScenarioCounts,
    RemoteWorkerDescriptor, RemoteWorkerError, RemoteWorkerIdentity, RemoteWorkerLimits,
    RemoteWorkerService, RemoteWorkerServiceConfig, WorkerCapabilityInventory, WorkerSurface,
    WorkerSurfaceCapability,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
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
            instance_id: "runner-west-1".to_string(),
            image_digest: IMAGE_DIGEST.to_string(),
        },
        inventory,
        RemoteWorkerLimits::default(),
    )
    .expect("worker descriptor")
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
        max_parallel_scenarios: 1,
        required_surfaces: vec![WorkerSurface::Tui],
        required_host_permission_digest: None,
        scenario_ids: vec!["terminal".to_string()],
        input: RemoteInputBundle {
            manifest: "suite.acl".to_string(),
            files: vec![RemoteInputFile::from_bytes(
                "suite.acl",
                format!("suite \"{job_id}\" {{ version = 1 }}\n"),
            )],
        },
    }
}

struct EvidenceExecutor;

#[async_trait]
impl RemoteJobExecutor for EvidenceExecutor {
    async fn execute(
        &self,
        job: RemoteExecutionJob,
        _cancellation: CancellationToken,
    ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
        let screen = job.artifacts_root().join("screens/page.txt");
        let log = job.artifacts_root().join("logs/run.log");
        tokio::fs::create_dir_all(screen.parent().expect("screen parent"))
            .await
            .expect("screen directory");
        tokio::fs::create_dir_all(log.parent().expect("log parent"))
            .await
            .expect("log directory");
        tokio::fs::write(&screen, format!("screen:{}", job.job_id()))
            .await
            .expect("screen evidence");
        tokio::fs::write(&log, format!("log:{}", job.job_id()))
            .await
            .expect("log evidence");
        Ok(RemoteExecutionResult {
            run_id: format!("run-{}", job.job_id()),
            suite: format!("suite-{}", job.job_id()),
            status: RemoteJobState::Passed,
            scenarios: RemoteScenarioCounts {
                passed: 1,
                failed: 0,
                timed_out: 0,
                cancelled: 0,
            },
            report: format!("{{\"job\":\"{}\"}}", job.job_id()).into_bytes(),
            media_type: "application/json".to_string(),
        })
    }
}

async fn open_service(
    root: &Path,
    descriptor: RemoteWorkerDescriptor,
    retention: RemoteRetentionPolicy,
) -> RemoteWorkerService {
    RemoteWorkerService::open(
        RemoteWorkerServiceConfig::new(root.to_path_buf(), descriptor)
            .with_retention_policy(retention),
        Arc::new(EvidenceExecutor),
    )
    .await
    .expect("open artifact-aware service")
}

async fn run_job(
    service: &RemoteWorkerService,
    descriptor: &RemoteWorkerDescriptor,
    job_id: &str,
    dispatch_id: &str,
) -> a3s_test_worker::RemoteJobSnapshot {
    service
        .submit(submission(
            descriptor,
            service.now_ms(),
            job_id,
            dispatch_id,
        ))
        .await
        .expect("submit indexed job");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = service
                .status(job_id, dispatch_id)
                .await
                .expect("indexed job status");
            if snapshot.state.terminal() {
                return snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("indexed job completion")
}

fn report_query(limit: u16, cursor: Option<String>) -> RemoteReportQuery {
    RemoteReportQuery {
        states: vec![RemoteJobState::Passed],
        suite: None,
        run_id: None,
        finished_after_ms: None,
        finished_before_ms: None,
        limit,
        cursor,
    }
}

#[tokio::test]
async fn completed_reports_and_evidence_are_indexed_digest_bound_and_restart_safe() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let retention = RemoteRetentionPolicy::default();
    let service = open_service(temp.path(), descriptor.clone(), retention.clone()).await;
    let terminal = run_job(&service, &descriptor, "job-indexed", "dispatch-indexed").await;
    let request_digest = terminal.request_digest.clone();
    let report = terminal
        .result
        .as_ref()
        .expect("report summary")
        .report
        .clone();

    let page = service
        .list_reports(report_query(10, None))
        .await
        .expect("report index");
    assert_eq!(page.reports.len(), 1);
    assert_eq!(page.reports[0].job, terminal);
    assert_eq!(page.reports[0].payload_state, RemotePayloadState::Retained);
    assert_eq!(page.reports[0].artifact_count, 3);
    assert!(page.reports[0].artifact_bytes > report.bytes);
    assert!(page.next_cursor.is_none());

    let artifacts = service
        .list_artifacts("job-indexed", "dispatch-indexed", &request_digest, 10, None)
        .await
        .expect("artifact index");
    assert_eq!(artifacts.artifacts.len(), 3);
    assert_eq!(artifacts.artifacts[0].kind, RemoteArtifactKind::Report);
    assert!(artifacts.artifacts[0].path.is_none());
    assert_eq!(artifacts.artifacts[1].path.as_deref(), Some("logs/run.log"));
    assert_eq!(
        artifacts.artifacts[2].path.as_deref(),
        Some("screens/page.txt")
    );

    let chunk = service
        .read_artifact(
            "job-indexed",
            "dispatch-indexed",
            &request_digest,
            RemoteArtifactSelector::Report {
                sha256: report.sha256.clone(),
            },
            0,
            5,
        )
        .await
        .expect("report chunk");
    assert_eq!(
        STANDARD
            .decode(&chunk.contents_base64)
            .expect("chunk Base64"),
        b"{\"job"
    );
    assert!(!chunk.eof);

    let evidence = artifacts.artifacts[2].clone();
    let evidence_chunk = service
        .read_artifact(
            "job-indexed",
            "dispatch-indexed",
            &request_digest,
            RemoteArtifactSelector::Evidence {
                path: evidence.path.clone().expect("evidence path"),
                sha256: evidence.sha256.clone(),
            },
            0,
            1_024,
        )
        .await
        .expect("evidence chunk");
    assert_eq!(
        STANDARD
            .decode(&evidence_chunk.contents_base64)
            .expect("evidence Base64"),
        b"screen:job-indexed"
    );
    assert!(evidence_chunk.eof);

    let mismatch = service
        .read_artifact(
            "job-indexed",
            "dispatch-indexed",
            &request_digest,
            RemoteArtifactSelector::Report {
                sha256: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            },
            0,
            5,
        )
        .await
        .expect_err("digest mismatch");
    assert_eq!(mismatch.code(), "test.worker.artifact.digest_mismatch");

    service.shutdown().await.expect("first shutdown");
    drop(service);
    let reopened = open_service(temp.path(), descriptor, retention).await;
    let recovered = reopened
        .list_reports(report_query(10, None))
        .await
        .expect("recovered report index");
    assert_eq!(recovered.reports.len(), 1);
    assert_eq!(recovered.reports[0].job.job_id, "job-indexed");
    reopened.shutdown().await.expect("reopened shutdown");
}

#[tokio::test]
async fn retention_prunes_payload_before_expiring_its_bounded_index() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let retention = RemoteRetentionPolicy {
        max_retained_jobs: 1,
        max_retained_bytes: 1024 * 1024,
        max_retention_age_ms: 60_000,
        max_indexed_jobs: 2,
        max_index_age_ms: 120_000,
    };
    let service = open_service(temp.path(), descriptor.clone(), retention).await;

    let first = run_job(&service, &descriptor, "job-1", "dispatch-1").await;
    run_job(&service, &descriptor, "job-2", "dispatch-2").await;
    let after_second = service
        .list_reports(report_query(10, None))
        .await
        .expect("two indexed reports");
    assert_eq!(
        after_second
            .reports
            .iter()
            .map(|entry| (&*entry.job.job_id, entry.payload_state))
            .collect::<Vec<_>>(),
        vec![
            ("job-2", RemotePayloadState::Retained),
            ("job-1", RemotePayloadState::Pruned),
        ]
    );
    assert!(!temp.path().join("jobs/job-1/report.bin").exists());
    assert!(!temp.path().join("jobs/job-1/artifacts").exists());
    assert!(!temp.path().join("jobs/job-1/input").exists());
    assert_eq!(
        service
            .read_artifact(
                "job-1",
                "dispatch-1",
                &first.request_digest,
                RemoteArtifactSelector::Report {
                    sha256: first
                        .result
                        .as_ref()
                        .expect("first report")
                        .report
                        .sha256
                        .clone(),
                },
                0,
                10,
            )
            .await
            .expect_err("pruned report")
            .code(),
        "test.worker.artifact.payload_pruned"
    );

    run_job(&service, &descriptor, "job-3", "dispatch-3").await;
    let after_third = service
        .list_reports(report_query(10, None))
        .await
        .expect("bounded report index");
    assert_eq!(
        after_third
            .reports
            .iter()
            .map(|entry| entry.job.job_id.as_str())
            .collect::<Vec<_>>(),
        vec!["job-3", "job-2"]
    );
    assert!(!temp.path().join("jobs/job-1").exists());
    assert_eq!(
        service
            .status("job-1", "dispatch-1")
            .await
            .expect_err("expired index status")
            .code(),
        "test.worker.remote.job_not_found"
    );
    service.shutdown().await.expect("service shutdown");
}

#[tokio::test]
async fn report_and_artifact_cursors_are_bound_and_replacement_is_detected() {
    let temp = TempDir::new().expect("temporary state root");
    let descriptor = descriptor();
    let service = open_service(
        temp.path(),
        descriptor.clone(),
        RemoteRetentionPolicy::default(),
    )
    .await;
    let first = run_job(&service, &descriptor, "job-a", "dispatch-a").await;
    run_job(&service, &descriptor, "job-b", "dispatch-b").await;
    run_job(&service, &descriptor, "job-c", "dispatch-c").await;

    let first_page = service
        .list_reports(report_query(1, None))
        .await
        .expect("first report page");
    assert_eq!(first_page.reports.len(), 1);
    let first_job = first_page.reports[0].job.job_id.clone();
    let cursor = first_page.next_cursor.expect("next report cursor");
    let second_page = service
        .list_reports(report_query(1, Some(cursor.clone())))
        .await
        .expect("second report page");
    assert_eq!(second_page.reports.len(), 1);
    assert_ne!(second_page.reports[0].job.job_id, first_job);

    let mut changed_query = report_query(1, Some(cursor));
    changed_query.suite = Some("suite-job-a".to_string());
    assert_eq!(
        service
            .list_reports(changed_query)
            .await
            .expect_err("cursor/query mismatch")
            .code(),
        "test.worker.artifact.cursor_mismatch"
    );

    let artifact_page = service
        .list_artifacts("job-a", "dispatch-a", &first.request_digest, 1, None)
        .await
        .expect("first artifact page");
    assert_eq!(artifact_page.artifacts.len(), 1);
    assert_eq!(artifact_page.artifacts[0].kind, RemoteArtifactKind::Report);
    let artifact_cursor = artifact_page.next_cursor.expect("artifact cursor");
    assert_eq!(
        service
            .list_artifacts(
                "job-a",
                "dispatch-a",
                &first.request_digest,
                1,
                Some("a".repeat(513)),
            )
            .await
            .expect_err("oversized artifact cursor")
            .code(),
        "test.worker.artifact.cursor_invalid"
    );
    let next_artifact = service
        .list_artifacts(
            "job-a",
            "dispatch-a",
            &first.request_digest,
            1,
            Some(artifact_cursor),
        )
        .await
        .expect("second artifact page");
    assert_eq!(next_artifact.artifacts.len(), 1);
    assert_eq!(
        next_artifact.artifacts[0].kind,
        RemoteArtifactKind::Evidence
    );

    let all_artifacts = service
        .list_artifacts("job-a", "dispatch-a", &first.request_digest, 10, None)
        .await
        .expect("complete artifact index");
    let screen = all_artifacts
        .artifacts
        .iter()
        .find(|artifact| artifact.path.as_deref() == Some("screens/page.txt"))
        .expect("screen descriptor")
        .clone();
    tokio::fs::write(
        temp.path().join("jobs/job-a/artifacts/screens/page.txt"),
        b"tamper:job-a",
    )
    .await
    .expect("replace indexed evidence");
    assert_eq!(
        service
            .read_artifact(
                "job-a",
                "dispatch-a",
                &first.request_digest,
                RemoteArtifactSelector::Evidence {
                    path: screen.path.expect("screen path"),
                    sha256: screen.sha256,
                },
                0,
                1_024,
            )
            .await
            .expect_err("replaced evidence")
            .code(),
        "test.worker.artifact.digest_mismatch"
    );
    service.shutdown().await.expect("service shutdown");
}
