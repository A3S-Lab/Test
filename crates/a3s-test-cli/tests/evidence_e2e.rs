mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use support::browser_process::assert_process_success;
use support::web_fixture::WebFixture;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

/// Chrome M138+ relaunches itself at medium integrity when this process holds
/// a full elevated token, and that parent exits 0 before `DevToolsActivePort`
/// exists. agent-browser 0.26 treats the exit as a failed launch. The flag
/// keeps the process the driver waits on, and it is a no-op for a medium token.
const ELEVATED_CHROME_LAUNCH_ARGS: &str = "--do-not-de-elevate";
const PREVIEW_PASSWORD: &str = "preview-secret";
const PREVIEW_TOKEN: &str = "YWRhOnByZXZpZXctc2VjcmV0";

fn browser_command(workspace: &Path) -> Command {
    let mut command = Command::new(binary());
    command
        .current_dir(workspace)
        .env("AGENT_BROWSER_ARGS", ELEVATED_CHROME_LAUNCH_ARGS)
        .env_remove("PREVIEW_PASSWORD");
    command
}

fn capture(name: &str, bytes: &[u8]) {
    let Ok(directory) = std::env::var("A3S_TEST_M34_CAPTURE") else {
        return;
    };
    let directory = PathBuf::from(directory);
    fs::create_dir_all(&directory).expect("capture directory");
    fs::write(directory.join(name), bytes).expect("capture file");
}

fn admitted_browser() -> PathBuf {
    if let Some(override_path) = std::env::var_os("A3S_TEST_AGENT_BROWSER") {
        let browser = resolve_browser(Path::new(&override_path)).unwrap_or_else(|| {
            panic!(
                "A3S_TEST_AGENT_BROWSER={:?} is not an admitted browser executable",
                override_path
            )
        });
        assert_browser_version(&browser);
        return browser;
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut found = Vec::new();
    for directory in std::env::split_paths(&path) {
        // Cargo prepends target/debug. An A3S product binary named a3s.exe is
        // not agent-browser, and its --version starts a shell that does not exit.
        for name in ["agent-browser", "agent-browser.exe"] {
            let candidate = directory.join(name);
            if candidate.is_file() {
                found.push(candidate);
            }
        }
    }
    let browser = found
        .iter()
        .find_map(|path| resolve_browser(path))
        .unwrap_or_else(|| {
            panic!(
                "admitted browser executable was not found. Set A3S_TEST_AGENT_BROWSER or install agent-browser 0.26.x. PATH hits: {found:?}"
            )
        });
    assert_browser_version(&browser);
    browser
}

fn resolve_browser(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let bin = parent
        .join("node_modules")
        .join("agent-browser")
        .join("bin");
    for name in [
        "agent-browser-win32-x64.exe",
        "agent-browser-linux-x64",
        "agent-browser-darwin-arm64",
        "agent-browser-darwin-x64",
    ] {
        let packaged = bin.join(name);
        if packaged.is_file() {
            return Some(packaged);
        }
    }
    let extension = path.extension().and_then(|extension| extension.to_str());
    if extension == Some("exe") || (cfg!(unix) && extension.is_none()) {
        return path.is_file().then(|| path.to_path_buf());
    }
    None
}

fn assert_browser_version(browser: &Path) {
    let mut child = Command::new(browser)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("probe {} failed: {error}", browser.display()));
    let started = std::time::Instant::now();
    let deadline = std::time::Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "browser version probe timed out after {}s: {}",
                    deadline.as_secs(),
                    browser.display()
                );
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => panic!("probe {} failed: {error}", browser.display()),
        }
    }
    let version = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("probe {} failed: {error}", browser.display()));
    assert!(
        version.status.success(),
        "browser version probe failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    assert!(
        printed.contains("0.26."),
        "admitted browser must be 0.26.x, probe returned: {printed}"
    );
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn terminal_command() -> (PathBuf, Vec<&'static str>) {
    if cfg!(windows) {
        (
            PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            vec![
                "-NoProfile",
                "-Command",
                "Write-Output ready; Start-Sleep -Seconds 20",
            ],
        )
    } else {
        (
            PathBuf::from("/bin/sh"),
            vec!["-c", "printf ready; sleep 20"],
        )
    }
}

