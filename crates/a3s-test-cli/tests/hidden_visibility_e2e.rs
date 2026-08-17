mod support;

use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use support::assertion_stability::run_hidden_visibility_e2e;
use support::browser_process::{
    assert_no_new_private_runtime_directories, private_runtime_directories,
};
use support::web_fixture::WebFixture;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_runs_hidden_visibility_suite() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping hidden visibility E2E");
        return;
    };
    assert!(
        browser.is_file(),
        "browser executable does not exist: {browser:?}"
    );
    let version = Command::new(&browser)
        .arg("--version")
        .output()
        .expect("probe standalone browser version");
    assert!(version.status.success(), "browser version probe failed");
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("0.26."),
        "real E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start Web fixture");
    let fixture_address = fixture.address();
    let temp = tempfile::tempdir().expect("temporary hidden visibility E2E workspace");
    let runtime_directories_before = private_runtime_directories();

    run_hidden_visibility_e2e(&binary(), &browser, &fixture.origin(), temp.path());

    assert_no_new_private_runtime_directories(&runtime_directories_before);
    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&fixture_address, Duration::from_millis(250)).is_err(),
        "hidden visibility fixture listener must be closed on drop"
    );
}
