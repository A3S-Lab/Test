use std::path::PathBuf;
use std::process::Command;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn distributed_schema_exposes_the_safety_invariants() {
    let output = Command::new(binary())
        .args(["distributed", "schema", "--compact"])
        .output()
        .expect("run distributed schema");
    assert!(output.status.success(), "{output:?}");
    let schema: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("distributed schema JSON");
    assert_eq!(schema["protocol"], "a3s.test.distributed-run/1");
    assert_eq!(schema["invariants"]["deterministic_sharding"], true);
    assert_eq!(
        schema["invariants"]["infrastructure_failures_never_quarantined"],
        true
    );
    assert_eq!(schema["plan_schema"]["additionalProperties"], false);
    assert_eq!(schema["analysis_schema"]["additionalProperties"], false);
}

#[cfg(unix)]
mod unix {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use a3s_test_core::Surface;
    use a3s_test_driver_tui::TuiCapabilities;
    use a3s_test_runner::{RunError, RunResult, RunStatus, ScenarioResult, StepResult};
    use a3s_test_worker::{
        RemoteExecutionJob, RemoteExecutionResult, RemoteJobExecutor, RemoteJobState,
        RemoteReportQuery, RemoteScenarioCounts, RemoteWorkerDescriptor, RemoteWorkerError,
        RemoteWorkerIdentity, RemoteWorkerService, RemoteWorkerServiceConfig,
        WorkerCapabilityInventory, WorkerSurfaceCapability,
    };
    use async_trait::async_trait;
    use axum::extract::State;
    use axum::http::{header, HeaderMap, StatusCode};
    use axum::response::{IntoResponse, Response};
    use axum::routing::post;
    use axum::{Json, Router};
    use tokio::net::TcpListener;
    use tokio_util::sync::CancellationToken;

    use std::path::Path;

    use super::{binary, Command};

    const IMAGE_DIGEST: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SUITE: &str = "distributed-tui";

    #[derive(Clone)]
    struct HttpState {
        service: RemoteWorkerService,
        authorization: String,
        cancel_seen: Arc<AtomicBool>,
        tamper_status_binding: bool,
    }

    struct MockWorker {
        endpoint: String,
        authorization_env: String,
        authorization: String,
        instance_id: String,
        service: RemoteWorkerService,
        state_root: std::path::PathBuf,
        cancel_seen: Arc<AtomicBool>,
        shutdown: CancellationToken,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl MockWorker {
        fn start(root: &Path, instance_id: &str, fixed: Arc<AtomicBool>, blocking: bool) -> Self {
            Self::start_with_options(root, instance_id, fixed, blocking, false)
        }

        fn start_with_tampered_status(
            root: &Path,
            instance_id: &str,
            fixed: Arc<AtomicBool>,
            blocking: bool,
        ) -> Self {
            Self::start_with_options(root, instance_id, fixed, blocking, true)
        }

        fn start_with_options(
            root: &Path,
            instance_id: &str,
            fixed: Arc<AtomicBool>,
            blocking: bool,
            tamper_status_binding: bool,
        ) -> Self {
            let authorization_env = format!(
                "A3S_TEST_WORKER_AUTHORIZATION_{}",
                instance_id.to_ascii_uppercase()
            );
            let authorization = format!("Bearer {instance_id}-secret");
            let state_root = root.join(format!("{instance_id}-state"));
            let server_state_root = state_root.clone();
            let shutdown = CancellationToken::new();
            let server_shutdown = shutdown.clone();
            let server_authorization = authorization.clone();
            let server_instance = instance_id.to_string();
            let cancel_seen = Arc::new(AtomicBool::new(false));
            let server_cancel_seen = Arc::clone(&cancel_seen);
            let (ready_tx, ready_rx) = mpsc::channel();
            let worker_thread = thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("mock worker runtime");
                runtime.block_on(async move {
                    let inventory = WorkerCapabilityInventory::local(
                        1,
                        vec![WorkerSurfaceCapability::Tui {
                            terminal: TuiCapabilities::compiled().expect("TUI capabilities"),
                        }],
                    )
                    .expect("worker inventory");
                    let descriptor = RemoteWorkerDescriptor::new(
                        RemoteWorkerIdentity {
                            instance_id: server_instance,
                            image_digest: IMAGE_DIGEST.to_string(),
                        },
                        inventory,
                        Default::default(),
                    )
                    .expect("worker descriptor");
                    let service = RemoteWorkerService::open(
                        RemoteWorkerServiceConfig::new(server_state_root, descriptor),
                        Arc::new(FixtureExecutor { fixed, blocking }),
                    )
                    .await
                    .expect("mock worker service");
                    let state = HttpState {
                        service: service.clone(),
                        authorization: server_authorization,
                        cancel_seen: server_cancel_seen,
                        tamper_status_binding,
                    };
                    let app = Router::new()
                        .route("/v1/worker", post(worker_request))
                        .route("/v1/artifacts", post(artifact_request))
                        .with_state(state);
                    let listener = TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("mock worker listener");
                    ready_tx
                        .send((
                            listener.local_addr().expect("mock worker address"),
                            service.clone(),
                        ))
                        .expect("publish mock worker address");
                    let graceful = server_shutdown.clone();
                    axum::serve(listener, app)
                        .with_graceful_shutdown(async move { graceful.cancelled().await })
                        .await
                        .expect("mock worker HTTP server");
                    service.shutdown().await.expect("mock worker shutdown");
                });
            });
            let (address, service) = ready_rx.recv().expect("mock worker readiness");
            Self {
                endpoint: format!("http://{address}"),
                authorization_env,
                authorization,
                instance_id: instance_id.to_string(),
                service,
                state_root,
                cancel_seen,
                shutdown,
                thread: Some(worker_thread),
            }
        }

