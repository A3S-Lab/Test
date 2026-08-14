mod admission;
mod artifacts;
mod model;
mod persistence;
mod runtime;
mod service;

pub use admission::{AdmittedRemoteFile, AdmittedRemoteJob};
pub use artifacts::*;
pub use model::*;
pub use service::*;

use schemars::Schema;
use serde::Serialize;

pub const REMOTE_WORKER_PROTOCOL: &str = "a3s.test.remote-worker/2";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RemoteWorkerProtocolInvariants {
    pub transport_authentication_required: bool,
    pub tls_termination_external: bool,
    pub external_image_identity_required: bool,
    pub request_cannot_select_executables: bool,
    pub deadline_and_lease_required: bool,
    pub idempotent_dispatch_required: bool,
    pub scenario_selection_digest_bound: bool,
    pub transports_artifacts: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RemoteWorkerProtocolSchema {
    pub protocol: &'static str,
    pub invariants: RemoteWorkerProtocolInvariants,
    pub request_schema: Schema,
    pub response_schema: Schema,
    pub descriptor_schema: Schema,
}

#[must_use]
pub fn remote_worker_protocol_schema() -> RemoteWorkerProtocolSchema {
    RemoteWorkerProtocolSchema {
        protocol: REMOTE_WORKER_PROTOCOL,
        invariants: RemoteWorkerProtocolInvariants {
            transport_authentication_required: true,
            tls_termination_external: true,
            external_image_identity_required: true,
            request_cannot_select_executables: true,
            deadline_and_lease_required: true,
            idempotent_dispatch_required: true,
            scenario_selection_digest_bound: true,
            transports_artifacts: false,
        },
        request_schema: schemars::schema_for!(RemoteWorkerRequest),
        response_schema: schemars::schema_for!(RemoteWorkerResponse),
        descriptor_schema: schemars::schema_for!(RemoteWorkerDescriptor),
    }
}
