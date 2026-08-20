use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

fn write_vite_project(root: &Path, include_testkit: bool) {
    let testkit = if include_testkit {
        r#", "@a3s-lab/testkit": "^0.4.0""#
    } else {
        ""
    };
    fs::write(
        root.join("package.json"),
        format!(
            r#"{{
  "name": "checkout-app",
  "scripts": {{ "dev": "vite" }},
  "devDependencies": {{ "vite": "^7.0.0"{testkit} }}
}}
"#,
        ),
    )
    .expect("package.json");
    fs::write(root.join("package-lock.json"), "{}\n").expect("package lock");
}

#[test]
fn help_exposes_the_workspace_vibe_loop_commands() {
    let output = Command::new(binary())
        .arg("--help")
        .output()
        .expect("run CLI help");

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("  init "), "{stdout}");
    assert!(stdout.contains("  doctor "), "{stdout}");
    assert!(stdout.contains("  dev "), "{stdout}");
}

#[test]
fn init_detects_vite_and_writes_a_typed_acl_profile() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_vite_project(temp.path(), true);

    let output = Command::new(binary())
        .args(["init", "--root", temp.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run workspace init");

    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("workspace init JSON");
    assert_eq!(result["protocol"], "a3s.test.project-init/1");
    assert_eq!(result["status"], "initialized");
    assert_eq!(result["project"]["id"], "checkout-app");
    assert_eq!(result["project"]["framework"], "vite");
    assert_eq!(result["project"]["package_manager"], "npm");
    assert_eq!(result["project"]["url"], "http://127.0.0.1:5173/");

    let config = temp.path().join(".a3s-test/project.acl");
    let source = fs::read_to_string(&config).expect("generated project profile");
    assert!(source.contains("project \"checkout-app\""), "{source}");
    assert!(source.contains("version = 1"), "{source}");
    assert!(source.contains("executable = \"npm\""), "{source}");
    assert!(source.contains("args = [\"run\", \"dev\"]"), "{source}");
    assert!(
        source.contains("url = \"http://127.0.0.1:5173/\""),
        "{source}"
    );
    assert!(source.contains("required = true"), "{source}");
}

#[test]
fn init_refuses_to_replace_an_existing_profile_without_force() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_vite_project(temp.path(), true);
    let config_dir = temp.path().join(".a3s-test");
    fs::create_dir(&config_dir).expect("config directory");
    let config = config_dir.join("project.acl");
    fs::write(&config, "keep me\n").expect("existing config");

    let output = Command::new(binary())
        .args(["init", "--root", temp.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run workspace init");

    assert!(!output.status.success(), "{output:?}");
    assert_eq!(fs::read_to_string(config).unwrap(), "keep me\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"), "{stderr}");
    assert!(stderr.contains("--force"), "{stderr}");
}

#[test]
fn doctor_returns_a_machine_readable_fix_for_a_missing_testkit() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_vite_project(temp.path(), false);
    let init = Command::new(binary())
        .args(["init", "--root", temp.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run workspace init");
    assert!(init.status.success(), "{init:?}");

    let output = Command::new(binary())
        .args(["doctor", "--root", temp.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run workspace doctor");

    assert!(!output.status.success(), "{output:?}");
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("workspace doctor JSON");
    assert_eq!(result["protocol"], "a3s.test.project-doctor/1");
    assert_eq!(result["status"], "failed");
    let checks = result["checks"].as_array().expect("doctor checks");
    let testkit = checks
        .iter()
        .find(|check| check["id"] == "testkit.dependency")
        .expect("Test Kit dependency check");
    assert_eq!(testkit["status"], "failed");
    assert_eq!(
        testkit["fix"], "npm install --save-dev @a3s-lab/testkit@0.6.1",
        "{testkit}"
    );
}

#[test]
fn doctor_distinguishes_a_declared_but_not_installed_testkit() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_vite_project(temp.path(), true);
    let init = Command::new(binary())
        .args(["init", "--root", temp.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run workspace init");
    assert!(init.status.success(), "{init:?}");

    let output = Command::new(binary())
        .args(["doctor", "--root", temp.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run workspace doctor");

    assert!(!output.status.success(), "{output:?}");
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("workspace doctor JSON");
    let checks = result["checks"].as_array().expect("doctor checks");
    let installed = checks
        .iter()
        .find(|check| check["id"] == "testkit.installed")
        .expect("installed Test Kit check");
    assert_eq!(installed["status"], "failed");
    assert!(
        installed["fix"]
            .as_str()
            .is_some_and(|fix| fix.contains("npm install")),
        "{installed}"
    );
}
