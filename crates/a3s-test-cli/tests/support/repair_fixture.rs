use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

use super::browser_process::bounded_output;
use super::testkit_bundle::bundle_browser_fixture;
use super::web_fixture::{start_testkit_fixture, TestKitFixture};

pub fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

pub fn admitted_browser() -> Option<PathBuf> {
    let browser = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from)?;
    assert!(
        browser.is_file(),
        "browser executable does not exist: {browser:?}"
    );
    let version = Command::new(&browser)
        .arg("--version")
        .output()
        .expect("probe standalone browser version");
    assert_process_success("probe standalone browser version", &version);
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("0.26."),
        "real repair E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );
    Some(browser)
}

pub fn start_fixture() -> (TempDir, TestKitFixture) {
    let (bundle_workspace, bundle) = bundle_browser_fixture("bundle repair lifecycle fixture");
    let fixture = start_testkit_fixture(bundle).expect("start repair lifecycle fixture");
    (bundle_workspace, fixture)
}

pub struct RepairSession {
    workspace: TempDir,
    browser: PathBuf,
    session: String,
    state: Value,
    armed: bool,
}

pub struct Transition<'a> {
    pub command: &'a str,
    pub finding_id: &'a str,
    pub request_id: &'a str,
    pub attempt_id: Option<&'a str>,
    pub summary: &'a str,
    pub message: Option<&'a str>,
    pub lease_ms: Option<u64>,
}

impl RepairSession {
    pub fn start(browser: &Path, fixture: &TestKitFixture, session: &str) -> Self {
        let workspace = tempfile::tempdir().expect("temporary repair lifecycle workspace");
        let start = Command::new(binary())
            .args([
                "agent",
                "start",
                &fixture.origin(),
                "--session",
                session,
                "--goal",
                "Exercise the embedded review repair lifecycle",
                "--success",
                "Every requested repair state is backed by browser and ledger evidence",
                "--browser-driver",
                "standalone",
                "--browser-executable",
                browser.to_str().expect("UTF-8 browser path"),
                "--command-timeout-ms",
                "60000",
                "--idle-timeout-ms",
                "60000",
                "--json",
            ])
            .current_dir(workspace.path())
            .output()
            .expect("start repair lifecycle session");
        assert_process_success("start repair lifecycle session", &start);
        let state_path = session_root(workspace.path(), session).join("session.json");
        let state = serde_json::from_slice(
            &std::fs::read(&state_path).expect("read repair lifecycle session state"),
        )
        .expect("repair lifecycle session state JSON");
        Self {
            workspace,
            browser: browser.to_path_buf(),
            session: session.to_string(),
            state,
            armed: true,
        }
    }

    pub fn state(&self) -> &Value {
        &self.state
    }

    pub fn workspace(&self) -> &Path {
        self.workspace.path()
    }

    pub fn agent(&self, arguments: &[&str]) -> Output {
        Command::new(binary())
            .arg("agent")
            .args(arguments)
            .current_dir(self.workspace.path())
            .output()
            .expect("run repair lifecycle CLI command")
    }

    pub fn browser(&self, arguments: &[&str]) -> Output {
        browser_command(&self.browser, &self.state, arguments)
    }

    pub fn watch(&self) -> Value {
        let output = self.agent(&[
            "repair-watch",
            "--session",
            &self.session,
            "--timeout-ms",
            "1000",
            "--batch-window-ms",
            "50",
            "--json",
        ]);
        json_output("watch repair queue", &output)
    }

    pub fn replay(&self) -> Value {
        let output = self.agent(&[
            "repair-watch",
            "--session",
            &self.session,
            "--timeout-ms",
            "1",
            "--batch-window-ms",
            "0",
            "--json",
        ]);
        json_output("replay repair queue", &output)
    }

    pub fn transition(&self, transition: Transition<'_>) -> Value {
        let mut arguments = vec![
            transition.command.to_string(),
            transition.finding_id.to_string(),
            "--session".to_string(),
            self.session.clone(),
            "--request-id".to_string(),
            transition.request_id.to_string(),
            "--summary".to_string(),
            transition.summary.to_string(),
        ];
        if let Some(attempt_id) = transition.attempt_id {
            arguments.extend(["--attempt-id".to_string(), attempt_id.to_string()]);
        }
        if let Some(message) = transition.message {
            arguments.extend(["--message".to_string(), message.to_string()]);
        }
        if let Some(lease_ms) = transition.lease_ms {
            arguments.extend(["--lease-ms".to_string(), lease_ms.to_string()]);
        }
        arguments.push("--json".to_string());
        let borrowed = arguments.iter().map(String::as_str).collect::<Vec<_>>();
        json_output(transition.command, &self.agent(&borrowed))
    }

