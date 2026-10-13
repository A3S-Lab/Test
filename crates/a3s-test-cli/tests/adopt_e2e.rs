mod support;

use std::fs;
use std::io::ErrorKind;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use a3s_test_core::{
    verified_session_candidate, Action, Expectation, RecordedStep, Target, TestSuite,
};
use support::browser_process::{
    assert_no_new_private_runtime_directories, assert_process_success, private_runtime_directories,
};
use support::web_fixture::WebFixture;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

fn run_at(workspace: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("run a3s-test")
}

fn assert_refused(output: &Output, code: &str) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected {code}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(code), "stderr missing {code}: {stderr}");
}

fn write_report(path: &Path, candidate: Option<&str>) {
    let report = serde_json::json!({
        "schema_version": 1,
        "session": "verified",
        "acl_candidate": candidate,
    });
    fs::write(
        path,
        serde_json::to_vec_pretty(&report).expect("report JSON"),
    )
    .expect("write report");
}

fn verified_candidate() -> String {
    verified_session_candidate(
        &[
            RecordedStep {
                index: 1,
                action: Action::Navigate {
                    url: "https://example.test/adopt".to_string(),
                },
                passed: true,
            },
            RecordedStep {
                index: 2,
                action: Action::Assert {
                    expectation: Expectation::TextVisible("Saved".to_string()),
                },
                passed: true,
            },
        ],
        &[],
    )
    .expect("verified candidate")
}

#[test]
fn adopt_writes_a_caller_named_suite_and_refuses_unstable_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = temp.path().join("report.json");
    let output = temp.path().join("checkout-adopted.acl");
    write_report(&report, Some(&verified_candidate()));

    let adopted = run_at(
        temp.path(),
        &[
            "adopt",
            "--report",
            report.to_str().expect("report path"),
            "--output",
            output.to_str().expect("output path"),
            "--name",
            "checkout-adopted",
            "--json",
        ],
    );
    assert_process_success("adopt verified candidate", &adopted);
    let body: serde_json::Value = serde_json::from_slice(&adopted.stdout).expect("adopt JSON");
    assert_eq!(body["suite"], "checkout-adopted");
    assert_eq!(body["scenarios"], 1);
    let acl = fs::read_to_string(&output).expect("written suite");
    let suite = TestSuite::from_acl(&acl).expect("parse adopted suite");
    assert_eq!(suite.name, "checkout-adopted");
    assert!(acl.contains("https://example.test/adopt"));
    assert!(acl.contains("Saved"));
    assert!(!acl.contains("long_press"));
    assert!(!acl.contains("@e"));
    assert!(!acl.contains("visual_point("));

    let unchanged = fs::read(&output).expect("reread suite");
    let refused = run_at(
        temp.path(),
        &[
            "adopt",
            "--report",
            report.to_str().expect("report path"),
            "--output",
            output.to_str().expect("output path"),
            "--name",
            "checkout-adopted",
        ],
    );
    assert_refused(&refused, "test.adopt.output_exists");
    assert_eq!(fs::read(&output).expect("suite after refusal"), unchanged);

    let replaced = run_at(
        temp.path(),
        &[
            "adopt",
            "--report",
            report.to_str().expect("report path"),
            "--output",
            output.to_str().expect("output path"),
            "--name",
            "checkout-adopted",
            "--replace",
            "--json",
        ],
    );
    assert_process_success("replace adopted suite", &replaced);

    let prose_output = temp.path().join("prose.acl");
    write_report(&report, None);
    let prose = run_at(
        temp.path(),
        &[
            "adopt",
            "--report",
            report.to_str().expect("report path"),
            "--output",
            prose_output.to_str().expect("prose output"),
        ],
    );
    assert_refused(&prose, "test.adopt.prose_only");
    assert!(
        !prose_output.exists(),
        "prose refusal must not write a suite"
    );

    let with_ref = r#"
suite "verified-session" {
    version = 1
    scenario "verified" {
        name = "Verified session"
        surface = "web"
        timeout_ms = 60000
        click "open" { target = ref("@e4") }
        expect "saved" { text = "Saved" }
    }
}
"#;
    write_report(&report, Some(with_ref));
    let ref_output = temp.path().join("ref.acl");
    let refused_ref = run_at(
        temp.path(),
        &[
            "adopt",
            "--report",
            report.to_str().expect("report path"),
            "--output",
            ref_output.to_str().expect("ref output"),
        ],
    );
    assert_refused(&refused_ref, "test.adopt.observation_ref");
    assert!(!ref_output.exists());

    let with_point = r#"
