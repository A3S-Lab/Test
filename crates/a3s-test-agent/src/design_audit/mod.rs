mod model;
mod provider;
mod service;

pub(crate) const MAX_DESIGN_AUDIT_IMAGE_BYTES: u64 = 32 * 1_024 * 1_024;

pub use model::{
    DesignAuditImageAttachment, DesignAuditOptions, DesignAuditProviderRequest,
    DesignAuditProviderResponse, DesignAuditRequest,
};
pub use provider::DesignAuditProvider;
pub use service::DesignAuditService;
