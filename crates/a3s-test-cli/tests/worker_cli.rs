use std::path::PathBuf;
use std::process::Command;

use a3s_test_worker::{REMOTE_ARTIFACT_PROTOCOL, REMOTE_WORKER_PROTOCOL};

#[cfg(unix)]
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use a3s_test_worker::{
    RemoteArtifactCommand, RemoteArtifactRequest, RemoteArtifactSelector, RemoteInputBundle,
    RemoteInputFile, RemoteJobState, RemoteJobSubmission, RemoteReportQuery, RemoteWorkerCommand,
    RemoteWorkerDescriptor, RemoteWorkerRequest, WorkerSurface,
};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn worker_schema_exposes_scheduling_evidence_without_trust_authority() {
    let output = Command::new(binary())
        .args(["worker", "schema", "--compact"])
        .output()
        .expect("run worker schema");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("worker schema JSON");
    assert_eq!(value["protocol"], "a3s.test.worker-capabilities/1");
    assert_eq!(value["authority"], "scheduling_evidence");
    assert_eq!(value["invariants"]["self_reported"], true);
    assert_eq!(value["invariants"]["authenticated"], false);
    assert_eq!(value["invariants"]["authorizes_execution"], false);
    assert_eq!(
        value["invariants"]["external_image_identity_required"],
        true
    );
    assert_eq!(value["inventory_schema"]["additionalProperties"], false);
}

#[test]
fn worker_inventory_reports_the_compiled_tui_surface_by_default() {
    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--max-parallel-scenarios",
            "4",
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("worker inventory JSON");
    assert_eq!(value["protocol"], "a3s.test.worker-capabilities/1");
    assert_eq!(value["max_parallel_scenarios"], 4);
    assert_eq!(value["surfaces"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["surfaces"][0]["surface"], "tui");
    assert!(value["surfaces"][0]["terminal"]["backend"].is_string());
}

#[test]
fn worker_inventory_rejects_an_unbounded_parallelism_claim() {
    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--max-parallel-scenarios",
            "65",
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("between 1 and 64"),
        "{output:?}"
    );
}

#[cfg(unix)]
#[test]
fn worker_inventory_adds_web_only_after_an_explicit_successful_probe() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("fake-agent-browser");
    fs::write(&driver, "#!/bin/sh\nprintf 'agent-browser 0.26.0\\n'\n").expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().expect("driver path"),
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("worker inventory JSON");
    assert_eq!(value["surfaces"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["surfaces"][0]["surface"], "web");
    assert_eq!(value["surfaces"][0]["execution"], "headless");
    assert_eq!(value["surfaces"][0]["browser"]["integration"], "standalone");
    assert_eq!(value["surfaces"][1]["surface"], "tui");
}

#[cfg(unix)]
#[test]
fn worker_inventory_fails_closed_when_the_requested_web_probe_fails() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("broken-agent-browser");
    fs::write(
        &driver,
        "#!/bin/sh\nprintf 'probe unavailable\\n' >&2\nexit 7\n",
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().expect("driver path"),
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("browser version probe failed"),
        "{output:?}"
    );
}

#[test]
fn worker_inventory_does_not_infer_a_browser_backend_from_an_executable() {
    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--browser-executable",
            "agent-browser",
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--browser-driver"),
        "{output:?}"
    );
}

#[test]
fn remote_worker_schema_exposes_authenticated_digest_bound_execution() {
    let output = Command::new(binary())
        .args(["worker", "remote", "schema", "--compact"])
        .output()
        .expect("run remote worker schema");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("remote worker schema JSON");
    assert_eq!(value["protocol"], REMOTE_WORKER_PROTOCOL);
    assert_eq!(
        value["invariants"]["transport_authentication_required"],
        true
    );
    assert_eq!(value["invariants"]["tls_termination_external"], true);
    assert_eq!(
        value["invariants"]["request_cannot_select_executables"],
        true
    );
    assert_eq!(value["invariants"]["transports_artifacts"], false);
    assert_eq!(value["request_schema"]["additionalProperties"], false);
}

