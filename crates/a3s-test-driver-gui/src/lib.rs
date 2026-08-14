//! A3S CUA adapter for GUI testing.
//!
//! The crate owns the typed compatibility boundary between A3S Test and the
//! external CUA daemon. Platform automation details stay behind that boundary.

mod admission;
mod api;
mod artifact;
mod compatibility;
mod config;
mod host;
mod lifecycle;
mod process;
mod protocol;
mod semantic;
mod session;
mod transport;

pub use admission::{CuaCapabilities, CuaClient, CuaTool};
pub use compatibility::{
    CuaCompatibility, CuaToolRequirement, GuiCertificationMatrix, GuiCertificationStatus,
    GuiEndpointMode, GuiExecutionProfile, GuiPlatform,
};
pub use config::{
    ApplicationIdentity, AttachSpec, CuaEndpoint, GuiAppTarget, GuiCaptureScope, GuiDriverConfig,
    GuiProfile, LaunchSpec, WindowSelector,
};
pub use host::{
    GuiHostPermission, GuiHostPermissionGrant, GuiHostPermissionSource, GuiHostProbe,
    GUI_HOST_PERMISSION_PROTOCOL,
};
pub use process::terminate_active_cua_processes;
pub use protocol::{
    CuaToolAnnotations, CuaToolDefinition, JsonRpcError, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse,
};
pub use session::{GuiDriver, GuiSession};
pub use transport::{
    CuaTransport, CuaTransportError, CuaTransportErrorKind, CuaTransportFactory, StdioCuaTransport,
    StdioCuaTransportFactory,
};