    pub fn abort(&mut self) -> Value {
        let output = self.agent(&["abort", "--session", &self.session, "--json"]);
        assert_process_success("abort repair lifecycle session", &output);
        self.armed = false;
        serde_json::from_slice(&output.stdout).expect("repair abort JSON")
    }

    pub fn ledger_path(&self) -> PathBuf {
        session_root(self.workspace.path(), &self.session).join("repairs.jsonl")
    }

    pub fn current_repairs(&self) -> Vec<Value> {
        current_repairs(&self.ledger_path())
    }
}

impl Drop for RepairSession {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.agent(&["abort", "--session", &self.session, "--json"]);
        }
    }
}

pub fn submit_findings(session: &RepairSession, findings_json: &str) -> Vec<Value> {
    submit_findings_in_browser(&session.browser, &session.state, findings_json)
}

pub fn submit_findings_in_browser(
    browser: &Path,
    state: &Value,
    findings_json: &str,
) -> Vec<Value> {
    let script = format!(
        "(async()=>{{const deadline=Date.now()+5000;while(Date.now()<deadline){{const bridge=window[Symbol.for('a3s.test.page-context')];if(typeof bridge?.submitRepair==='function')return JSON.stringify(bridge.submitRepair({{findings:{findings_json}}}));await new Promise(resolve=>setTimeout(resolve,25));}}throw new Error('timed out waiting for the Test Kit repair bridge')}})()"
    );
    let output = browser_command(browser, state, &["eval", &script]);
    assert_process_success("submit repair findings through the page bridge", &output);
    let encoded = browser_eval_result(&output);
    serde_json::from_str(&encoded).expect("browser repair submission JSON")
}

pub fn submit_layout_findings_from_overlay(session: &RepairSession) -> Value {
    let output = session.browser(&["eval", LAYOUT_OVERLAY_SUBMISSION_SCRIPT]);
    assert_process_success(
        "submit typed layout findings through the review overlay",
        &output,
    );
    let encoded = browser_eval_result(&output);
    serde_json::from_str(&encoded).expect("browser layout overlay result JSON")
}

pub fn target_node_ids(session: &RepairSession, test_ids: &[&str]) -> Vec<String> {
    target_node_ids_in_browser(&session.browser, &session.state, test_ids)
}

pub fn target_node_ids_in_browser(browser: &Path, state: &Value, test_ids: &[&str]) -> Vec<String> {
    let test_ids = serde_json::to_string(test_ids).expect("test IDs JSON");
    let script = format!(
        "(async()=>{{const deadline=Date.now()+5000;while(Date.now()<deadline){{const bridge=window[Symbol.for('a3s.test.page-context')];if(typeof bridge?.snapshot==='function'){{const ids=new Set({test_ids});return JSON.stringify(bridge.snapshot({{detail:'forensic'}}).nodes.filter(node=>ids.has(node.testId)).map(node=>node.id));}}await new Promise(resolve=>setTimeout(resolve,25));}}throw new Error('timed out waiting for the Test Kit page-context bridge')}})()"
    );
    let output = browser_command(browser, state, &["eval", &script]);
    assert_process_success("capture repair target node IDs", &output);
    let encoded = browser_eval_result(&output);
    serde_json::from_str(&encoded).expect("browser node ID JSON")
}

