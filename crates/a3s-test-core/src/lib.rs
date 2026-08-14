//! Typed test specifications and surface-driver contracts for A3S Test.

mod contract;
mod design_audit;
mod driver;
mod error;
mod manifest;
mod model;
mod page_context;
mod reconcile;

pub use contract::{
    AdmittedProvenance, ContractCitation, ContractContext, ContractElement, ContractMode,
    ContractProvenanceKind, ContractProvenanceStatus, ContractSeverity, ContractVariant,
    SurfaceContract, SurfaceContractDraft,
};
pub use design_audit::{
    DesignAuditAuthority, DesignAuditDimension, DesignAuditFinding, DesignAuditNormalizedRegion,
    DesignAuditPriority, DesignAuditProvenance, DesignAuditProviderIdentity, DesignAuditReport,
    DesignAuditTarget, DesignAuditUsage, DESIGN_AUDIT_REPORT_PROTOCOL,
};

pub use driver::{
    DriverSession, PageContextInspectRequest, PageContextInspectScope, ScenarioContext,
    SurfaceDriver,
};
pub use error::{DriverError, SpecError};
pub use model::{
    Action, CaptureOperation, DialogOperation, Evidence, Expectation, FrameTarget,
    GroundingScreenshot, LoadState, ModifierKey, NetworkRoute, PageContextComponent,
    PageContextGeometry, PageContextLocator, PageContextNode, PageContextNodeState,
    PageContextObservation, PageContextPage, PageContextPoint, PageContextPosition,
    PageContextRect, PageContextSize, PageContextSnapshot, PageContextSource, PageContextTheme,
    PageContextViewport, PageContextVisualViewport, RepairAclProof, RepairActor, RepairAttempt,
    RepairBatch, RepairBatchItemResult, RepairBatchStatus, RepairCheckResult, RepairCheckStatus,
    RepairEvidenceBundle, RepairEvidencePhase, RepairEvidenceRequest, RepairFinding,
    RepairHumanAction, RepairHumanActionKind, RepairIntent, RepairLayoutCanvas, RepairLayoutIntent,
    RepairRelation, RepairSeverity, RepairStatus, RepairStatusEvent, RepairTarget,
    RepairTargetKind, RepairThreadMessage, RepairVerification, StepOutput, Surface,
    SurfaceObservation, TabOperation, Target, TestScenario, TestStep, TestSuite, VideoOperation,
    WaitCondition, ACTION_PROTOCOL_REVISION, PAGE_CONTEXT_PROTOCOL, REPAIR_PROTOCOL,
};
pub use page_context::{
    action_uses_observation_target, action_uses_page_context_ref, bind_page_context_refs,
    preferred_page_context_target, resolve_page_context_refs, PageContextBindings,
    PageContextRefError,
};
pub use reconcile::{
    ContractFinding, ContractMatch, ContractMatchStrategy, ContractOutcome, ContractReport,
};
