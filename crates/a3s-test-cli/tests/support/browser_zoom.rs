use std::process::{Command, Output};

use super::browser_process::assert_process_success;

pub fn capture_testkit_zoom_geometry(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
) -> serde_json::Value {
    let output = command(&[
        "eval",
        "(()=>{const snapshot=window[Symbol.for('a3s.test.page-context')].snapshot({detail:'forensic'});const target=snapshot.nodes.find(node=>node.testId==='zoom-edge');return JSON.stringify({page:snapshot.page,target});})()",
    ]);
    assert_process_success(context, &output);
    let encoded: String = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context} did not return an encoded JSON string: {error}"));
    serde_json::from_str(&encoded)
        .unwrap_or_else(|error| panic!("{context} returned invalid TestKit JSON: {error}"))
}

pub fn set_browser_page_scale(cdp_url: &str, factor: f64, command: &impl Fn(&[&str]) -> Output) {
    let port = cdp_url
        .split_once("://")
        .and_then(|(_, rest)| rest.split('/').next())
        .and_then(|authority| authority.rsplit_once(':'))
        .map(|(_, port)| port)
        .unwrap_or_else(|| panic!("browser returned an invalid CDP URL: {cdp_url}"));
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            CDP_PAGE_SCALE_SCRIPT,
            port,
            &factor.to_string(),
        ])
        .output()
        .expect("run bounded CDP page-scale helper");
    assert_process_success("set browser page scale", &output);
    let expected = format!("visualViewport.scale==={factor}");
    let wait = command(&["wait", "--fn", &expected]);
    assert_process_success("wait for browser page scale", &wait);
}

pub fn assert_approx(actual: Option<&serde_json::Value>, expected: f64, epsilon: f64, label: &str) {
    let actual = actual
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| panic!("{label} was not a number"));
    assert!(
        (actual - expected).abs() <= epsilon,
        "{label}: expected {expected} ± {epsilon}, got {actual}"
    );
}

pub fn json_number(value: &serde_json::Value, pointer: &str, label: &str) -> f64 {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| panic!("{label} was not a number"))
}

const CDP_PAGE_SCALE_SCRIPT: &str = r#"
const port = process.argv[1];
const factor = Number(process.argv[2]);
if (!/^\d{1,5}$/.test(port) || !Number.isFinite(factor) || factor < 0.25 || factor > 5) {
  throw new Error("invalid bounded page-scale arguments");
}
const response = await fetch(`http://127.0.0.1:${port}/json/list`);
if (!response.ok) throw new Error(`CDP target discovery returned ${response.status}`);
const targets = await response.json();
const page = targets.find((target) => target.type === "page");
if (!page?.webSocketDebuggerUrl) throw new Error("CDP page target missing");
await new Promise((resolve, reject) => {
  const socket = new WebSocket(page.webSocketDebuggerUrl);
  const timer = setTimeout(() => reject(new Error("CDP page-scale command timed out")), 5_000);
  socket.onopen = () => socket.send(JSON.stringify({
    id: 1,
    method: "Emulation.setPageScaleFactor",
    params: { pageScaleFactor: factor },
  }));
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id !== 1) return;
    clearTimeout(timer);
    socket.close();
    if (message.error) reject(new Error(JSON.stringify(message.error)));
    else resolve();
  };
  socket.onerror = () => reject(new Error("CDP WebSocket failed"));
});
"#;