fn passing_suite(origin: &str) -> String {
    format!(
        r##"
suite "evidence" {{
    version = 1

    scenario "icons" {{
        name = "Icon names, inert observation, and cookies"
        surface = "web"
        timeout_ms = 90000

        navigate "open" {{
            url = "{origin}/evidence.html"
        }}

        click "save" {{
            target = role("button", "Save")
        }}

        expect "saved" {{
            target = css("#out")
            rendered_text = "saved"
        }}

        snapshot "controls" {{
            interactive = true
        }}

        expect "quiet-copy" {{
            target = css("#quiet")
            rendered_text = "Quiet"
        }}

        click "quiet" {{
            target = css("#quiet")
        }}

        set_cookie "jar" {{
            origin = "{origin}"
            names = ["a", "b"]
            values = ["1", "2"]
        }}

        click "read" {{
            target = css("#read")
        }}

        expect "cookies" {{
            target = css("#cookies")
            rendered_text = "a=1; b=2"
        }}
    }}

    scenario "isolation" {{
        name = "A fresh session has no cookies"
        surface = "web"
        timeout_ms = 90000

        navigate "open" {{
            url = "{origin}/evidence.html"
        }}

        click "read" {{
            target = css("#read")
        }}

        expect "none" {{
            target = css("#cookies")
            rendered_text = "none"
        }}
    }}

    scenario "preview" {{
        name = "Preview headers stay on the target origin"
        surface = "web"
        timeout_ms = 90000

        preview_headers "site" {{
            origin = "{origin}"
            header_names = ["X-Preview"]
            header_values = ["on"]
            username = "ada"
            password_env = "PREVIEW_PASSWORD"
        }}

        navigate "open" {{
            url = "{origin}/evidence.html"
        }}

        click "probe" {{
            target = css("#probe")
        }}

        expect "same-origin" {{
            target = css("#same")
            rendered_text = "preview=yes basic=yes"
        }}

        expect "other-origin" {{
            target = css("#other")
            rendered_text = "preview=no basic=no"
        }}
    }}
}}
"##
    )
}

fn web_suite(name: &str, scenario: &str, body: &str) -> String {
    format!(
        r#"
suite "{name}" {{
    version = 1
    scenario "{scenario}" {{
        name = "{scenario}"
        surface = "web"
        timeout_ms = 90000
        {body}
    }}
}}
"#
    )
}

fn report_of(output: &Output, label: &str) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{label} report is not JSON: {error}\n{}",
            output_text(output)
        )
    })
}

fn scenario<'a>(report: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    report["scenarios"]
        .as_array()
        .and_then(|scenarios| scenarios.iter().find(|item| item["id"] == id))
        .unwrap_or_else(|| panic!("missing scenario {id} in {report}"))
}

fn step<'a>(report: &'a serde_json::Value, scenario_id: &str, id: &str) -> &'a serde_json::Value {
    scenario(report, scenario_id)["steps"]
        .as_array()
        .and_then(|steps| steps.iter().find(|item| item["id"] == id))
        .unwrap_or_else(|| panic!("missing step {scenario_id}.{id} in {report}"))
}

fn assert_passed(report: &serde_json::Value, scenario_id: &str, id: &str) {
    assert_eq!(
        step(report, scenario_id, id)["status"],
        "passed",
        "{scenario_id}.{id} did not pass: {report}"
    );
}

fn run_web(workspace: &Path, suite: &Path, browser: &Path, password: Option<&str>) -> Output {
    let browser_arg = browser.to_str().expect("UTF-8 browser path");
    let mut command = browser_command(workspace);
    if let Some(password) = password {
        command.env("PREVIEW_PASSWORD", password);
    }
    command
        .args([
            "run",
            suite.to_str().expect("suite path"),
            "--browser-driver",
            "standalone",
            "--browser-executable",
            browser_arg,
            "--command-timeout-ms",
            "60000",
            "--idle-timeout-ms",
            "15000",
            "--cleanup-timeout-ms",
            "15000",
            "--infrastructure-retries",
            "0",
            "--json",
        ])
        .output()
        .expect("run web suite")
}

fn assert_secret_absent(text: &str) {
    assert!(
        !text.contains(PREVIEW_PASSWORD),
        "report leaked the preview password"
    );
    assert!(
        !text.contains(PREVIEW_TOKEN),
        "report leaked the basic-auth token"
    );
}

