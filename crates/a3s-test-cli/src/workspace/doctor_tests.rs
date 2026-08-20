use std::fs;
use std::path::PathBuf;

use serde_json::json;
use url::Url;

use super::*;
use crate::workspace::config::{
    BrowserProfile, DevServerProfile, ProjectBrowserDriver, TestKitProfile,
};

#[tokio::test]
async fn compatible_installed_testkit_versions_pass() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_testkit(temp.path(), "0.5.0");
    let profile = profile(temp.path().to_path_buf());
    let mut checks = Vec::new();

    check_testkit(&profile, Some(&declared_package()), &mut checks).await;

    let installed = check(&checks, "testkit.installed");
    assert_eq!(installed.status, CheckStatus::Passed);
    assert!(installed.summary.contains("0.5.0"));
    assert!(installed.summary.contains("passes the static"));
    assert!(installed.summary.contains("a3s-test dev"));
    assert!(installed.summary.contains("live protocol handshake"));
    assert!(!installed.summary.contains("installed and compatible"));
}

#[tokio::test]
async fn incompatible_installed_testkit_versions_fail_with_a_fix() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_testkit(temp.path(), "0.6.0");
    let profile = profile(temp.path().to_path_buf());
    let mut checks = Vec::new();

    check_testkit(&profile, Some(&declared_package()), &mut checks).await;

    let installed = check(&checks, "testkit.installed");
    assert_eq!(installed.status, CheckStatus::Failed);
    assert!(installed.summary.contains(">=0.4.0, <0.6.0"));
    assert_eq!(
        installed.fix.as_deref(),
        Some("npm install --save-dev @a3s-lab/testkit@0.5.0")
    );
}

fn check<'a>(checks: &'a [DoctorCheck], id: &str) -> &'a DoctorCheck {
    checks
        .iter()
        .find(|check| check.id == id)
        .expect("doctor check")
}

fn declared_package() -> serde_json::Value {
    json!({
        "scripts": { "dev": "vite" },
        "devDependencies": { "@a3s-lab/testkit": "^0.4.0" }
    })
}

fn write_testkit(root: &std::path::Path, version: &str) {
    let directory = root.join("node_modules/@a3s-lab/testkit");
    fs::create_dir_all(&directory).expect("Test Kit directory");
    fs::write(
        directory.join("package.json"),
        format!("{{\"name\":\"@a3s-lab/testkit\",\"version\":\"{version}\"}}\n"),
    )
    .expect("Test Kit metadata");
}

fn profile(root: PathBuf) -> ProjectProfile {
    ProjectProfile {
        id: "fixture".to_string(),
        config_path: root.join(".a3s-test/project.acl"),
        dev_server: DevServerProfile {
            executable: "npm".to_string(),
            arguments: vec!["run".to_string(), "dev".to_string()],
            working_directory: root.clone(),
            url: Url::parse("http://127.0.0.1:5173/").expect("URL"),
            startup_timeout_ms: 120_000,
            cleanup_timeout_ms: 10_000,
        },
        browser: BrowserProfile {
            driver: ProjectBrowserDriver::A3s,
            executable: None,
            session: "dev".to_string(),
            headed: true,
            command_timeout_ms: 25_000,
            idle_timeout_ms: 300_000,
        },
        testkit: TestKitProfile { required: true },
        root,
    }
}
