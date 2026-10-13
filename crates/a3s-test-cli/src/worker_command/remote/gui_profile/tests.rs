use super::*;
use std::num::NonZeroU32;

use a3s_test_driver_gui::GuiHostPermission;

fn profile_source(permission_source: &str, permissions: &str) -> String {
    format!(
        r#"gui_host "desktop-primary" {{
  endpoint = "installed_daemon"
  proxy_executable = "bin/cua-driver"
  policy_file = "policy.yaml"
  macos_bundle_id = "com.example.Editor"
  target = "launch"
  arguments = ["--safe-mode"]
  profile = "window_vision"
  permission_source = "{permission_source}"
  permissions = {permissions}
}}
"#
    )
}

#[test]
fn parses_an_explicit_deployment_owned_gui_profile() {
    let parsed = parse(
        &profile_source("driver_daemon", "[\"accessibility\", \"screen_recording\"]"),
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::from([OsString::from("A3S_TEST_WORKER_AUTHORIZATION_DESKTOP")]),
    )
    .expect("GUI profile");
    assert_eq!(parsed.id, "desktop-primary");
    assert_eq!(parsed.config.profile, GuiProfile::WindowVision);
    assert!(matches!(parsed.config.target, GuiAppTarget::Launch(_)));
    assert!(parsed
        .config
        .removed_environment
        .contains(&OsString::from("A3S_TEST_WORKER_AUTHORIZATION_DESKTOP")));
    assert_eq!(
        parsed.declared_permissions.permissions,
        [
            GuiHostPermission::Accessibility,
            GuiHostPermission::ScreenRecording,
        ]
    );
}

#[test]
fn rejects_implicit_or_wrongly_attributed_host_permissions() {
    let missing = profile_source("driver_daemon", "[\"accessibility\"]");
    let error = parse(
        &missing,
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::new(),
    )
    .expect_err("incomplete permission grant");
    assert!(error.to_string().contains("permissions"));

    let wrong_source = profile_source("host", "[\"accessibility\", \"screen_recording\"]");
    let error = parse(
        &wrong_source,
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::new(),
    )
    .expect_err("permission source mismatch");
    assert!(error.to_string().contains("ownership"));
}

#[test]
fn attach_admits_macos_process_name_for_unpackaged_identity() {
    let source = r#"gui_host "desktop-attach" {
  endpoint = "installed_daemon"
  proxy_executable = "bin/cua-driver"
  policy_file = "policy.yaml"
  macos_bundle_id = "dev.a3s.desktop"
  target = "attach"
  attach_pid = 4242
  macos_process_name = "A3S"
  profile = "semantic"
  permission_source = "driver_daemon"
  permissions = ["accessibility", "screen_recording"]
}
"#;
    let parsed = parse(
        source,
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::new(),
    )
    .expect("attach profile with process name");
    match &parsed.config.target {
        GuiAppTarget::Attach(spec) => {
            assert_eq!(spec.process_id.map(NonZeroU32::get), Some(4242));
            assert_eq!(spec.process_name.as_deref(), Some("A3S"));
        }
        other => panic!("expected attach target, got {other:?}"),
    }
    let (target, _, process_name, windows_process_name) =
        worker_target(&parsed.config.target).expect("worker target");
    assert_eq!(target, WorkerGuiTarget::Attach);
    assert_eq!(process_name.as_deref(), Some("A3S"));
    assert_eq!(windows_process_name, None);
}

#[test]
fn launch_rejects_macos_process_name() {
    let source = r#"gui_host "desktop-launch" {
  endpoint = "installed_daemon"
  proxy_executable = "bin/cua-driver"
  policy_file = "policy.yaml"
  macos_bundle_id = "com.example.Editor"
  target = "launch"
  macos_process_name = "Editor"
  profile = "semantic"
  permission_source = "driver_daemon"
  permissions = ["accessibility", "screen_recording"]
}
"#;
    let error = parse(
        source,
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::new(),
    )
    .expect_err("process name is attach-only");
    assert!(
        error.to_string().contains("macos_process_name"),
        "unexpected error: {error}"
    );
}

#[test]
fn parses_a_windows_installed_daemon_profile() {
    let source = r#"gui_host "desktop-primary" {
  endpoint = "installed_daemon"
  proxy_executable = "bin/cua-driver"
  policy_file = "policy.yaml"
  windows_executable = "C:/Program Files/Example/editor.exe"
  target = "attach"
  windows_process_name = "editor.exe"
  profile = "semantic"
  permission_source = "driver_daemon"
  permissions = ["accessibility"]
}
"#;
    let parsed = parse(
        source,
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::new(),
    )
    .expect("Windows GUI profile");
    match &parsed.config.target {
        GuiAppTarget::Attach(spec) => {
            assert!(matches!(
                spec.application,
                ApplicationIdentity::WindowsExecutable { .. }
            ));
            assert_eq!(spec.process_name.as_deref(), Some("editor.exe"));
        }
        other => panic!("expected attach target, got {other:?}"),
    }
    assert_eq!(
        parsed.declared_permissions.permissions,
        [GuiHostPermission::Accessibility]
    );
    let (_, application, macos_process_name, windows_process_name) =
        worker_target(&parsed.config.target).expect("worker target");
    assert!(matches!(
        application,
        WorkerGuiApplication::WindowsExecutable {
            expected_publisher: None,
            ..
        }
    ));
    assert_eq!(macos_process_name, None);
    assert_eq!(windows_process_name.as_deref(), Some("editor.exe"));
}

#[test]
fn rejects_a_windows_profile_that_claims_screen_recording_or_an_embedded_socket() {
    let screen_recording = r#"gui_host "desktop-primary" {
  endpoint = "installed_daemon"
  proxy_executable = "bin/cua-driver"
  policy_file = "policy.yaml"
  windows_executable = "C:/Program Files/Example/editor.exe"
  permission_source = "driver_daemon"
  permissions = ["accessibility", "screen_recording"]
}
"#;
    let error = parse(
        screen_recording,
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::new(),
    )
    .expect_err("Windows grant is accessibility only");
    assert!(error.to_string().contains("accessibility"), "{error}");

    let embedded = r#"gui_host "desktop-primary" {
  endpoint = "embedded_socket"
  proxy_executable = "bin/cua-driver"
  embedded_socket = "cua.sock"
  policy_file = "policy.yaml"
  windows_executable = "C:/Program Files/Example/editor.exe"
  permission_source = "host"
  permissions = ["accessibility"]
}
"#;
    let error = parse(
        embedded,
        Path::new("/workspace/gui-host.acl"),
        Duration::from_secs(30),
        BTreeSet::new(),
    )
    .expect_err("Windows embedded socket is unsupported");
    assert!(error.to_string().contains("installed_daemon"), "{error}");
}