pub fn browser_command(browser: &Path, state: &Value, arguments: &[&str]) -> Output {
    let namespace = state["namespace"].as_str().expect("browser namespace");
    let driver_session = state["driver_session"]
        .as_str()
        .expect("browser session id");
    let runtime_dir = state["runtime_dir"]
        .as_str()
        .expect("browser runtime directory");
    let mut allowed_domains = state["browser_allowed_origins"]
        .as_array()
        .expect("browser allowed origins")
        .iter()
        .filter_map(|origin| {
            url::Url::parse(origin.as_str().expect("allowed origin"))
                .ok()
                .and_then(|url| url.host_str().map(str::to_string))
        })
        .collect::<Vec<_>>();
    allowed_domains.extend(
        state["browser_allowed_domains"]
            .as_array()
            .expect("browser allowed domains")
            .iter()
            .map(|domain| domain.as_str().expect("allowed domain").to_string()),
    );
    allowed_domains.sort();
    allowed_domains.dedup();
    let allowed_domains = allowed_domains.join(",");
    let mut command = Command::new(browser);
    command
        .env("AGENT_BROWSER_NAMESPACE", namespace)
        .env("AGENT_BROWSER_SOCKET_DIR", runtime_dir)
        .env("AGENT_BROWSER_IDLE_TIMEOUT_MS", "60000")
        .env("AGENT_BROWSER_ALLOWED_DOMAINS", &allowed_domains)
        .env("AGENT_BROWSER_ARGS", "--headless=new")
        .args([
            "--session",
            driver_session,
            "--json",
            "--headed",
            "false",
            "--allowed-domains",
            &allowed_domains,
            "--engine",
            "chrome",
        ])
        .args(arguments);
    bounded_output(&mut command, "run command against the owned repair browser")
}

pub fn json_output(context: &str, output: &Output) -> Value {
    assert_process_success(context, output);
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context} did not return JSON: {error}"))
}

pub fn assert_process_success(context: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn browser_eval_result(output: &Output) -> String {
    let value: Value = serde_json::from_slice(&output.stdout).expect("browser eval response JSON");
    value
        .pointer("/data/result")
        .or_else(|| value.get("result"))
        .and_then(Value::as_str)
        .expect("browser eval encoded JSON string")
        .to_string()
}

const LAYOUT_OVERLAY_SUBMISSION_SCRIPT: &str = r##"
(async () => {
  const nextFrame = () => new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))
  );
  const waitFor = async (predicate, label) => {
    for (let frame = 0; frame < 120; frame += 1) {
      if (predicate()) return;
      await nextFrame();
    }
    throw new Error(`timed out waiting for ${label}`);
  };
  await waitFor(
    () => document.querySelector("[data-a3s-testkit-overlay]")?.shadowRoot?.querySelector(".a3s-panel"),
    "the Test Kit review panel",
  );
  const host = document.querySelector("[data-a3s-testkit-overlay]");
  const shadow = host.shadowRoot;
  const source = document.querySelector("#layout-section");
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!source || !bridge) throw new Error("layout fixture is incomplete");
  const sourceStyleBefore = source.getAttribute("style");
  const button = (label) => [...shadow.querySelectorAll("button")]
    .find((candidate) => candidate.textContent.trim() === label);
  const click = (label) => {
    const target = button(label);
    if (!target || target.disabled) throw new Error(`button is unavailable: ${label}`);
    target.click();
  };
  const setInput = (label, value) => {
    const input = shadow.querySelector(`[aria-label="${label}"]`);
    if (!(input instanceof HTMLInputElement)) throw new Error(`input is unavailable: ${label}`);
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    setter.call(input, String(value));
    input.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
  };
  const setSelect = (label, value) => {
    const select = shadow.querySelector(`[aria-label="${label}"]`);
    if (!(select instanceof HTMLSelectElement)) throw new Error(`select is unavailable: ${label}`);
    const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")?.set;
    setter.call(select, value);
    select.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
  };
  const pointer = (type, x, y, buttons) => document.body.dispatchEvent(new PointerEvent(type, {
    bubbles: true,
    composed: true,
    button: 0,
    buttons,
    clientX: x,
    clientY: y,
    pointerId: 1,
    pointerType: "mouse",
    isPrimary: true,
  }));

  click("Layout");
  await nextFrame();
  setInput("Layout purpose", "Developer tool landing page");
  setSelect("Layout canvas", "wireframe");
  setInput("Layout component type", "Pricing section");
  await nextFrame();
  click("Draw placement");
  await nextFrame();
  pointer("pointerdown", 700, 320, 1);
  pointer("pointermove", 1000, 480, 1);
  pointer("pointerup", 1000, 480, 0);
  await waitFor(() => button("Add draft") && !button("Add draft").disabled, "the placement editor");
  click("Add draft");
  await waitFor(() => shadow.querySelector(".a3s-list")?.textContent.includes("Place Pricing section"), "the placement draft");

  source.focus();
  await nextFrame();
  click("Select section on page");
  await waitFor(() => document.activeElement === source, "layout source focus restoration");
  source.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, composed: true }));
  await waitFor(() => shadow.querySelector(".a3s-layout-source")?.textContent.includes("Layout source section"), "the selected layout source");
  for (const [label, value] of [
    ["Layout x", 40],
    ["Layout y", 420],
    ["Layout width", 560],
    ["Layout height", 180],
  ]) setInput(label, value);
  await waitFor(() => button("Create rearrange draft") && !button("Create rearrange draft").disabled, "the rearrange action");
  click("Create rearrange draft");
  await waitFor(() => button("Add draft") && !button("Add draft").disabled, "the rearrange editor");
  click("Add draft");
  await waitFor(() => button("Send selected (2)") && !button("Send selected (2)").disabled, "the layout batch action");
  click("Send selected (2)");
  await waitFor(() => bridge.listRepairs().length === 2, "the submitted layout batch");

  const repairs = bridge.listRepairs();
  return JSON.stringify({
    sourceStyleBefore,
    sourceStyleAfter: source.getAttribute("style"),
    batchIds: repairs.map((repair) => repair.batchId),
    layoutKinds: repairs.map((repair) => repair.target.layout?.kind),
  });
})()
"##;

