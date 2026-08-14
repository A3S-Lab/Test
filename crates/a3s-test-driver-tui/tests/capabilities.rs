use a3s_test_driver_tui::{TuiBackend, TuiCapabilities, TuiFeature, TUI_CAPABILITY_PROTOCOL};

#[test]
fn compiled_projection_reports_the_reviewed_backend_features_and_limits() {
    let capabilities = TuiCapabilities::compiled().expect("supported TUI backend");

    assert_eq!(capabilities.protocol, TUI_CAPABILITY_PROTOCOL);
    #[cfg(unix)]
    assert_eq!(capabilities.backend, TuiBackend::UnixPty);
    #[cfg(windows)]
    assert_eq!(capabilities.backend, TuiBackend::WindowsConPty);
    for feature in [
        TuiFeature::AlternateScreen,
        TuiFeature::KeyChords,
        TuiFeature::OwnedProcessTree,
        TuiFeature::Paste,
        TuiFeature::RegexWaits,
        TuiFeature::Resize,
        TuiFeature::SemanticViewport,
        TuiFeature::TerminalRecording,
        TuiFeature::TextWaits,
    ] {
        assert!(capabilities.features.contains(&feature), "{feature:?}");
    }
    assert_eq!(capabilities.limits.max_columns, 1_000);
    assert_eq!(capabilities.limits.max_rows, 500);
    assert_eq!(capabilities.limits.max_scrollback_rows, 10_000);
    assert_eq!(capabilities.limits.max_output_bytes, 16 * 1024 * 1024);
    assert_eq!(capabilities.limits.max_terminal_cells, 2_000_000);
}

#[test]
fn capability_projection_has_a_strict_generated_schema() {
    let schema = serde_json::to_value(schemars::schema_for!(TuiCapabilities))
        .expect("TUI capability schema");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["protocol"]["const"],
        TUI_CAPABILITY_PROTOCOL
    );

    let mut value = serde_json::to_value(TuiCapabilities::compiled().expect("capabilities"))
        .expect("capabilities JSON");
    value
        .as_object_mut()
        .expect("capability object")
        .insert("guessed".to_string(), serde_json::Value::Bool(true));
    serde_json::from_value::<TuiCapabilities>(value).expect_err("unknown field must fail");
}