        fn config_block(&self) -> String {
            format!(
                r#"  worker "{}" {{
    endpoint = "{}"
    image_digest = "{}"
    authorization_env = "{}"
    max_parallel_scenarios = 1
  }}
"#,
                self.instance_id, self.endpoint, IMAGE_DIGEST, self.authorization_env
            )
        }

        fn add_authorization(&self, command: &mut Command) {
            command.env(&self.authorization_env, &self.authorization);
        }

        fn shutdown(mut self) {
            self.shutdown.cancel();
            self.thread
                .take()
                .expect("owned mock worker")
                .join()
                .expect("join mock worker");
        }
    }

    impl Drop for MockWorker {
        fn drop(&mut self) {
            self.shutdown.cancel();
            if let Some(worker) = self.thread.take() {
                let _ = worker.join();
            }
        }
    }

    struct FixtureExecutor {
        fixed: Arc<AtomicBool>,
        blocking: bool,
    }

    #[async_trait]
    impl RemoteJobExecutor for FixtureExecutor {
        async fn execute(
            &self,
            job: RemoteExecutionJob,
            cancellation: CancellationToken,
        ) -> Result<RemoteExecutionResult, RemoteWorkerError> {
            if self.blocking {
                cancellation.cancelled().await;
                return Err(RemoteWorkerError::new(
                    "test.fixture.cancelled",
                    "blocking fixture observed cancellation",
                    false,
                ));
            }
            let fixed = self.fixed.load(Ordering::SeqCst);
            let scenarios = job
                .scenario_ids()
                .iter()
                .map(|id| fixture_scenario(id, fixed))
                .collect::<Vec<_>>();
            let status = if scenarios
                .iter()
                .all(|scenario| scenario.status == RunStatus::Passed)
            {
                RunStatus::Passed
            } else {
                RunStatus::Failed
            };
            let result = RunResult {
                run_id: format!("remote-{}", job.job_id()),
                suite: SUITE.to_string(),
                status,
                scenarios,
            };
            let mut counts = RemoteScenarioCounts {
                passed: 0,
                failed: 0,
                timed_out: 0,
                cancelled: 0,
            };
            for scenario in &result.scenarios {
                match scenario.status {
                    RunStatus::Passed => counts.passed += 1,
                    RunStatus::Failed => counts.failed += 1,
                    RunStatus::TimedOut => counts.timed_out += 1,
                    RunStatus::Cancelled => counts.cancelled += 1,
                }
            }
            Ok(RemoteExecutionResult {
                run_id: result.run_id.clone(),
                suite: result.suite.clone(),
                status: if status == RunStatus::Passed {
                    RemoteJobState::Passed
                } else {
                    RemoteJobState::Failed
                },
                scenarios: counts,
                report: serde_json::to_vec(&result).expect("fixture report"),
                media_type: "application/vnd.a3s-test.run-result+json".to_string(),
            })
        }
    }

    fn fixture_scenario(id: &str, fixed: bool) -> ScenarioResult {
        let failed = id == "known" && !fixed;
        ScenarioResult {
            id: id.to_string(),
            name: id.to_string(),
            surface: Surface::Tui,
            status: if failed {
                RunStatus::Failed
            } else {
                RunStatus::Passed
            },
            duration_ms: if id == "known" { 80 } else { 40 },
            steps: failed
                .then(|| StepResult {
                    id: "assert".to_string(),
                    status: RunStatus::Failed,
                    duration_ms: 10,
                    attempts: 1,
                    output: None,
                    error: Some(RunError {
                        code: "test.assert.text_visible".to_string(),
                        message: "known fixture is not fixed".to_string(),
                    }),
                })
                .into_iter()
                .collect(),
            error: None,
            cleanup_error: None,
        }
    }

    async fn worker_request(
        State(state): State<HttpState>,
        headers: HeaderMap,
        Json(request): Json<a3s_test_worker::RemoteWorkerRequest>,
    ) -> Response {
        if !authorized(&headers, &state.authorization) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        let is_cancel = matches!(
            &request.command,
            a3s_test_worker::RemoteWorkerCommand::Cancel { .. }
        );
        let tamper_status = state.tamper_status_binding
            && matches!(
                &request.command,
                a3s_test_worker::RemoteWorkerCommand::Status { .. }
            );
        if is_cancel {
            state.cancel_seen.store(true, Ordering::SeqCst);
        }
        let mut response = state.service.handle(request).await;
        if tamper_status {
            if let a3s_test_worker::RemoteWorkerOutcome::Job { job } = &mut response.outcome {
                job.request_digest = IMAGE_DIGEST.to_string();
            }
        }
        Json(response).into_response()
    }

    async fn artifact_request(
        State(state): State<HttpState>,
        headers: HeaderMap,
        Json(request): Json<a3s_test_worker::RemoteArtifactRequest>,
    ) -> Response {
        if !authorized(&headers, &state.authorization) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        Json(state.service.handle_artifact(request).await).into_response()
    }

    fn authorized(headers: &HeaderMap, expected: &str) -> bool {
        headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            == Some(expected)
    }

    #[test]
    fn distributed_run_shards_exactly_and_compares_a_second_verified_run() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fixed = Arc::new(AtomicBool::new(false));
        let first_worker = MockWorker::start(temp.path(), "runner_a", Arc::clone(&fixed), false);
        let second_worker = MockWorker::start(temp.path(), "runner_b", Arc::clone(&fixed), false);
        std::fs::write(
            temp.path().join("suite.acl"),
            r#"suite "distributed-tui" {
  scenario "alpha" { surface = "tui" expect "ready" { text = "ready" } }
  scenario "beta" { surface = "tui" expect "ready" { text = "ready" } }
  scenario "known" { surface = "tui" expect "fixed" { text = "fixed" } }
}
"#,
        )
        .expect("suite");
        let config = format!(
            r#"distributed_run "integration" {{
  manifest = "suite.acl"
  history_root = "history"
  history_window = 10
  job_timeout_ms = 20000
  lease_ms = 5000
  poll_interval_ms = 50
  http_timeout_ms = 5000
{}{}  quarantine "known" {{
    reason = "Fixture is unresolved during the first run"
    owner = "test-team"
    issue = "https://issues.example.test/known"
    expires_at_ms = {}
  }}
}}
"#,
            first_worker.config_block(),
            second_worker.config_block(),
            unix_ms() + 120_000,
        );
        let config_path = temp.path().join("distributed.acl");
        std::fs::write(&config_path, config).expect("distributed config");

        let plan = distributed_command(&config_path, &first_worker, &second_worker)
            .args(["distributed", "plan"])
            .arg(&config_path)
            .arg("--compact")
            .output()
            .expect("run distributed plan");
        assert!(plan.status.success(), "{plan:?}");
        let plan: serde_json::Value =
            serde_json::from_slice(&plan.stdout).expect("distributed plan JSON");
        assert_eq!(plan["shards"].as_array().map(Vec::len), Some(2));
        let planned = plan["shards"]
            .as_array()
            .expect("plan shards")
            .iter()
            .flat_map(|shard| {
                shard["scenario_ids"]
                    .as_array()
                    .expect("scenario IDs")
                    .iter()
                    .map(|id| id.as_str().expect("scenario ID").to_string())
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            planned,
            ["alpha", "beta", "known"]
                .map(str::to_string)
                .into_iter()
                .collect()
        );

        let first = run_distributed(&config_path, &first_worker, &second_worker);
        assert!(first.status.success(), "{first:?}");
        let first: serde_json::Value =
            serde_json::from_slice(&first.stdout).expect("first distributed report");
        assert_eq!(first["status"], "passed");
        assert_eq!(first["counts"]["passed"], 2);
        assert_eq!(first["counts"]["quarantined_failed"], 1);
        assert_eq!(first["shard_issues"].as_array().map(Vec::len), Some(0));
        let first_run_id = first["run_id"].as_str().expect("first run ID").to_string();

        fixed.store(true, Ordering::SeqCst);
        let second = run_distributed(&config_path, &first_worker, &second_worker);
        assert!(second.status.success(), "{second:?}");
        let second: serde_json::Value =
            serde_json::from_slice(&second.stdout).expect("second distributed report");
        assert_eq!(second["status"], "passed");
        assert_eq!(second["baseline_run_id"], first_run_id);
        assert_eq!(second["counts"]["passed"], 2);
        assert_eq!(second["counts"]["quarantined_passed"], 1);
        let known = second["scenarios"]
            .as_array()
            .expect("scenario analyses")
            .iter()
            .find(|scenario| scenario["id"] == "known")
            .expect("known scenario");
        assert_eq!(known["change"], "fixed");
        assert_eq!(known["flake"]["flaky"], true);
        assert!(temp
            .path()
            .join(format!(
                "history/reports/{}.json",
                second["run_id"].as_str().expect("second run ID")
            ))
            .is_file());

        first_worker.shutdown();
        second_worker.shutdown();
    }

    #[test]
    fn interrupt_cancels_the_exact_remote_job_and_returns_130() {
        let temp = tempfile::tempdir().expect("tempdir");
        let worker = MockWorker::start(
            temp.path(),
            "runner_cancel",
            Arc::new(AtomicBool::new(false)),
            true,
        );
        std::fs::write(
            temp.path().join("suite.acl"),
            r#"suite "distributed-tui" {
  scenario "slow" { surface = "tui" expect "ready" { text = "ready" } }
}
"#,
        )
        .expect("suite");
        let config_path = temp.path().join("distributed.acl");
        std::fs::write(
            &config_path,
            format!(
                r#"distributed_run "interrupt" {{
  manifest = "suite.acl"
  history_root = "interrupt-history"
  job_timeout_ms = 20000
  lease_ms = 5000
  poll_interval_ms = 50
  http_timeout_ms = 5000
{}}}
"#,
                worker.config_block()
            ),
        )
        .expect("distributed config");
        let mut command = Command::new(binary());
        command
            .current_dir(temp.path())
            .args(["distributed", "run"])
            .arg(&config_path)
            .arg("--json");
        worker.add_authorization(&mut command);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("report query runtime");
        let child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("start cancellable distributed run");
        let process_id = child.id();
        let job_root = worker.state_root.join("jobs");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let admitted_job = std::fs::read_dir(&job_root)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .find_map(|job_id| {
                    job_id.strip_prefix("job-").and_then(|suffix| {
                        let dispatch_id = format!("dispatch-{suffix}");
                        runtime
                            .block_on(worker.service.status(&job_id, &dispatch_id))
                            .ok()
                    })
                });
            if admitted_job.is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "remote job was not submitted");
            thread::sleep(Duration::from_millis(20));
        }
        assert!(Command::new("kill")
            .args(["-INT", &process_id.to_string()])
            .status()
            .expect("interrupt distributed run")
            .success());
        let (wait_tx, wait_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = wait_tx.send(child.wait_with_output());
        });
        let output = match wait_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(output) => output.expect("wait distributed run"),
            Err(_) => {
                let _ = Command::new("kill")
                    .args(["-KILL", &process_id.to_string()])
                    .status();
                panic!("distributed cancellation timed out");
            }
        };
        assert_eq!(output.status.code(), Some(130), "{output:?}");
        let analysis: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("cancelled analysis JSON");
        assert_eq!(analysis["status"], "cancelled");
        assert_eq!(analysis["counts"]["cancelled"], 1);

        let report_deadline = Instant::now() + Duration::from_secs(5);
        let reports = loop {
            let reports = runtime
                .block_on(worker.service.list_reports(RemoteReportQuery {
                    states: vec![RemoteJobState::Cancelled],
                    suite: None,
                    run_id: None,
                    finished_after_ms: None,
                    finished_before_ms: None,
                    limit: 10,
                    cursor: None,
                }))
                .expect("cancelled report index");
            if !reports.reports.is_empty() {
                break reports;
            }
            assert!(
                Instant::now() < report_deadline,
                "remote cancellation was not retained"
            );
            thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(reports.reports.len(), 1);
        assert_eq!(reports.reports[0].job.state, RemoteJobState::Cancelled);
        assert!(reports.reports[0]
            .job
            .job_id
            .starts_with("job-run-interrupt"));

        worker.shutdown();
    }

    #[test]
    fn mismatched_status_binding_stops_renewal_and_cancels_the_exact_remote_job() {
        let temp = tempfile::tempdir().expect("tempdir");
        let worker = MockWorker::start_with_tampered_status(
            temp.path(),
            "runner_tampered",
            Arc::new(AtomicBool::new(false)),
            true,
        );
        std::fs::write(
            temp.path().join("suite.acl"),
            r#"suite "distributed-tui" {
  scenario "slow" { surface = "tui" expect "ready" { text = "ready" } }
}
"#,
        )
        .expect("suite");
        let config_path = temp.path().join("distributed.acl");
        std::fs::write(
            &config_path,
            format!(
                r#"distributed_run "tampered" {{
  manifest = "suite.acl"
  history_root = "tampered-history"
  job_timeout_ms = 20000
  lease_ms = 5000
  poll_interval_ms = 50
  http_timeout_ms = 5000
{}}}
"#,
                worker.config_block()
            ),
        )
        .expect("distributed config");
        let mut command = Command::new(binary());
        command
            .current_dir(temp.path())
            .args(["distributed", "run"])
            .arg(&config_path)
            .arg("--json");
        worker.add_authorization(&mut command);
        let output = command.output().expect("run tampered distributed suite");
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let analysis: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("infrastructure analysis JSON");
        assert_eq!(analysis["status"], "infrastructure_failed");
        assert_eq!(analysis["counts"]["infrastructure_failed"], 1);
        assert_eq!(analysis["shard_issues"].as_array().map(Vec::len), Some(1));
        assert!(worker.cancel_seen.load(Ordering::SeqCst));

        worker.shutdown();
    }

    fn distributed_command(config_path: &Path, first: &MockWorker, second: &MockWorker) -> Command {
        let mut command = Command::new(binary());
        command.current_dir(config_path.parent().expect("config parent"));
        first.add_authorization(&mut command);
        second.add_authorization(&mut command);
        command
    }

    fn run_distributed(
        config_path: &Path,
        first: &MockWorker,
        second: &MockWorker,
    ) -> std::process::Output {
        distributed_command(config_path, first, second)
            .args(["distributed", "run"])
            .arg(config_path)
            .arg("--json")
            .output()
            .expect("run distributed suite")
    }

    fn unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Unix time")
            .as_millis()
            .try_into()
            .expect("millisecond timestamp")
    }
}