fn session_root(workspace: &Path, session: &str) -> PathBuf {
    workspace
        .join(".a3s-test")
        .join("agent-sessions")
        .join(session)
}

fn current_repairs(path: &Path) -> Vec<Value> {
    let contents = std::fs::read_to_string(path).expect("read repair ledger");
    let mut order = Vec::<String>::new();
    let mut records = std::collections::BTreeMap::<String, Value>::new();
    for line in contents.lines() {
        let event: Value = serde_json::from_str(line).expect("repair ledger event JSON");
        if let Some(submitted) = event.get("submitted").or_else(|| event.get("Submitted")) {
            let finding = submitted["finding"].clone();
            let finding_id = finding["id"].as_str().expect("submitted finding id");
            order.push(finding_id.to_string());
            records.insert(
                finding_id.to_string(),
                serde_json::json!({
                    "finding": finding,
                    "status": "queued",
                    "sequence": 0,
                    "attemptId": Value::Null,
                    "summary": Value::Null,
                    "message": Value::Null,
                    "verification": Value::Null,
                }),
            );
        } else if event.get("kind").and_then(Value::as_str) == Some("submitted") {
            insert_submitted(&mut order, &mut records, event["finding"].clone());
        } else if let Some(transition) = event.get("transition").or_else(|| event.get("Transition"))
        {
            apply_transition(&mut records, &transition["event"]);
        } else if event.get("kind").and_then(Value::as_str) == Some("transition") {
            apply_transition(&mut records, &event["event"]);
        }
    }
    assert!(
        !order.is_empty(),
        "repair ledger did not contain a recognized submission event:\n{contents}"
    );
    order
        .into_iter()
        .map(|finding_id| records.remove(&finding_id).expect("ordered repair record"))
        .collect()
}

fn insert_submitted(
    order: &mut Vec<String>,
    records: &mut std::collections::BTreeMap<String, Value>,
    finding: Value,
) {
    let finding_id = finding["id"].as_str().expect("submitted finding id");
    order.push(finding_id.to_string());
    records.insert(
        finding_id.to_string(),
        serde_json::json!({
            "finding": finding,
            "status": "queued",
            "sequence": 0,
            "attemptId": Value::Null,
            "summary": Value::Null,
            "message": Value::Null,
            "verification": Value::Null,
        }),
    );
}

fn apply_transition(records: &mut std::collections::BTreeMap<String, Value>, stored: &Value) {
    let finding_id = stored["finding_id"]
        .as_str()
        .expect("transition finding id");
    let record = records
        .get_mut(finding_id)
        .expect("transition finding was submitted");
    record["status"] = stored["status"].clone();
    record["sequence"] = stored["sequence"].clone();
    record["attemptId"] = stored["attempt_id"].clone();
    record["summary"] = stored["summary"].clone();
    record["message"] = stored["message"].clone();
    if !stored["verification"].is_null() {
        record["verification"] = stored["verification"].clone();
    }
}
