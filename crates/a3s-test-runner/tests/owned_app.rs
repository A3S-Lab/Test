//! Owned-app admission and process-group reaping.
//!
//! The scripted host counts ready, spawn, and reap calls. The Unix host
//! starts a real process group and proves that group is gone afterwards.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, NamedViewport, OwnedApp, ScenarioContext, StepOutput,
    Surface, SurfaceDriver, TestScenario, TestStep, TestSuite,
};
#[cfg(unix)]
use a3s_test_runner::UnixAppProcess;
use a3s_test_runner::{
    start_owned_app, AppHandle, AppProcess, AppStop, RetryPolicy, RunStatus, Runner, RunnerOptions,
    StartedApp,
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

const SECRET_URL: &str = "http://user:s3cret-token@127.0.0.1:9/private";

#[derive(Clone, Copy)]
enum Behavior {
    Pass,
    Fail,
    Hang,
}

struct FakeDriver {
    opens: Arc<AtomicUsize>,
    behavior: Behavior,
}

struct FakeSession {
    behavior: Behavior,
}

#[async_trait]
impl SurfaceDriver for FakeDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeSession {
            behavior: self.behavior,
        }))
    }
}

#[async_trait]
impl DriverSession for FakeSession {
    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        match self.behavior {
            Behavior::Pass => Ok(StepOutput::new("ok")),
            Behavior::Fail => Err(DriverError::new("fake.failure", "planned failure")),
            Behavior::Hang => std::future::pending().await,
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

struct ScriptedProcess {
    answering: bool,
    fail_reap: bool,
    ready_calls: AtomicUsize,
    spawn_calls: AtomicUsize,
    commands: Mutex<Vec<Vec<String>>>,
    reaps: Mutex<Vec<AppStop>>,
    reap_nows: AtomicUsize,
}

impl ScriptedProcess {
    fn new(answering: bool) -> Arc<Self> {
        Self::host(answering, false)
    }

    fn failing_reap(answering: bool) -> Arc<Self> {
        Self::host(answering, true)
    }

    fn host(answering: bool, fail_reap: bool) -> Arc<Self> {
        Arc::new(Self {
            answering,
            fail_reap,
            ready_calls: AtomicUsize::new(0),
            spawn_calls: AtomicUsize::new(0),
            commands: Mutex::new(Vec::new()),
            reaps: Mutex::new(Vec::new()),
            reap_nows: AtomicUsize::new(0),
        })
    }

    fn ready_calls(&self) -> usize {
        self.ready_calls.load(Ordering::SeqCst)
    }

    fn spawn_calls(&self) -> usize {
        self.spawn_calls.load(Ordering::SeqCst)
    }

    fn commands(&self) -> Vec<Vec<String>> {
        self.commands.lock().expect("commands").clone()
    }

    fn reaps(&self) -> Vec<AppStop> {
        self.reaps.lock().expect("reaps").clone()
    }

    fn reap_nows(&self) -> usize {
        self.reap_nows.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AppProcess for ScriptedProcess {
    async fn ready(&self, _url: &str) -> Result<bool, DriverError> {
        self.ready_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.answering)
    }

    async fn spawn(&self, command: &[String]) -> Result<AppHandle, DriverError> {
        self.spawn_calls.fetch_add(1, Ordering::SeqCst);
        self.commands
            .lock()
            .expect("commands")
            .push(command.to_vec());
        Ok(AppHandle { pid: 4242 })
    }

    async fn reap(&self, _handle: AppHandle, stop: AppStop) -> Result<(), DriverError> {
        self.reaps.lock().expect("reaps").push(stop);
        if self.fail_reap {
            return Err(DriverError::new(
                "test.run.app_reap_failed",
                "planned reap failure",
            ));
        }
        Ok(())
    }

    fn reap_now(&self, _handle: &AppHandle) {
        self.reap_nows.fetch_add(1, Ordering::SeqCst);
    }
}

fn app(command: Vec<&str>, ready_url: &str, reuse: bool) -> OwnedApp {
    OwnedApp {
        command: command.into_iter().map(str::to_string).collect(),
        ready_url: ready_url.to_string(),
        reuse,
    }
}

fn suite(app: OwnedApp, timeout_ms: u64, scenarios: usize) -> TestSuite {
    TestSuite {
        name: "owned-app".to_string(),
        version: 1,
        scenarios: (0..scenarios)
            .map(|index| TestScenario {
                id: format!("scenario-{index}"),
                name: format!("Scenario {index}"),
                surface: Surface::Web,
                timeout_ms,
                steps: vec![TestStep {
                    id: "step".to_string(),
                    action: Action::Snapshot { interactive: true },
                    stability: None,
                    assertion_mode: Default::default(),
                    wait_mode: Default::default(),
                }],
            })
            .collect(),
        app: Some(app),
        targets: Vec::new(),
    }
}

fn assert_not_started(suite_result: &a3s_test_runner::RunResult, code: &str) {
    assert_eq!(suite_result.status, RunStatus::Failed);
    assert!(!suite_result.scenarios.is_empty());
    for scenario in &suite_result.scenarios {
        assert!(
            scenario.steps.is_empty(),
            "a refused app must not run steps"
        );
        assert!(scenario.target.is_none());
        let error = scenario.error.as_ref().expect("startup error");
        assert_eq!(error.code, code);
        assert!(!error.message.contains("s3cret-token"));
        assert!(!error.message.contains("secret-path"));
        assert!(!error.message.contains(SECRET_URL));
    }
}

async fn run_result(
    process: Arc<ScriptedProcess>,
    behavior: Behavior,
    ci: bool,
    suite: &TestSuite,
    cancellation: CancellationToken,
) -> (a3s_test_runner::RunResult, usize) {
    let opens = Arc::new(AtomicUsize::new(0));
    let runner = Runner::new(
        vec![Arc::new(FakeDriver {
            opens: Arc::clone(&opens),
            behavior,
        })],
        RunnerOptions {
            cleanup_timeout: Duration::from_secs(1),
            retry_policy: RetryPolicy {
                max_retries: 0,
                backoff: Duration::from_millis(1),
            },
            ..RunnerOptions::default()
        },
    )
    .expect("runner")
    .with_app_process(process)
    .with_ci(ci);
    let result = runner.run(suite, cancellation).await;
    (result, opens.load(Ordering::SeqCst))
}

#[tokio::test]
async fn empty_command_fails_before_ready_or_spawn() {
    for command in [Vec::new(), vec!["/bin/app", ""]] {
        let process = ScriptedProcess::new(false);
        let owned = app(command, "http://127.0.0.1:9/", false);
        let (result, opens) = run_result(
            Arc::clone(&process),
            Behavior::Pass,
            false,
            &suite(owned, 1_000, 1),
            CancellationToken::new(),
        )
        .await;
        assert_eq!(opens, 0);
        assert_eq!(process.ready_calls(), 0);
        assert_eq!(process.spawn_calls(), 0);
        assert!(process.reaps().is_empty());
        assert_eq!(process.reap_nows(), 0);
        assert_not_started(&result, "test.run.app_command");
    }
}

#[tokio::test]
async fn illegal_ready_endpoint_fails_before_any_probe() {
    let cases = [
        ("ftp://127.0.0.1:9/secret-path", "test.run.app_ready_url"),
        ("http://127.0.0.1:0/hidden", "test.run.app_port"),
    ];
    for (ready_url, code) in cases {
        let process = ScriptedProcess::new(true);
        let (result, opens) = run_result(
            Arc::clone(&process),
            Behavior::Pass,
            false,
            &suite(app(vec!["/bin/app"], ready_url, false), 1_000, 2),
            CancellationToken::new(),
        )
        .await;
        assert_eq!(opens, 0);
        assert_eq!(process.ready_calls(), 0);
        assert_eq!(process.spawn_calls(), 0);
        assert_eq!(result.scenarios.len(), 2);
        assert_not_started(&result, code);
    }
}

#[tokio::test]
async fn ci_refuses_reuse_before_the_ready_probe() {
    let process = ScriptedProcess::new(true);
    let (result, opens) = run_result(
        Arc::clone(&process),
        Behavior::Pass,
        true,
        &suite(app(vec!["/bin/app"], SECRET_URL, true), 1_000, 2),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(opens, 0);
    assert_eq!(process.ready_calls(), 0);
    assert_eq!(process.spawn_calls(), 0);
    assert!(process.reaps().is_empty());
    assert_not_started(&result, "test.run.app_reuse_denied");
    assert_eq!(
        result.scenarios[0].error.as_ref().expect("error").message,
        "continuous integration refuses to reuse an owned app"
    );
}

#[tokio::test]
async fn answering_endpoint_without_reuse_does_not_open_or_spawn() {
    let process = ScriptedProcess::new(true);
    let (result, opens) = run_result(
        Arc::clone(&process),
        Behavior::Pass,
        false,
        &suite(
            app(vec!["/bin/app", "--port", "9"], SECRET_URL, false),
            1_000,
            2,
        ),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(opens, 0);
    assert_eq!(process.ready_calls(), 1);
    assert_eq!(process.spawn_calls(), 0);
    assert!(process.commands().is_empty());
    assert!(process.reaps().is_empty());
    assert_eq!(process.reap_nows(), 0);
    assert_eq!(result.scenarios.len(), 2);
    assert_not_started(&result, "test.run.app_already_running");
    assert_eq!(
        result.scenarios[0].error.as_ref().expect("error").message,
        "owned app ready endpoint already answers"
    );
}

#[tokio::test]
async fn reuse_attaches_without_spawn_and_does_not_reap() {
    let process = ScriptedProcess::new(true);
    let (result, opens) = run_result(
        Arc::clone(&process),
        Behavior::Pass,
        false,
        &suite(app(vec!["/bin/app"], "http://127.0.0.1:9/", true), 1_000, 1),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(opens, 1);
    assert_eq!(process.ready_calls(), 1);
    assert_eq!(process.spawn_calls(), 0);
    assert!(process.reaps().is_empty());
    assert_eq!(process.reap_nows(), 0);
    assert!(result.scenarios[0].target.is_none());
    assert_eq!(result.scenarios[0].steps[0].id, "step");
}

#[tokio::test]
async fn spawned_argument_list_is_reaped_on_finish_cancel_and_timeout() {
    let command = vec!["/bin/app", "--ready", "http://127.0.0.1:9/"];
    let owned = app(command.clone(), "http://127.0.0.1:9/", false);

    let finished = ScriptedProcess::new(false);
    let (result, opens) = run_result(
        Arc::clone(&finished),
        Behavior::Pass,
        false,
        &suite(owned.clone(), 1_000, 1),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(opens, 1);
    assert_eq!(finished.spawn_calls(), 1);
    assert_eq!(
        finished.commands(),
        vec![command
            .iter()
            .map(|part| (*part).to_string())
            .collect::<Vec<_>>()]
    );
    assert_eq!(finished.reaps(), vec![AppStop::Finish]);
    assert_eq!(finished.reap_nows(), 0);

    let failed = ScriptedProcess::new(false);
    let (result, _) = run_result(
        Arc::clone(&failed),
        Behavior::Fail,
        false,
        &suite(owned.clone(), 1_000, 1),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(failed.reaps(), vec![AppStop::Finish]);
    assert_eq!(failed.reap_nows(), 0);

    let cancelled = ScriptedProcess::new(false);
    let token = CancellationToken::new();
    token.cancel();
    let (result, opens) = run_result(
        Arc::clone(&cancelled),
        Behavior::Hang,
        false,
        &suite(owned.clone(), 1_000, 1),
        token,
    )
    .await;
    assert_eq!(result.status, RunStatus::Cancelled);
    assert_eq!(opens, 0, "a cancelled run must not open the surface");
    assert_eq!(cancelled.spawn_calls(), 1);
    assert_eq!(cancelled.reaps(), vec![AppStop::Cancel]);

    let timed_out = ScriptedProcess::new(false);
    let (result, opens) = run_result(
        Arc::clone(&timed_out),
        Behavior::Hang,
        false,
        &suite(owned, 200, 1),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(result.status, RunStatus::TimedOut);
    assert_eq!(opens, 1);
    assert_eq!(timed_out.reaps(), vec![AppStop::Timeout]);
    assert_eq!(timed_out.reap_nows(), 0);
}

#[tokio::test]
async fn reap_failure_uses_the_emergency_path() {
    let process = ScriptedProcess::failing_reap(false);
    let (result, opens) = run_result(
        Arc::clone(&process),
        Behavior::Pass,
        false,
        &suite(
            app(vec!["/bin/app"], "http://127.0.0.1:9/", false),
            1_000,
            1,
        ),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(opens, 1);
    assert_eq!(process.reaps(), vec![AppStop::Finish]);
    assert_eq!(process.reap_nows(), 1);
}

#[tokio::test]
async fn direct_admission_matches_the_runner_codes() {
    let host = ScriptedProcess::new(true);
    let empty = start_owned_app(
        &app(vec![], "http://127.0.0.1:9/", false),
        host.as_ref(),
        false,
    )
    .await
    .expect_err("empty command");
    assert_eq!(empty.code(), "test.run.app_command");
    assert_eq!(host.ready_calls(), 0);

    let reused = start_owned_app(
        &app(vec!["/bin/app"], SECRET_URL, true),
        host.as_ref(),
        true,
    )
    .await
    .expect_err("ci reuse");
    assert_eq!(reused.code(), "test.run.app_reuse_denied");
    assert!(!reused.message().contains("s3cret-token"));
    assert_eq!(host.ready_calls(), 0);

    let attached = start_owned_app(
        &app(vec!["/bin/app"], "http://127.0.0.1:9/", true),
        host.as_ref(),
        false,
    )
    .await
    .expect("attach");
    assert_eq!(
        attached,
        StartedApp {
            handle: None,
            spawned: false,
        }
    );
    assert_eq!(host.spawn_calls(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn reaps_the_owned_process_group() {
    let host = UnixAppProcess::new();
    let started = start_owned_app(
        &OwnedApp {
            command: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 30 & wait".to_string(),
            ],
            ready_url: "http://127.0.0.1:9/".to_string(),
            reuse: false,
        },
        &host,
        false,
    )
    .await
    .expect("spawn");
    assert!(started.spawned);
    let pid = started.handle.expect("process id").pid;
    assert_ne!(pid, 0);

    let mut group = String::new();
    for _ in 0..40 {
        group = process_group(pid).await;
        if group.lines().filter(|line| !line.trim().is_empty()).count() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        group.lines().filter(|line| !line.trim().is_empty()).count() >= 2,
        "sleep was not in the owned process group: {group}"
    );

    host.reap(AppHandle { pid }, AppStop::Finish)
        .await
        .expect("reap");

    let mut after = String::new();
    for _ in 0..20 {
        after = process_group(pid).await;
        if after.trim().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        after.trim().is_empty(),
        "owned process group was still alive: {after}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn reused_ready_endpoint_leaves_the_existing_process_running() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let port = listener.local_addr().expect("port").port();
    let mut child = tokio::process::Command::new("/bin/sleep")
        .arg("45")
        .kill_on_drop(true)
        .spawn()
        .expect("sleep");
    let pid = child.id().expect("sleep pid");
    let host = UnixAppProcess::new();
    let started = start_owned_app(
        &OwnedApp {
            command: vec!["/bin/sleep".to_string(), "45".to_string()],
            ready_url: format!("http://127.0.0.1:{port}/"),
            reuse: true,
        },
        &host,
        false,
    )
    .await
    .expect("reuse");
    assert_eq!(
        started,
        StartedApp {
            handle: None,
            spawned: false,
        }
    );
    drop(host);
    drop(listener);
    let still = process_row(pid).await;
    assert!(
        still.contains(&pid.to_string()),
        "reused process was killed: {still}"
    );
    child.start_kill().expect("stop sleep");
    let _ = child.wait().await;
}

#[cfg(unix)]
async fn process_group(pid: u32) -> String {
    let output = tokio::process::Command::new("ps")
        .args(["-o", "pid=", "-g", &pid.to_string()])
        .output()
        .await
        .expect("ps");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[cfg(unix)]
async fn process_row(pid: u32) -> String {
    let output = tokio::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "pid="])
        .output()
        .await
        .expect("ps");
    String::from_utf8_lossy(&output.stdout).to_string()
}

type RecordedOpens = Arc<Mutex<Vec<(PathBuf, Vec<Action>)>>>;

struct TracingDriver {
    opens: RecordedOpens,
}

struct TracingSession {
    opens: RecordedOpens,
    index: usize,
}

#[async_trait]
impl SurfaceDriver for TracingDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError> {
        let index = {
            let mut opens = self.opens.lock().expect("opens");
            opens.push((context.artifacts_dir.clone(), Vec::new()));
            opens.len() - 1
        };
        Ok(Box::new(TracingSession {
            opens: Arc::clone(&self.opens),
            index,
        }))
    }
}

#[async_trait]
impl DriverSession for TracingSession {
    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.opens.lock().expect("opens")[self.index]
            .1
            .push(step.action.clone());
        Ok(StepOutput::new("ok"))
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

fn viewport_suite(targets: Vec<NamedViewport>) -> TestSuite {
    TestSuite {
        name: "viewports".to_string(),
        version: 1,
        scenarios: vec![TestScenario {
            id: "scenario".to_string(),
            name: "Scenario".to_string(),
            surface: Surface::Web,
            timeout_ms: 1_000,
            steps: vec![TestStep {
                id: "step".to_string(),
                action: Action::Snapshot { interactive: true },
                stability: None,
                assertion_mode: Default::default(),
                wait_mode: Default::default(),
            }],
        }],
        app: None,
        targets,
    }
}

fn evidence_root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "a3s-test-viewports-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).expect("evidence root");
    path
}

async fn run_viewports(
    targets: Vec<NamedViewport>,
) -> (
    a3s_test_runner::RunResult,
    Vec<(PathBuf, Vec<Action>)>,
    PathBuf,
) {
    let root = evidence_root();
    let opens = Arc::new(Mutex::new(Vec::new()));
    let runner = Runner::new(
        vec![Arc::new(TracingDriver {
            opens: Arc::clone(&opens),
        })],
        RunnerOptions {
            cleanup_timeout: Duration::from_secs(1),
            ..RunnerOptions::default()
        },
    )
    .expect("runner")
    .with_artifacts_root(root.clone())
    .expect("artifacts");
    let result = runner
        .run(&viewport_suite(targets), CancellationToken::new())
        .await;
    let recorded = opens.lock().expect("opens").clone();
    (result, recorded, root)
}

#[tokio::test]
async fn default_suite_uses_one_viewport_and_one_evidence_directory() {
    let (result, recorded, root) = run_viewports(Vec::new()).await;
    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(result.scenarios.len(), 1);
    assert!(result.scenarios[0].target.is_none());
    assert_eq!(result.scenarios[0].steps.len(), 1);
    assert_eq!(result.scenarios[0].steps[0].id, "step");
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].0.ends_with("scenario"));
    assert_eq!(recorded[0].1, vec![Action::Snapshot { interactive: true }]);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn named_viewports_keep_separate_evidence_directories() {
    let (result, recorded, root) = run_viewports(vec![
        NamedViewport {
            name: "desktop".to_string(),
            width: 1280,
            height: 720,
            scale: None,
        },
        NamedViewport {
            name: "mobile".to_string(),
            width: 390,
            height: 844,
            scale: Some(2),
        },
    ])
    .await;
    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(result.scenarios.len(), 2);
    assert_eq!(result.scenarios[0].target.as_deref(), Some("desktop"));
    assert_eq!(result.scenarios[1].target.as_deref(), Some("mobile"));
    assert_eq!(result.scenarios[0].steps[0].id, "viewport");
    assert_eq!(result.scenarios[1].steps[0].id, "viewport");
    assert_eq!(recorded.len(), 2);
    assert!(recorded[0].0.ends_with("desktop"));
    assert!(recorded[1].0.ends_with("mobile"));
    assert_ne!(recorded[0].0, recorded[1].0);
    assert_eq!(
        recorded[0].1,
        vec![
            Action::Viewport {
                width: 1280,
                height: 720,
                scale: None,
            },
            Action::Snapshot { interactive: true },
        ]
    );
    assert_eq!(
        recorded[1].1,
        vec![
            Action::Viewport {
                width: 390,
                height: 844,
                scale: Some(2),
            },
            Action::Snapshot { interactive: true },
        ]
    );
    let _ = std::fs::remove_dir_all(root);
}