#[test]
fn evidence_behaviors_run_on_the_local_fixture_and_fail_closed_elsewhere() {
    let temp = tempfile::tempdir().expect("temporary evidence workspace");
    let browser = admitted_browser();
    let fixture = WebFixture::start().expect("start evidence fixture");
    let origin = fixture.origin();

    let passing_path = temp.path().join("evidence.acl");
    let passing_acl = passing_suite(&origin);
    fs::write(&passing_path, &passing_acl).expect("evidence suite");
    capture("evidence.acl", passing_acl.as_bytes());
    let passed = run_web(temp.path(), &passing_path, &browser, Some(PREVIEW_PASSWORD));
    capture("run-evidence.json", &passed.stdout);
    capture("run-evidence.err", &passed.stderr);
    assert_process_success("run evidence suite", &passed);
    let passed_text = output_text(&passed);
    assert_secret_absent(&passed_text);
    let report = report_of(&passed, "evidence");
    assert_eq!(report["status"], "passed");
    assert_passed(&report, "icons", "save");
    assert_passed(&report, "icons", "saved");
    assert_passed(&report, "icons", "quiet");
    assert_passed(&report, "icons", "cookies");
    let snapshot = step(&report, "icons", "controls")["output"]["data"].to_string();
    assert!(
        snapshot.contains("Live"),
        "interactive snapshot omitted Live: {snapshot}"
    );
    assert!(
        !snapshot.contains("Quiet"),
        "interactive snapshot included inert Quiet: {snapshot}"
    );
    let jar = &step(&report, "icons", "jar")["output"]["data"];
    assert_eq!(jar["names"], serde_json::json!(["a", "b"]));
    assert_eq!(jar["count"], 2);
    assert!(!jar.to_string().contains("\"value\""));
    assert_passed(&report, "isolation", "none");
    assert_passed(&report, "preview", "same-origin");
    assert_passed(&report, "preview", "other-origin");
    let headers = &step(&report, "preview", "site")["output"]["data"];
    assert_eq!(
        headers["headers"],
        serde_json::json!(["X-Preview", "Authorization"])
    );
    assert_eq!(headers["password_env"], "PREVIEW_PASSWORD");
    assert!(!headers.to_string().contains("\"on\""));
    assert_secret_absent(&headers.to_string());

    let pixel_body = format!(
        r##"
        viewport "frame" {{
            width = 800
            height = 600
        }}
        navigate "open" {{
            url = "{origin}/pixel.html"
        }}
        expect "baseline" {{
            pixels = "baseline.png"
            threshold_millis = 0
            update = true
        }}
        expect "stable" {{
            pixels = "baseline.png"
            threshold_millis = 0
            update = false
        }}
        click "shift" {{
            target = css("#shift")
        }}
        expect "changed" {{
            pixels = "baseline.png"
            threshold_millis = 0
            update = false
        }}
        "##
    );
    let pixel_path = temp.path().join("pixels.acl");
    fs::write(&pixel_path, web_suite("pixels", "pixels", &pixel_body)).expect("pixel suite");
    let pixels = run_web(temp.path(), &pixel_path, &browser, None);
    capture("run-pixels.json", &pixels.stdout);
    capture("run-pixels.err", &pixels.stderr);
    assert_eq!(
        pixels.status.code(),
        Some(1),
        "pixel mismatch should fail the run\n{}",
        output_text(&pixels)
    );
    let pixel_report = report_of(&pixels, "pixels");
    assert_passed(&pixel_report, "pixels", "baseline");
    assert_passed(&pixel_report, "pixels", "stable");
    assert_passed(&pixel_report, "pixels", "shift");
    let changed = step(&pixel_report, "pixels", "changed");
    assert_eq!(changed["error"]["code"], "test.assert.pixel_diff");
    let message = changed["error"]["message"].as_str().expect("diff message");
    assert!(message.contains("exceeds allowed 0"), "{message}");
    let run_id = pixel_report["run_id"].as_str().expect("run id");
    let artifacts = temp
        .path()
        .join(".a3s-test")
        .join("runs")
        .join(run_id)
        .join("pixels");
    let wrote = message
        .split_once("wrote ")
        .map(|(_, names)| names)
        .expect("mismatch message names the images");
    for name in wrote.split(", ") {
        let name = name.trim().trim_start_matches("and ").trim();
        let path = artifacts.join(name);
        assert!(path.is_file(), "missing mismatch image {}", path.display());
        assert!(fs::metadata(&path).expect("image metadata").len() > 32);
    }

    let flicker_body = format!(
        r#"
        navigate "open" {{
            url = "{origin}/flicker.html"
        }}
        expect "flicker" {{
            pixels = "baseline.png"
            threshold_millis = 0
            update = true
        }}
        "#
    );
    let flicker_path = temp.path().join("flicker.acl");
    fs::write(
        &flicker_path,
        web_suite("flicker", "flicker", &flicker_body),
    )
    .expect("flicker suite");
    let flicker = run_web(temp.path(), &flicker_path, &browser, None);
    capture("run-flicker.json", &flicker.stdout);
    capture("run-flicker.err", &flicker.stderr);
    let flicker_report = report_of(&flicker, "flicker");
    assert_eq!(
        step(&flicker_report, "flicker", "flicker")["error"]["code"],
        "test.driver.web.pixel_unstable",
        "{flicker_report}"
    );
    let flicker_run = flicker_report["run_id"].as_str().expect("flicker run id");
    let flicker_artifacts = temp
        .path()
        .join(".a3s-test")
        .join("runs")
        .join(flicker_run)
        .join("flicker");
    assert!(!flicker_artifacts.join("pixel-mismatch-0-diff.png").exists());
    assert!(!flicker_artifacts.join("baseline.png").exists());

    let rejected_cookie = r#"
        set_cookie "jar" {
            origin = "file:///tmp"
            names = ["a"]
            values = ["1"]
        }
    "#;
    let cookie_path = temp.path().join("cookie-origin.acl");
    fs::write(
        &cookie_path,
        web_suite("cookie-origin", "cookie-origin", rejected_cookie),
    )
    .expect("cookie suite");
    let cookie = run_web(temp.path(), &cookie_path, &browser, None);
    capture("run-cookie-origin.json", &cookie.stdout);
    capture("run-cookie-origin.err", &cookie.stderr);
    let cookie_report = report_of(&cookie, "cookie origin");
    let cookie_error = &step(&cookie_report, "cookie-origin", "jar")["error"];
    assert_eq!(
        cookie_error["code"],
        "test.driver.web.cookie_origin_invalid"
    );
    assert_eq!(
        cookie_error["message"],
        "cookie batch was rejected before any cookie command"
    );

    let missing_secret = format!(
        r#"
        preview_headers "site" {{
            origin = "{origin}"
            header_names = ["X-Preview"]
            header_values = ["on"]
            username = "ada"
            password_env = "PREVIEW_PASSWORD"
        }}
        "#
    );
    let secret_path = temp.path().join("missing-password.acl");
    fs::write(
        &secret_path,
        web_suite("missing-password", "missing-password", &missing_secret),
    )
    .expect("secret suite");
    let secret = run_web(temp.path(), &secret_path, &browser, None);
    capture("run-missing-password.json", &secret.stdout);
    capture("run-missing-password.err", &secret.stderr);
    let secret_report = report_of(&secret, "missing password");
    let secret_error = &step(&secret_report, "missing-password", "site")["error"];
    assert_eq!(secret_error["code"], "test.driver.web.secret_unavailable");
    let secret_message = secret_error["message"].as_str().expect("secret message");
    assert!(
        secret_message.contains("PREVIEW_PASSWORD"),
        "{secret_message}"
    );
    assert!(
        secret_message.contains("unset or empty"),
        "{secret_message}"
    );
    assert_secret_absent(&output_text(&secret));

    let (terminal, terminal_args) = terminal_command();
    let terminal_path = terminal.to_str().expect("terminal path").to_string();
    let terminal_steps = [
        (
            "set-cookie",
            r#"set_cookie "jar" { origin = "http://127.0.0.1" names = ["a"] values = ["1"] }"#,
        ),
        (
            "preview-headers",
            r#"preview_headers "site" { origin = "http://127.0.0.1" header_names = ["X-Preview"] header_values = ["on"] }"#,
        ),
    ];
    for (name, step_acl) in terminal_steps {
        let suite = temp.path().join(format!("tui-{name}.acl"));
        let body = format!(
            r#"
suite "reject-surface" {{
    version = 1
    scenario "terminal" {{
        name = "Reject a Web verb"
        surface = "tui"
        timeout_ms = 15000
        {step_acl}
    }}
}}
"#
        );
        fs::write(&suite, body).expect("tui suite");
        let mut args = vec![
            "run".to_string(),
            suite.to_str().expect("suite path").to_string(),
            "--tui-executable".to_string(),
            terminal_path.clone(),
            "--command-timeout-ms".to_string(),
            "15000".to_string(),
            "--cleanup-timeout-ms".to_string(),
            "5000".to_string(),
            "--infrastructure-retries".to_string(),
            "0".to_string(),
            "--json".to_string(),
        ];
        for argument in &terminal_args {
            args.push("--tui-arg".to_string());
            args.push((*argument).to_string());
        }
        let output = browser_command(temp.path())
            .args(&args)
            .output()
            .expect("run tui rejection");
        capture(&format!("tui-{name}.json"), &output.stdout);
        capture(&format!("tui-{name}.err"), &output.stderr);
        let tui_report = report_of(&output, name);
        assert_eq!(
            tui_report["scenarios"][0]["steps"][0]["error"]["code"],
            "test.driver.tui.action_unsupported",
            "tui {name} report: {tui_report}"
        );
    }
}
