mod files;
mod model;
mod payload;
pub(super) mod persistence;
pub(super) mod retention;
pub(super) mod service;

pub use model::*;

use schemars::Schema;
use serde::Serialize;

pub const REMOTE_ARTIFACT_PROTOCOL: &str = "a3s.test.remote-artifacts/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RemoteArtifactProtocolInvariants {
    pub transport_authentication_required: bool,
    pub deployment_owned_retention: bool,
    pub digest_bound_reads: bool,
    pub bounded_pagination: bool,
    pub bounded_chunks: bool,
    pub no_arbitrary_paths: bool,
    pub transports_artifacts: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RemoteArtifactProtocolSchema {
    pub protocol: &'static str,
    pub invariants: RemoteArtifactProtocolInvariants,
    pub request_schema: Schema,
    pub response_schema: Schema,
    pub descriptor_schema: Schema,
}

#[must_use]
pub fn remote_artifact_protocol_schema() -> RemoteArtifactProtocolSchema {
    RemoteArtifactProtocolSchema {
        protocol: REMOTE_ARTIFACT_PROTOCOL,
        invariants: RemoteArtifactProtocolInvariants {
            transport_authentication_required: true,
            deployment_owned_retention: true,
            digest_bound_reads: true,
            bounded_pagination: true,
            bounded_chunks: true,
            no_arbitrary_paths: true,
            transports_artifacts: true,
        },
        request_schema: schemars::schema_for!(RemoteArtifactRequest),
        response_schema: schemars::schema_for!(RemoteArtifactResponse),
        descriptor_schema: schemars::schema_for!(RemoteArtifactDescriptor),
    }
}
