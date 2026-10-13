mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use a3s_test_core::Action;
use support::browser_process::{
    assert_no_new_private_runtime_directories, assert_process_success, private_runtime_directories,
};
use support::web_fixture::WebFixture;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

/// Chrome M138+ relaunches itself at medium integrity when this process holds
/// a full elevated token, and that parent exits 0 before `DevToolsActivePort`
/// exists. agent-browser 0.26 treats the exit as a failed launch. The flag
/// keeps the process the driver waits on, and it is a no-op for a medium token.
const ELEVATED_CHROME_LAUNCH_ARGS: &str = "--do-not-de-elevate";

struct SessionCleanup {
    workspace: PathBuf,
    session: String,
    armed: bool,
}

impl Drop for SessionCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = browser_command(&self.workspace)
            .args(["agent", "abort", "--session", &self.session, "--json"])
            .output();
    }
}

fn browser_command(workspace: &Path) -> Command {
    let mut command = Command::new(binary());
    command
        .current_dir(workspace)
        .env("AGENT_BROWSER_ARGS", ELEVATED_CHROME_LAUNCH_ARGS);
    command
}

fn capture(name: &str, bytes: &[u8]) {
    let Ok(directory) = std::env::var("A3S_TEST_M33_CAPTURE") else {
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

fn verb_suite(origin: &str) -> String {
    format!(
        r##"
suite "protocol-18" {{
    version = 1

    scenario "verbs" {{
        name = "Admitted Web verbs"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{
            url = "{origin}/verbs.html"
        }}

        click "load-live" {{
            target = css("#load")
        }}

        wait_response "live-response" {{
            pattern = "api/profile"
            timeout_ms = 5000
        }}

        expect "live-name" {{
            target = css("#name")
            rendered_text = "Ada"
        }}

        network_abort "drop" {{
            pattern = "*/api/abort-probe"
        }}

        click "abort" {{
            target = css("#abort")
        }}

        expect "aborted" {{
            target = css("#abort-out")
            rendered_text = "failed"
        }}

        network_continue "restore" {{
            pattern = "*/api/abort-probe"
        }}

        click "abort-live" {{
            target = css("#abort")
        }}

        expect "restored" {{
            target = css("#abort-out")
            rendered_text = "ok"
        }}

        scroll_until "pane-end" {{
            target = css("#end")
            container = css("#pane")
            direction = "down"
            timeout_ms = 5000
        }}

        expect "end-visible" {{
            in_viewport = css("#end")
        }}

        long_press "hold" {{
            target = css("#hold")
            duration_ms = 700
        }}

        expect "held" {{
            target = css("#held")
            rendered_text = "held"
        }}
    }}
}}
"##
    )
}

fn surface_suite(step: &str) -> String {
    format!(
        r#"
suite "reject-surface" {{
    version = 1
    scenario "terminal" {{
        name = "Reject a Web verb"
        surface = "tui"
        timeout_ms = 15000
        {step}
    }}
}}
"#
    )
}

#[test]
fn protocol18_verbs_run_on_the_local_fixture_and_fail_closed_elsewhere() {
    let temp = tempfile::tempdir().expect("temporary protocol 18 workspace");
    let rejected = [
        (
            "evaluate",
            r#"evaluate "script" {}"#,
            "test.spec.browser_verb_unsupported",
        ),
        (
            "init-script",
            r#"init_script "boot" {}"#,
            "test.spec.browser_verb_unsupported",
        ),
        (
            "forward",
            r#"forward "next" {}"#,
            "test.spec.action_unknown",
        ),
        (
            "closed-shadow",
            r#"closed_shadow "root" {}"#,
            "test.spec.browser_verb_unsupported",
        ),
        (
            "click-position",
            r##"click "box" { target = css("#go") position = "center" }"##,
            "test.spec.browser_verb_unsupported",
        ),
        (
            "fulfill-status",
            r#"network_fulfill "api" { pattern = "*api/profile" body = "Grace" status = 200 }"#,
            "test.spec.browser_verb_unsupported",
        ),
        (
            "fulfill-body",
            r#"network_fulfill "stub" { pattern = "*/api/profile" body = "{\"name\":\"Grace\"}" }"#,
            "test.spec.browser_verb_unsupported",
        ),
        (
            "modified-click",
            r##"click "shifted" { target = css("#go") modifiers = ["shift"] }"##,
            "test.spec.browser_verb_unsupported",
        ),
        (
            "modified-double-click",
            r##"double_click "shifted" { target = css("#dbl") modifiers = ["shift"] }"##,
            "test.spec.browser_verb_unsupported",
        ),
    ];
    for (name, step, code) in rejected {
        let suite = temp.path().join(format!("{name}.acl"));
        fs::write(
            &suite,
            surface_suite(step).replace("surface = \"tui\"", "surface = \"web\""),
        )
        .expect("rejected suite");
        let before = private_runtime_directories();
        let output = browser_command(temp.path())
            .args(["run", suite.to_str().expect("suite path"), "--json"])
            .output()
            .expect("run rejected verb");
        capture(
            &format!("reject-{name}.txt"),
            output_text(&output).as_bytes(),
        );
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name} should fail before a browser command\n{}",
            output_text(&output)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(code),
            "{name} stderr missing {code}: {stderr}"
        );
        assert_no_new_private_runtime_directories(&before);
    }

    let (terminal, terminal_args) = terminal_command();
    let terminal_steps = [
        ("reload", r#"reload "again" {}"#),
        (
            "long-press",
            r##"long_press "hold" { target = css("#hold") duration_ms = 500 }"##,
        ),
        (
            "container-scroll",
            r##"scroll_until "pane" { target = css("#end") container = css("#pane") direction = "down" timeout_ms = 50 }"##,
        ),
    ];
    let terminal_path = terminal.to_str().expect("terminal path").to_string();
    for (name, step) in terminal_steps {
        let suite = temp.path().join(format!("tui-{name}.acl"));
        fs::write(&suite, surface_suite(step)).expect("tui suite");
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
        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
                panic!(
                    "tui {name} report is not JSON: {error}\n{}",
                    output_text(&output)
                )
            });
        let error = &report["scenarios"][0]["steps"][0]["error"];
        assert_eq!(
            error["code"], "test.driver.tui.action_unsupported",
            "tui {name} report: {report}"
        );
    }

    let browser = admitted_browser();
    let fixture = WebFixture::start().expect("start verb fixture");
    let origin = fixture.origin();
    let suite_path = temp.path().join("verbs.acl");
    fs::write(&suite_path, verb_suite(&origin)).expect("verb suite");
    capture(
        "verbs.acl",
        &fs::read(&suite_path).expect("read verb suite"),
    );
    let browser_arg = browser.to_str().expect("UTF-8 browser path").to_string();
    let ran = browser_command(temp.path())
        .args([
            "run",
            suite_path.to_str().expect("suite path"),
            "--browser-driver",
            "standalone",
            "--browser-executable",
            browser_arg.as_str(),
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
        .expect("run verb suite");
    capture("run-verbs.json", &ran.stdout);
    capture("run-verbs.err", &ran.stderr);
    assert_process_success("run protocol 18 verbs", &ran);
    let report: serde_json::Value = serde_json::from_slice(&ran.stdout).expect("verb report");
    assert_eq!(report["status"], "passed");

    let session_name = format!("protocol18-{}", std::process::id());
    let mut session = SessionCleanup {
        workspace: temp.path().to_path_buf(),
        session: session_name.clone(),
        armed: true,
    };
    let page = format!("{origin}/verbs.html");
    let started = browser_command(temp.path())
        .args([
            "agent",
            "start",
            page.as_str(),
            "--session",
            session_name.as_str(),
            "--goal",
            "Reload stays on the fixture origin",
            "--success",
            "A cross-origin reload is denied",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            browser_arg.as_str(),
            "--command-timeout-ms",
            "60000",
            "--idle-timeout-ms",
            "30000",
            "--json",
        ])
        .output()
        .expect("start reload session");
    capture("reload-start.json", &started.stdout);
    assert_process_success("start reload session", &started);

    let redirect = format!("{origin}/redirect-cross");
    let navigate =
        serde_json::to_string(&Action::Navigate { url: redirect }).expect("navigate JSON");
    let crossed = browser_command(temp.path())
        .args([
            "agent",
            "act",
            "--session",
            session_name.as_str(),
            "--action-json",
            navigate.as_str(),
            "--json",
        ])
        .output()
        .expect("navigate to cross-origin redirect");
    capture("reload-navigate.json", &crossed.stdout);
    capture("reload-navigate.err", &crossed.stderr);
    assert_process_success("follow cross-origin redirect", &crossed);

    let reload = serde_json::to_string(&Action::Reload {}).expect("reload JSON");
    let denied = browser_command(temp.path())
        .args([
            "agent",
            "act",
            "--session",
            session_name.as_str(),
            "--action-json",
            reload.as_str(),
            "--json",
        ])
        .output()
        .expect("reload unapproved origin");
    capture("reload-denied.json", &denied.stdout);
    capture("reload-denied.err", &denied.stderr);
    let denied_body: serde_json::Value =
        serde_json::from_slice(&denied.stdout).expect("reload denial JSON");
    assert_eq!(
        denied_body["error"]["code"],
        "test.driver.web.navigation_origin_denied",
        "{}",
        output_text(&denied)
    );
    let message = denied_body["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("reload"), "{message}");
    assert!(!message.contains("/blocked"), "{message}");
    assert!(!message.contains("secret"), "{message}");
    session.armed = false;
    let aborted = browser_command(temp.path())
        .args([
            "agent",
            "abort",
            "--session",
            session_name.as_str(),
            "--json",
        ])
        .output()
        .expect("abort reload session");
    capture("reload-abort.json", &aborted.stdout);
    assert_process_success("abort reload session", &aborted);
}