suite "verified-session" {
    version = 1
    scenario "verified" {
        name = "Verified session"
        surface = "web"
        timeout_ms = 60000
        click "open" { target = visual_point("shot", 3, 4) }
        expect "saved" { text = "Saved" }
    }
}
"#;
    write_report(&report, Some(with_point));
    let point_output = temp.path().join("point.acl");
    let refused_point = run_at(
        temp.path(),
        &[
            "adopt",
            "--report",
            report.to_str().expect("report path"),
            "--output",
            point_output.to_str().expect("point output"),
        ],
    );
    assert_refused(&refused_point, "test.adopt.visual_point");
    assert!(!point_output.exists());

    let secret = "adopt-secret-literal";
    let secret_candidate = verified_session_candidate(
        &[
            RecordedStep {
                index: 1,
                action: Action::Navigate {
                    url: "https://example.test/login".to_string(),
                },
                passed: true,
            },
            RecordedStep {
                index: 2,
                action: Action::Assert {
                    expectation: Expectation::TextVisible(secret.to_string()),
                },
                passed: true,
            },
        ],
        &[],
    )
    .expect("candidate containing a literal");
    write_report(&report, Some(&secret_candidate));
    let secret_output = temp.path().join("secret.acl");
    let refused_secret = Command::new(binary())
        .args([
            "adopt",
            "--report",
            report.to_str().expect("report path"),
            "--output",
            secret_output.to_str().expect("secret output"),
            "--secret-env",
            "A3S_TEST_ADOPT_SECRET_LITERAL",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_ADOPT_SECRET_LITERAL", secret)
        .output()
        .expect("adopt secret candidate");
    assert_refused(&refused_secret, "test.adopt.secret_literal");
    let stderr = String::from_utf8_lossy(&refused_secret.stderr);
    assert!(
        !stderr.contains(secret),
        "refusal must not echo the secret literal"
    );
    assert!(!secret_output.exists());
}

fn admitted_browser() -> PathBuf {
    if let Some(value) = std::env::var_os("A3S_TEST_AGENT_BROWSER") {
        let path = PathBuf::from(value);
        assert!(
            path.is_file(),
            "admitted browser executable is missing: {}",
            path.display()
        );
        assert_browser_version(&path);
        return path;
    }
    let mut found = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            for name in [
                "agent-browser.exe",
                "agent-browser",
                "agent-browser.cmd",
                "agent-browser.ps1",
            ] {
                let candidate = directory.join(name);
                if candidate.is_file() {
                    found.push(candidate);
                }
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
    let version = Command::new(browser)
        .arg("--version")
        .output()
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

fn agent(workspace: &Path, args: &[String]) -> Output {
    browser_command(workspace)
        .arg("agent")
        .args(args)
        .output()
        .expect("run a3s-test agent")
}

fn capture(name: &str, bytes: &[u8]) {
    let Ok(directory) = std::env::var("A3S_TEST_M32_CAPTURE") else {
        return;
    };
    let directory = PathBuf::from(directory);
    fs::create_dir_all(&directory).expect("capture directory");
    fs::write(directory.join(name), bytes).expect("capture file");
}

fn acl_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(acl_files(&path));
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("acl") {
            found.push(path);
        }
    }
    found
}

fn failed_step_error(report: &serde_json::Value) -> &serde_json::Value {
    report["scenarios"][0]["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|step| step["status"] == "failed")
        .unwrap_or_else(|| panic!("no failed step in {report}"))
        .get("error")
        .expect("failed step error")
}

#[test]
fn adopted_suite_runs_without_a_model_provider_and_marks_a_stale_locator() {
    let browser = admitted_browser();
    let fixture = WebFixture::start().expect("start adopt fixture");
    let origin = fixture.origin();
    let page = format!("{origin}/adopt.html");
    let temp = tempfile::tempdir().expect("temporary adopt workspace");
    let session_name = format!("adopt-{}", std::process::id());
    let mut session = SessionCleanup {
        workspace: temp.path().to_path_buf(),
        session: session_name.clone(),
        armed: true,
    };
    let browser_arg = browser.to_str().expect("UTF-8 browser path").to_string();
    let started = agent(
        temp.path(),
        &[
            "start".to_string(),
            page.clone(),
            "--session".to_string(),
            session_name.clone(),
            "--goal".to_string(),
            "Confirm the save control".to_string(),
            "--success".to_string(),
            "Save profile is rendered".to_string(),
            "--browser-driver".to_string(),
            "standalone".to_string(),
            "--browser-executable".to_string(),
            browser_arg.clone(),
            "--command-timeout-ms".to_string(),
            "60000".to_string(),
            "--idle-timeout-ms".to_string(),
            "30000".to_string(),
            "--json".to_string(),
        ],
    );
    assert_process_success("start adopt session", &started);

    let navigate =
        serde_json::to_string(&Action::Navigate { url: page.clone() }).expect("navigate JSON");
    let opened = agent(
        temp.path(),
        &[
            "act".to_string(),
            "--session".to_string(),
            session_name.clone(),
            "--action-json".to_string(),
            navigate,
            "--json".to_string(),
        ],
    );
    assert_process_success("record navigate", &opened);

    let assert_label = serde_json::to_string(&Action::Assert {
        expectation: Expectation::RenderedText {
            target: Target::Role {
                role: "button".to_string(),
                name: "Save profile".to_string(),
            },
            value: "Save profile".to_string(),
        },
    })
    .expect("assertion JSON");
    let confirmed = agent(
        temp.path(),
        &[
            "act".to_string(),
            "--session".to_string(),
            session_name.clone(),
            "--action-json".to_string(),
            assert_label,
            "--json".to_string(),
        ],
    );
    assert_process_success("record rendered-text assertion", &confirmed);

    let finished = agent(
        temp.path(),
        &[
            "finish".to_string(),
            "--session".to_string(),
            session_name.clone(),
            "--status".to_string(),
            "passed".to_string(),
            "--summary".to_string(),
            "Save profile stayed visible".to_string(),
            "--json".to_string(),
        ],
    );
    assert_process_success("finish adopt session", &finished);
    session.armed = false;
    capture("finish.json", &finished.stdout);

    let session_root = temp
        .path()
        .join(".a3s-test")
        .join("agent-sessions")
        .join(&session_name);
    let report_path = session_root.join("report.json");
    assert!(
        report_path.is_file(),
        "finish must write the session report"
    );
    assert!(
        acl_files(&session_root).is_empty(),
        "finish must not write an ACL suite: {:?}",
        acl_files(&session_root)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report_path).expect("read report")).expect("report JSON");
    let candidate = report["acl_candidate"]
        .as_str()
        .expect("verified session candidate");
    assert!(candidate.contains("rendered_text"));
    assert!(candidate.contains("Save profile"));
    assert!(!candidate.contains("@e"));
    capture(
        "report.json",
        &serde_json::to_vec_pretty(&report).expect("pretty report"),
    );

    let suite_path = temp.path().join("save-profile.acl");
    let adopted = run_at(
        temp.path(),
        &[
            "adopt",
            "--report",
            report_path.to_str().expect("report path"),
            "--output",
            suite_path.to_str().expect("suite path"),
            "--name",
            "save-profile",
            "--json",
        ],
    );
    assert_process_success("adopt finished session", &adopted);
    capture("adopt.json", &adopted.stdout);
    let written = fs::read(&suite_path).expect("adopted suite");
    capture("save-profile.acl", &written);
    let written_text = String::from_utf8(written).expect("UTF-8 suite");
    let suite = TestSuite::from_acl(&written_text).expect("parse adopted suite");
    assert_eq!(suite.name, "save-profile");
    assert!(written_text.contains(&page));
    assert!(!written_text.contains("long_press"));
    assert!(!written_text.contains("reload"));
    assert!(!written_text.contains("network_fulfill"));

    let probe = TcpListener::bind("127.0.0.1:0").expect("provider probe");
    let probe_addr = probe.local_addr().expect("probe address");
    probe.set_nonblocking(true).expect("nonblocking probe");
    let endpoint = format!("http://{probe_addr}/v1");
    fs::write(
        temp.path().join("provider.acl"),
        format!(
            "agent_run \"decoy\" {{\n    provider {{\n        endpoint = \"{endpoint}\"\n    }}\n}}\n"
        ),
    )
    .expect("decoy provider file");

    let runtime_before = private_runtime_directories();
    let passed = browser_command(temp.path())
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
        .current_dir(temp.path())
        .env("A3S_TEST_LLM_ENDPOINT", &endpoint)
        .env("A3S_TEST_PROVIDER_ENDPOINT", &endpoint)
        .env("OPENAI_BASE_URL", &endpoint)
        .output()
        .expect("run adopted suite");
    assert_process_success("run adopted suite", &passed);
    capture("run-pass.json", &passed.stdout);
    let passed_report: serde_json::Value =
        serde_json::from_slice(&passed.stdout).expect("pass report");
    assert_eq!(passed_report["status"], "passed");
    assert_eq!(passed_report["scenarios"][0]["status"], "passed");
    assert!(passed_report["scenarios"][0]["cleanup_error"].is_null());
    let passed_stderr = String::from_utf8_lossy(&passed.stderr);
    assert!(
        !passed_stderr.contains("provider HTTP"),
        "run stderr mentioned a provider client: {passed_stderr}"
    );

    let renamed = written_text.replace("/adopt.html", "/adopt-renamed.html");
    let renamed_path = temp.path().join("save-profile-renamed.acl");
    fs::write(&renamed_path, &renamed).expect("renamed suite");
    let stale = browser_command(temp.path())
        .args([
            "run",
            renamed_path.to_str().expect("renamed suite"),
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
        .current_dir(temp.path())
        .env("A3S_TEST_LLM_ENDPOINT", &endpoint)
        .env("A3S_TEST_PROVIDER_ENDPOINT", &endpoint)
        .env("OPENAI_BASE_URL", &endpoint)
        .output()
        .expect("run stale suite");
    assert_eq!(
        stale.status.code(),
        Some(1),
        "stale run should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&stale.stdout),
        String::from_utf8_lossy(&stale.stderr)
    );
    capture("run-stale.json", &stale.stdout);
    let stale_report: serde_json::Value =
        serde_json::from_slice(&stale.stdout).expect("stale report");
    assert_eq!(stale_report["status"], "failed");
    let stale_error = failed_step_error(&stale_report);
    assert_eq!(stale_error["code"], "test.driver.web.target_not_found");
    assert_eq!(stale_error["re_recordable"], true);
    assert!(
        !stale_error["code"]
            .as_str()
            .unwrap_or_default()
            .starts_with("test.assert."),
        "a missing locator must not be an assertion mismatch: {stale_error}"
    );

    let ambiguous = written_text.replace("/adopt.html", "/adopt-ambiguous.html");
    let ambiguous_path = temp.path().join("save-profile-ambiguous.acl");
    fs::write(&ambiguous_path, &ambiguous).expect("ambiguous suite");
    let ambiguous_run = browser_command(temp.path())
        .args([
            "run",
            ambiguous_path.to_str().expect("ambiguous suite"),
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
        .current_dir(temp.path())
        .output()
        .expect("run ambiguous suite");
    assert_eq!(ambiguous_run.status.code(), Some(1), "{ambiguous_run:?}");
    capture("run-ambiguous.json", &ambiguous_run.stdout);
    let ambiguous_report: serde_json::Value =
        serde_json::from_slice(&ambiguous_run.stdout).expect("ambiguous report");
    let ambiguous_error = failed_step_error(&ambiguous_report);
    assert_eq!(ambiguous_error["code"], "test.driver.web.target_ambiguous");
    assert_eq!(ambiguous_error["re_recordable"], true);

    let mismatch = written_text.replace("/adopt.html", "/adopt-mismatch.html");
    let mismatch_path = temp.path().join("save-profile-mismatch.acl");
    fs::write(&mismatch_path, &mismatch).expect("mismatch suite");
    let mismatch_run = browser_command(temp.path())
        .args([
            "run",
            mismatch_path.to_str().expect("mismatch suite"),
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
        .current_dir(temp.path())
        .output()
        .expect("run assertion mismatch");
    assert_eq!(mismatch_run.status.code(), Some(1), "{mismatch_run:?}");
    capture("run-mismatch.json", &mismatch_run.stdout);
    let mismatch_report: serde_json::Value =
        serde_json::from_slice(&mismatch_run.stdout).expect("mismatch report");
    let mismatch_error = failed_step_error(&mismatch_report);
    assert_eq!(mismatch_error["code"], "test.assert.rendered_text");
    assert_ne!(mismatch_error["re_recordable"].as_bool(), Some(true));

    match probe.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        other => panic!("a3s-test run contacted a model provider: {other:?}"),
    }
    assert_no_new_private_runtime_directories(&runtime_before);
}
