use std::collections::BTreeSet;

use a3s_test_driver_gui::{GuiHostPermissionGrant, GuiHostPermissionSource};
use a3s_test_driver_tui::{TuiBackend, TuiCapabilities};
use a3s_test_driver_web::{BrowserCapabilities, BrowserIntegration, WebCapability};
use a3s_test_worker::{
    worker_capability_protocol_schema, WebExecutionMode, WorkerCapabilityInventory,
    WorkerGuiApplication, WorkerGuiCapability, WorkerGuiEndpoint, WorkerGuiPerception,
    WorkerGuiTarget, WorkerSurface, WorkerSurfaceCapability, WORKER_CAPABILITY_PROTOCOL,
};

fn standalone_web() -> WorkerSurfaceCapability {
    WorkerSurfaceCapability::Web {
        execution: WebExecutionMode::Headless,
        browser: BrowserCapabilities {
            integration: BrowserIntegration::Standalone,
            version: "0.26.0".to_string(),
            protocol_revision: 8,
            features: [
                WebCapability::Accessibility,
                WebCapability::Console,
                WebCapability::ContextClicks,
                WebCapability::Dialogs,
                WebCapability::DomainContainment,
                WebCapability::Downloads,
                WebCapability::DragAndDrop,
                WebCapability::ElementInteractions,
                WebCapability::FormControls,
                WebCapability::Frames,
                WebCapability::Har,
                WebCapability::MouseWheel,
                WebCapability::NetworkRoutes,
                WebCapability::PageErrors,
                WebCapability::Screenshots,
                WebCapability::SyntheticMicrophone,
                WebCapability::Tabs,
                WebCapability::Trace,
                WebCapability::Uploads,
                WebCapability::Video,
                WebCapability::Viewport,
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            page_context_protocol: None,
        },
    }
}

fn tui() -> WorkerSurfaceCapability {
    WorkerSurfaceCapability::Tui {
        terminal: TuiCapabilities::compiled().expect("compiled TUI capabilities"),
    }
}

fn gui() -> WorkerSurfaceCapability {
    let host_permissions = GuiHostPermissionGrant::required(GuiHostPermissionSource::DriverDaemon);
    let application = match std::env::consts::OS {
        "macos" => WorkerGuiApplication::MacosBundle {
            bundle_id: "com.example.Editor".to_string(),
        },
        "windows" => WorkerGuiApplication::WindowsExecutable {
            path: "C:/Program Files/Example/editor.exe".to_string(),
            expected_publisher: Some("Example, Inc.".to_string()),
        },
        _ => WorkerGuiApplication::LinuxDesktop {
            desktop_id: "com.example.Editor".to_string(),
        },
    };
    WorkerSurfaceCapability::Gui {
        desktop: Box::new(WorkerGuiCapability {
            profile_id: "desktop-primary".to_string(),
            compatibility_profile: "macos-installed-daemon".to_string(),
            endpoint: WorkerGuiEndpoint::InstalledDaemon,
            perception: WorkerGuiPerception::Semantic,
            target: WorkerGuiTarget::Launch,
            application,
            cua_driver_version: "0.10.0".to_string(),
            mcp_protocol: "2025-06-18".to_string(),
            capability_vocabulary: "cua.capabilities/1".to_string(),
            tools_schema: "cua.tools/1".to_string(),
            configuration_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            policy_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            host_permission_digest: host_permissions.digest(),
            host_permissions,
        }),
    }
}

#[test]
fn local_inventory_is_versioned_unique_and_stably_sorted() {
    let inventory = WorkerCapabilityInventory::local(4, vec![tui(), standalone_web()])
        .expect("local inventory");

    assert_eq!(inventory.protocol, WORKER_CAPABILITY_PROTOCOL);
    assert_eq!(inventory.max_parallel_scenarios, 4);
    assert_eq!(inventory.surfaces[0].surface(), WorkerSurface::Web);
    assert_eq!(inventory.surfaces[1].surface(), WorkerSurface::Tui);
    inventory.validate().expect("valid inventory");

    let encoded = serde_json::to_string(&inventory).expect("serialize inventory");
    let decoded: WorkerCapabilityInventory =
        serde_json::from_str(&encoded).expect("deserialize inventory");
    assert_eq!(decoded, inventory);
}

#[test]
fn inventory_rejects_ambiguous_or_unbounded_scheduling_claims() {
    let duplicate =
        WorkerCapabilityInventory::local(1, vec![tui(), tui()]).expect_err("duplicate surface");
    assert_eq!(duplicate.code(), "test.worker.inventory.surface_duplicate");

    for limit in [0, 65] {
        let error = WorkerCapabilityInventory::local(limit, vec![tui()])
            .expect_err("invalid parallel limit");
        assert_eq!(error.code(), "test.worker.inventory.parallelism_invalid");
    }
}

#[test]
fn inventory_requires_exclusive_gui_slots_and_exact_permission_evidence() {
    let error = WorkerCapabilityInventory::local(2, vec![gui()])
        .expect_err("one desktop cannot run concurrent GUI scenarios");
    assert_eq!(
        error.code(),
        "test.worker.inventory.gui_parallelism_invalid"
    );

    let mut inventory =
        WorkerCapabilityInventory::local(1, vec![gui()]).expect("exclusive GUI inventory");
    assert_eq!(inventory.surfaces[0].surface(), WorkerSurface::Gui);
    let WorkerSurfaceCapability::Gui { desktop } = &mut inventory.surfaces[0] else {
        panic!("GUI capability");
    };
    desktop.host_permission_digest =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string();
    let error = inventory
        .validate()
        .expect_err("permission digest drift must fail");
    assert_eq!(
        error.code(),
        "test.worker.inventory.host_permission_digest_mismatch"
    );
}

#[test]
fn inventory_rejects_a_tui_backend_that_contradicts_the_worker_target() {
    let mut inventory = WorkerCapabilityInventory::local(1, vec![tui()]).expect("inventory");
    let WorkerSurfaceCapability::Tui { terminal } = &mut inventory.surfaces[0] else {
        panic!("expected TUI capability");
    };
    terminal.backend = match terminal.backend {
        TuiBackend::UnixPty => TuiBackend::WindowsConPty,
        TuiBackend::WindowsConPty => TuiBackend::UnixPty,
    };

    let error = inventory.validate().expect_err("mismatched TUI backend");
    assert_eq!(
        error.code(),
        "test.worker.inventory.surface_target_mismatch"
    );
}

#[test]
fn inventory_wire_shape_rejects_unknown_fields() {
    let inventory = WorkerCapabilityInventory::local(1, vec![tui()]).expect("inventory");
    let mut value = serde_json::to_value(inventory).expect("inventory JSON");
    value
        .as_object_mut()
        .expect("inventory object")
        .insert("trusted".to_string(), serde_json::Value::Bool(true));

    let error = serde_json::from_value::<WorkerCapabilityInventory>(value)
        .expect_err("unknown field must fail");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn schema_states_the_inventory_authority_and_external_identity_boundary() {
    let protocol = worker_capability_protocol_schema();
    assert_eq!(protocol.protocol, WORKER_CAPABILITY_PROTOCOL);
    assert!(protocol.invariants.self_reported);
    assert!(!protocol.invariants.authenticated);
    assert!(!protocol.invariants.authorizes_execution);
    assert!(protocol.invariants.web_executable_probe_required);
    assert!(protocol.invariants.compiled_tui_projection_required);
    assert!(protocol.invariants.gui_host_probe_required);
    assert!(protocol.invariants.gui_host_permissions_explicit);
    assert!(protocol.invariants.gui_desktop_exclusive);
    assert!(protocol.invariants.external_image_identity_required);

    let schema = serde_json::to_value(protocol.inventory_schema).expect("schema JSON");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["protocol"]["const"],
        WORKER_CAPABILITY_PROTOCOL
    );
    assert_eq!(schema["properties"]["max_parallel_scenarios"]["minimum"], 1);
    assert_eq!(
        schema["properties"]["max_parallel_scenarios"]["maximum"],
        64
    );
    assert_eq!(schema["properties"]["surfaces"]["minItems"], 1);
    assert_eq!(schema["properties"]["surfaces"]["maxItems"], 3);
}