#[test]
fn remote_artifact_schema_exposes_bounded_digest_bound_transport() {
    let output = Command::new(binary())
        .args(["worker", "artifacts", "schema", "--compact"])
        .output()
        .expect("run remote artifact schema");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("remote artifact schema JSON");
    assert_eq!(value["protocol"], REMOTE_ARTIFACT_PROTOCOL);
    assert_eq!(
        value["invariants"]["transport_authentication_required"],
        true
    );
    assert_eq!(value["invariants"]["deployment_owned_retention"], true);
    assert_eq!(value["invariants"]["digest_bound_reads"], true);
    assert_eq!(value["invariants"]["bounded_pagination"], true);
    assert_eq!(value["invariants"]["bounded_chunks"], true);
    assert_eq!(value["invariants"]["no_arbitrary_paths"], true);
    assert_eq!(value["invariants"]["transports_artifacts"], true);
    assert_eq!(value["request_schema"]["additionalProperties"], false);
}

#[test]
fn remote_worker_serve_rejects_non_loopback_listeners() {
    let output = Command::new(binary())
        .args([
            "worker",
            "serve",
            "--listen",
            "0.0.0.0:0",
            "--instance-id",
            "worker-test",
            "--image-digest",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--authorization-env",
            "A3S_TEST_REMOTE_WORKER_AUTH",
            "--tui-executable",
            "/bin/sh",
        ])
        .output()
        .expect("run remote worker with unsafe listener");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("must bind a loopback address"),
        "{output:?}"
    );
}

#[test]
fn remote_worker_serve_rejects_unbounded_deadline_configuration_before_startup() {
    let output = Command::new(binary())
        .args([
            "worker",
            "serve",
            "--instance-id",
            "worker-test",
            "--image-digest",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--authorization-env",
            "A3S_TEST_REMOTE_WORKER_AUTH",
            "--tui-executable",
            "/bin/sh",
            "--command-timeout-ms",
            "300001",
        ])
        .output()
        .expect("run remote worker with unbounded command timeout");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot exceed 300000 ms"),
        "{output:?}"
    );
}

#[test]
fn remote_worker_serve_rejects_an_unordered_retention_policy_before_startup() {
    let output = Command::new(binary())
        .args([
            "worker",
            "serve",
            "--instance-id",
            "worker-test",
            "--image-digest",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--authorization-env",
            "A3S_TEST_REMOTE_WORKER_AUTH",
            "--tui-executable",
            "/bin/sh",
            "--retention-max-jobs",
            "2",
            "--report-index-max-jobs",
            "1",
        ])
        .output()
        .expect("run remote worker with unordered retention");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("retention limits"),
        "{output:?}"
    );
}

