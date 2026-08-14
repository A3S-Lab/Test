//! Typed scheduling evidence and remote execution for A3S Test workers.

mod distributed;
mod gui;
mod remote;

pub use distributed::*;
pub use gui::*;
pub use remote::*;

use a3s_test_driver_tui::{TuiBackend, TuiCapabilities};
use a3s_test_driver_web::BrowserCapabilities;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const WORKER_CAPABILITY_PROTOCOL: &str = "a3s.test.worker-capabilities/2";
const MAX_PARALLEL_SCENARIOS: u16 = 64;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOperatingSystem {
    Linux,
    Macos,
    Windows,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerArchitecture {
    Aarch64,
    X86_64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRuntime {
    #[schemars(length(min = 1, max = 64))]
    pub implementation: String,
    #[schemars(length(min = 1, max = 128))]
    pub version: String,
    pub operating_system: WorkerOperatingSystem,
    pub architecture: WorkerArchitecture,
}

impl WorkerRuntime {
    fn current() -> Result<Self, WorkerCapabilityError> {
        Ok(Self {
            implementation: "a3s-test".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            operating_system: current_operating_system()?,
            architecture: current_architecture()?,
        })
    }

    fn validate(&self) -> Result<(), WorkerCapabilityError> {
        if self.implementation.trim().is_empty()
            || self.implementation.len() > 64
            || self.version.trim().is_empty()
            || self.version.len() > 128
        {
            return Err(inventory_error(
                "test.worker.inventory.runtime_invalid",
                "worker implementation and version must be bounded and non-empty",
            ));
        }
        Version::parse(&self.version).map_err(|_| {
            inventory_error(
                "test.worker.inventory.runtime_invalid",
                "worker version must be semantic",
            )
        })?;
        Ok(())
    }
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum WorkerSurface {
    Web,
    Gui,
    Tui,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebExecutionMode {
    Headless,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "surface", rename_all = "lowercase", deny_unknown_fields)]
pub enum WorkerSurfaceCapability {
    Web {
        execution: WebExecutionMode,
        browser: BrowserCapabilities,
    },
    Gui {
        desktop: Box<WorkerGuiCapability>,
    },
    Tui {
        terminal: TuiCapabilities,
    },
}

impl WorkerSurfaceCapability {
    #[must_use]
    pub fn surface(&self) -> WorkerSurface {
        match self {
            Self::Web { .. } => WorkerSurface::Web,
            Self::Gui { .. } => WorkerSurface::Gui,
            Self::Tui { .. } => WorkerSurface::Tui,
        }
    }

    fn validate(&self) -> Result<(), WorkerCapabilityError> {
        let result = match self {
            Self::Web { browser, .. } => browser.validate(),
            Self::Gui { desktop } => return desktop.validate(),
            Self::Tui { terminal } => terminal.validate(),
        };
        result.map_err(|error| {
            inventory_error(
                "test.worker.inventory.surface_invalid",
                format!(
                    "{} surface capability is invalid: {error}",
                    self.surface_name()
                ),
            )
        })
    }

    fn surface_name(&self) -> &'static str {
        match self.surface() {
            WorkerSurface::Web => "Web",
            WorkerSurface::Gui => "GUI",
            WorkerSurface::Tui => "TUI",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerCapabilityInventory {
    #[schemars(schema_with = "worker_capability_protocol_field_schema")]
    pub protocol: String,
    pub runtime: WorkerRuntime,
    #[schemars(range(min = 1, max = 64))]
    pub max_parallel_scenarios: u16,
    #[schemars(length(min = 1, max = 3))]
    pub surfaces: Vec<WorkerSurfaceCapability>,
}

impl WorkerCapabilityInventory {
    pub fn local(
        max_parallel_scenarios: u16,
        mut surfaces: Vec<WorkerSurfaceCapability>,
    ) -> Result<Self, WorkerCapabilityError> {
        surfaces.sort_by_key(WorkerSurfaceCapability::surface);
        let inventory = Self {
            protocol: WORKER_CAPABILITY_PROTOCOL.to_string(),
            runtime: WorkerRuntime::current()?,
            max_parallel_scenarios,
            surfaces,
        };
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn validate(&self) -> Result<(), WorkerCapabilityError> {
        if self.protocol != WORKER_CAPABILITY_PROTOCOL {
            return Err(inventory_error(
                "test.worker.inventory.protocol_unsupported",
                format!("unsupported worker capability protocol {:?}", self.protocol),
            ));
        }
        self.runtime.validate()?;
        if !(1..=MAX_PARALLEL_SCENARIOS).contains(&self.max_parallel_scenarios) {
            return Err(inventory_error(
                "test.worker.inventory.parallelism_invalid",
                format!(
                    "maximum parallel scenarios must be between 1 and {MAX_PARALLEL_SCENARIOS}"
                ),
            ));
        }
        if self.surfaces.is_empty() {
            return Err(inventory_error(
                "test.worker.inventory.surface_required",
                "worker inventory must contain at least one surface",
            ));
        }
        if self.max_parallel_scenarios != 1
            && self
                .surfaces
                .iter()
                .any(|capability| capability.surface() == WorkerSurface::Gui)
        {
            return Err(inventory_error(
                "test.worker.inventory.gui_parallelism_invalid",
                "a GUI worker represents one exclusive desktop and must advertise exactly one parallel scenario",
            ));
        }
        for capability in &self.surfaces {
            if let WorkerSurfaceCapability::Tui { terminal } = capability {
                let backend_matches_target = matches!(
                    (self.runtime.operating_system, terminal.backend),
                    (
                        WorkerOperatingSystem::Linux | WorkerOperatingSystem::Macos,
                        TuiBackend::UnixPty
                    ) | (WorkerOperatingSystem::Windows, TuiBackend::WindowsConPty)
                );
                if !backend_matches_target {
                    return Err(inventory_error(
                        "test.worker.inventory.surface_target_mismatch",
                        "TUI backend does not match the worker operating system",
                    ));
                }
            }
            if let WorkerSurfaceCapability::Gui { desktop } = capability {
                if desktop.application.operating_system() != self.runtime.operating_system {
                    return Err(inventory_error(
                        "test.worker.inventory.surface_target_mismatch",
                        "GUI application identity does not match the worker operating system",
                    ));
                }
            }
            capability.validate()?;
        }
        for adjacent in self.surfaces.windows(2) {
            let left = adjacent[0].surface();
            let right = adjacent[1].surface();
            if left == right {
                return Err(inventory_error(
                    "test.worker.inventory.surface_duplicate",
                    "worker inventory cannot contain duplicate surfaces",
                ));
            }
            if left > right {
                return Err(inventory_error(
                    "test.worker.inventory.surface_order_invalid",
                    "worker surfaces must use canonical order",
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn gui_capability(&self) -> Option<&WorkerGuiCapability> {
        self.surfaces
            .iter()
            .find_map(|capability| match capability {
                WorkerSurfaceCapability::Gui { desktop } => Some(desktop.as_ref()),
                _ => None,
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerCapabilityAuthority {
    SchedulingEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct WorkerCapabilityInvariants {
    pub self_reported: bool,
    pub authenticated: bool,
    pub authorizes_execution: bool,
    pub web_executable_probe_required: bool,
    pub compiled_tui_projection_required: bool,
    pub gui_host_probe_required: bool,
    pub gui_host_permissions_explicit: bool,
    pub gui_desktop_exclusive: bool,
    pub external_image_identity_required: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkerCapabilityProtocolSchema {
    pub protocol: &'static str,
    pub authority: WorkerCapabilityAuthority,
    pub invariants: WorkerCapabilityInvariants,
    pub inventory_schema: Schema,
}

#[must_use]
pub fn worker_capability_protocol_schema() -> WorkerCapabilityProtocolSchema {
    WorkerCapabilityProtocolSchema {
        protocol: WORKER_CAPABILITY_PROTOCOL,
        authority: WorkerCapabilityAuthority::SchedulingEvidence,
        invariants: WorkerCapabilityInvariants {
            self_reported: true,
            authenticated: false,
            authorizes_execution: false,
            web_executable_probe_required: true,
            compiled_tui_projection_required: true,
            gui_host_probe_required: true,
            gui_host_permissions_explicit: true,
            gui_desktop_exclusive: true,
            external_image_identity_required: true,
        },
        inventory_schema: schemars::schema_for!(WorkerCapabilityInventory),
    }
}

fn worker_capability_protocol_field_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "const": WORKER_CAPABILITY_PROTOCOL
    })
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct WorkerCapabilityError {
    code: &'static str,
    message: String,
}

impl WorkerCapabilityError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }
}

fn inventory_error(code: &'static str, message: impl Into<String>) -> WorkerCapabilityError {
    WorkerCapabilityError {
        code,
        message: message.into(),
    }
}

fn current_operating_system() -> Result<WorkerOperatingSystem, WorkerCapabilityError> {
    match std::env::consts::OS {
        "linux" => Ok(WorkerOperatingSystem::Linux),
        "macos" => Ok(WorkerOperatingSystem::Macos),
        "windows" => Ok(WorkerOperatingSystem::Windows),
        target => Err(inventory_error(
            "test.worker.inventory.target_unsupported",
            format!("unsupported worker operating system {target:?}"),
        )),
    }
}

fn current_architecture() -> Result<WorkerArchitecture, WorkerCapabilityError> {
    match std::env::consts::ARCH {
        "aarch64" => Ok(WorkerArchitecture::Aarch64),
        "x86_64" => Ok(WorkerArchitecture::X86_64),
        target => Err(inventory_error(
            "test.worker.inventory.target_unsupported",
            format!("unsupported worker architecture {target:?}"),
        )),
    }
}
