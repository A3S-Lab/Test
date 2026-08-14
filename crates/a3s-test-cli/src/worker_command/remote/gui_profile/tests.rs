use super::*;

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