#[cfg(unix)]
#[test]
fn authenticated_remote_http_host_executes_a_real_tui_job() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let state_root = temp.path().join("worker-state");
    let browser = temp.path().join("fake-agent-browser");
    fs::write(
        &browser,
        "#!/bin/sh\n\
         test -z \"${A3S_TEST_REMOTE_WORKER_AUTH+x}\" || exit 97\n\
         printf 'agent-browser 0.26.0\\n'\n",
    )
    .expect("write browser probe");
    fs::set_permissions(&browser, fs::Permissions::from_mode(0o755))
        .expect("browser probe permissions");
    let authorization = "Bearer remote-test-secret";
    let script = "test -z \"${A3S_TEST_REMOTE_WORKER_AUTH+x}\" || exit 97; \
                  stty -echo; printf 'ready\\n'; IFS= read -r line; \
                  printf 'input:%s\\n' \"$line\"";
    let child = Command::new(binary())
        .args([
            "worker",
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--state-root",
            state_root.to_str().expect("state root"),
            "--instance-id",
            "worker-tui-e2e",
            "--image-digest",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--authorization-env",
            "A3S_TEST_REMOTE_WORKER_AUTH",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            browser.to_str().expect("browser path"),
            "--web-allow-origin",
            "https://example.test",
            "--tui-executable",
            "/bin/sh",
            "--tui-arg",
            "-c",
            "--tui-arg",
            script,
            "--cleanup-timeout-ms",
            "5000",
            "--compact",
        ])
        .env("A3S_TEST_REMOTE_WORKER_AUTH", authorization)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start remote worker host");
    let mut child = OwnedChild::new(child);
    let process_id = child.id();
    let stdout = child.stdout.take().expect("worker stdout");
    let mut stdout = BufReader::new(stdout);
    let mut ready_line = String::new();
    stdout
        .read_line(&mut ready_line)
        .expect("read worker readiness");
    assert!(
        !ready_line.is_empty(),
        "remote worker exited before readiness"
    );
    let ready: serde_json::Value =
        serde_json::from_str(&ready_line).expect("remote worker readiness JSON");
    let address = ready["listen"].as_str().expect("remote worker address");
    let descriptor: RemoteWorkerDescriptor =
        serde_json::from_value(ready["worker"].clone()).expect("worker descriptor");
    assert_eq!(ready["artifacts"]["protocol"], REMOTE_ARTIFACT_PROTOCOL);

    let inspect = RemoteWorkerRequest {
        protocol: REMOTE_WORKER_PROTOCOL.to_string(),
        request_id: "inspect-unauthorized".to_string(),
        command: RemoteWorkerCommand::Inspect,
    };
    let (status, error) = post_json(address, None, &inspect);
    assert_eq!(status, 401);
    assert_eq!(error["code"], "test.worker.remote.transport_unauthorized");
    let (status, error) = post_json(address, Some("Bearer wrong-secret"), &inspect);
    assert_eq!(status, 401);
    assert_eq!(error["code"], "test.worker.remote.transport_unauthorized");
    let (status, inspected) = post_json(address, Some(authorization), &inspect);
    assert_eq!(status, 200, "{inspected}");
    assert_eq!(inspected["outcome"]["type"], "descriptor");
    assert_eq!(
        inspected["outcome"]["worker"]["inventory_digest"],
        descriptor.inventory_digest.as_str()
    );

    let now_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Unix time")
        .as_millis()
        .try_into()
        .expect("millisecond timestamp");
    let manifest = br#"suite "remote-tui" {
    version = 1
    scenario "terminal" {
        surface = "tui"
        timeout_ms = 5000
        wait "ready" { text = "ready" }
        terminal_paste "input" { text = "hello" }
        press "submit" { key = "Enter" }
        wait "echoed" { regex = "input:hello" }
        terminal_recording "record" { path = "terminal/session.vt" }
    }
}
"#;
    let submission = RemoteJobSubmission {
        job_id: "job-tui-e2e".to_string(),
        dispatch_id: "dispatch-tui-e2e".to_string(),
        worker_instance: descriptor.identity.instance_id.clone(),
        required_image_digest: descriptor.identity.image_digest.clone(),
        required_inventory_digest: descriptor.inventory_digest.clone(),
        issued_at_ms: now_ms,
        deadline_ms: now_ms + 30_000,
        lease_expires_at_ms: now_ms + 20_000,
        max_parallel_scenarios: 1,
        required_surfaces: vec![WorkerSurface::Tui],
        input: RemoteInputBundle {
            manifest: "suite.acl".to_string(),
            files: vec![RemoteInputFile::from_bytes("suite.acl", manifest)],
        },
    };
    let submit = RemoteWorkerRequest {
        protocol: REMOTE_WORKER_PROTOCOL.to_string(),
        request_id: "submit-tui-e2e".to_string(),
        command: RemoteWorkerCommand::Submit { job: submission },
    };
    let (status, submitted) = post_json(address, Some(authorization), &submit);
    assert_eq!(status, 200, "{submitted}");
    assert_eq!(submitted["outcome"]["type"], "job");

    let deadline = Instant::now() + Duration::from_secs(10);
    let terminal = loop {
        assert!(Instant::now() < deadline, "remote TUI job did not finish");
        let request = RemoteWorkerRequest {
            protocol: REMOTE_WORKER_PROTOCOL.to_string(),
            request_id: "status-tui-e2e".to_string(),
            command: RemoteWorkerCommand::Status {
                job_id: "job-tui-e2e".to_string(),
                dispatch_id: "dispatch-tui-e2e".to_string(),
            },
        };
        let (status, response) = post_json(address, Some(authorization), &request);
        assert_eq!(status, 200, "{response}");
        let state: RemoteJobState =
            serde_json::from_value(response["outcome"]["job"]["state"].clone())
                .expect("remote job state");
        if state.terminal() {
            break response;
        }
        thread::yield_now();
    };
    assert_eq!(terminal["outcome"]["job"]["state"], "passed");
    assert_eq!(
        terminal["outcome"]["job"]["result"]["scenarios"]["passed"],
        1
    );
    let artifact_inspect = RemoteArtifactRequest {
        protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
        request_id: "artifact-inspect".to_string(),
        command: RemoteArtifactCommand::Inspect,
    };
    let (status, error) = post_json_at_path(address, "/v1/artifacts", None, &artifact_inspect);
    assert_eq!(status, 401);
    assert_eq!(error["code"], "test.worker.remote.transport_unauthorized");
    let (status, artifact_service) = post_json_at_path(
        address,
        "/v1/artifacts",
        Some(authorization),
        &artifact_inspect,
    );
    assert_eq!(status, 200, "{artifact_service}");
    assert_eq!(artifact_service["outcome"]["type"], "descriptor");
    assert_eq!(
        artifact_service["outcome"]["service"]["inventory_digest"],
        descriptor.inventory_digest
    );

    let list_reports = RemoteArtifactRequest {
        protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
        request_id: "artifact-reports".to_string(),
        command: RemoteArtifactCommand::ListReports {
            query: RemoteReportQuery {
                states: vec![RemoteJobState::Passed],
                suite: None,
                run_id: None,
                finished_after_ms: None,
                finished_before_ms: None,
                limit: 10,
                cursor: None,
            },
        },
    };
    let (status, reports) =
        post_json_at_path(address, "/v1/artifacts", Some(authorization), &list_reports);
    assert_eq!(status, 200, "{reports}");
    assert_eq!(reports["outcome"]["type"], "reports");
    assert_eq!(
        reports["outcome"]["page"]["reports"][0]["job"]["job_id"],
        "job-tui-e2e"
    );
    assert_eq!(
        reports["outcome"]["page"]["reports"][0]["payload_state"],
        "retained"
    );

    let request_digest = terminal["outcome"]["job"]["request_digest"]
        .as_str()
        .expect("remote request digest");
    let report_digest = terminal["outcome"]["job"]["result"]["report"]["sha256"]
        .as_str()
        .expect("remote report digest");
    let list_artifacts = RemoteArtifactRequest {
        protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
        request_id: "artifact-list".to_string(),
        command: RemoteArtifactCommand::ListArtifacts {
            job_id: "job-tui-e2e".to_string(),
            dispatch_id: "dispatch-tui-e2e".to_string(),
            expected_request_digest: request_digest.to_string(),
            limit: 10,
            cursor: None,
        },
    };
    let (status, artifacts) = post_json_at_path(
        address,
        "/v1/artifacts",
        Some(authorization),
        &list_artifacts,
    );
    assert_eq!(status, 200, "{artifacts}");
    assert_eq!(artifacts["outcome"]["type"], "artifacts");
    assert_eq!(
        artifacts["outcome"]["page"]["artifacts"][0]["kind"],
        "report"
    );
    assert!(artifacts["outcome"]["page"]["artifacts"]
        .as_array()
        .is_some_and(|artifacts| artifacts.len() >= 2));

    let read_report = RemoteArtifactRequest {
        protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
        request_id: "artifact-read".to_string(),
        command: RemoteArtifactCommand::Read {
            job_id: "job-tui-e2e".to_string(),
            dispatch_id: "dispatch-tui-e2e".to_string(),
            expected_request_digest: request_digest.to_string(),
            artifact: RemoteArtifactSelector::Report {
                sha256: report_digest.to_string(),
            },
            offset: 0,
            max_bytes: 1024 * 1024,
        },
    };
    let (status, report_chunk) =
        post_json_at_path(address, "/v1/artifacts", Some(authorization), &read_report);
    assert_eq!(status, 200, "{report_chunk}");
    assert_eq!(report_chunk["outcome"]["type"], "chunk");
    assert_eq!(report_chunk["outcome"]["chunk"]["eof"], true);
    assert!(report_chunk["outcome"]["chunk"]["contents_base64"]
        .as_str()
        .is_some_and(|contents| !contents.is_empty()));

    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(state_root.join("jobs/job-tui-e2e/report.bin"))
            .expect("persisted remote report"),
    )
    .expect("remote run report JSON");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["scenarios"][0]["surface"], "tui");
    let evidence = PathBuf::from(
        report["scenarios"][0]["steps"][4]["output"]["evidence"][0]["path"]
            .as_str()
            .expect("remote terminal evidence path"),
    );
    let artifact_root = std::fs::canonicalize(state_root.join("jobs/job-tui-e2e/artifacts"))
        .expect("canonical remote artifact root");
    assert!(
        evidence.starts_with(&artifact_root),
        "{}",
        evidence.display()
    );
    assert!(evidence.is_file(), "{}", evidence.display());

    let signal = Command::new("kill")
        .args(["-INT", &process_id.to_string()])
        .status()
        .expect("signal remote worker host");
    assert!(signal.success());
    let (wait_tx, wait_rx) = mpsc::channel();
    let child = child.take();
    thread::spawn(move || {
        let _ = wait_tx.send(child.wait_with_output());
    });
    let output = match wait_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(result) => result.expect("wait for remote worker host"),
        Err(_) => {
            let _ = Command::new("kill")
                .args(["-KILL", &process_id.to_string()])
                .status();
            wait_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("reap timed-out remote worker host")
                .expect("wait after forced remote worker cleanup")
        }
    };
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains(authorization),
        "authorization leaked: {stderr}"
    );
}

#[cfg(unix)]
struct OwnedChild {
    child: Option<Child>,
}

#[cfg(unix)]
impl OwnedChild {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn id(&self) -> u32 {
        self.child.as_ref().expect("owned child").id()
    }

    fn take(&mut self) -> Child {
        self.child.take().expect("owned child")
    }
}

#[cfg(unix)]
impl std::ops::Deref for OwnedChild {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        self.child.as_ref().expect("owned child")
    }
}

#[cfg(unix)]
impl std::ops::DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.child.as_mut().expect("owned child")
    }
}

#[cfg(unix)]
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(unix)]
fn post_json(
    address: &str,
    authorization: Option<&str>,
    value: &impl serde::Serialize,
) -> (u16, serde_json::Value) {
    post_json_at_path(address, "/v1/worker", authorization, value)
}

#[cfg(unix)]
fn post_json_at_path(
    address: &str,
    path: &str,
    authorization: Option<&str>,
    value: &impl serde::Serialize,
) -> (u16, serde_json::Value) {
    let body = serde_json::to_vec(value).expect("HTTP request JSON");
    let mut stream = TcpStream::connect(address).expect("connect to remote worker");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("HTTP read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("HTTP write timeout");
    let authorization = authorization
        .map(|value| format!("Authorization: {value}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\n{authorization}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write HTTP headers");
    stream.write_all(&body).expect("write HTTP body");
    stream.flush().expect("flush HTTP request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read HTTP response");
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP response separator");
    let headers = std::str::from_utf8(&response[..separator]).expect("HTTP response headers");
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .expect("HTTP response status");
    let value = serde_json::from_slice(&response[separator + 4..]).expect("HTTP response JSON");
    (status, value)
}
