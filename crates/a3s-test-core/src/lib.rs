//! Typed test specifications and surface-driver contracts for A3S Test.

mod contract;
mod design_audit;
mod driver;
mod error;
mod manifest;
mod model;
mod page_context;
mod reconcile;
mod ui_understanding;

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
    Action, AssertionMode, AssertionStability, CaptureOperation, DialogOperation, ElementState,
    Evidence, Expectation, FrameTarget, GroundingScreenshot, LayoutRect, LayoutRelation, LoadState,
    ModifierKey, NetworkRoute, PageContextComponent, PageContextGeometry, PageContextLocator,
    PageContextNode, PageContextNodeState, PageContextObservation, PageContextPage,
    PageContextPoint, PageContextPosition, PageContextRect, PageContextSize, PageContextSnapshot,
    PageContextSource, PageContextTheme, PageContextViewport, PageContextVisualViewport,
    RepairAclProof, RepairActor, RepairAttempt, RepairBatch, RepairBatchItemResult,
    RepairBatchStatus, RepairCheckResult, RepairCheckStatus, RepairEvidenceBundle,
    RepairEvidencePhase, RepairEvidenceRequest, RepairFinding, RepairHumanAction,
    RepairHumanActionKind, RepairIntent, RepairLayoutCanvas, RepairLayoutIntent, RepairRelation,
    RepairSeverity, RepairStatus, RepairStatusEvent, RepairTarget, RepairTargetKind,
    RepairThreadMessage, RepairVerification, StepOutput, Surface, SurfaceObservation, TabOperation,
    Target, TestScenario, TestStep, TestSuite, VideoOperation, WaitCondition, WaitMode,
    ACTION_PROTOCOL_REVISION, DEFAULT_ASSERTION_SAMPLE_INTERVAL_MS, MAX_ASSERTION_STABILITY_MS,
    MAX_ASSERTION_STABILITY_SAMPLES, MAX_LAYOUT_COORDINATE_ABS, MAX_LAYOUT_TOLERANCE_PX,
    MAX_RENDERED_TEXT_ITEMS, MIN_ASSERTION_STABILITY_MS, PAGE_CONTEXT_PROTOCOL, REPAIR_PROTOCOL,
};
pub use page_context::{
    action_uses_observation_target, action_uses_page_context_ref,
    bind_page_context_observation_refs, bind_page_context_refs, preferred_page_context_target,
    resolve_page_context_refs, validate_action_page_context_refs, PageContextBindings,
    PageContextRefError,
};
pub use reconcile::{
    ContractFinding, ContractMatch, ContractMatchStrategy, ContractOutcome, ContractReport,
};
pub use ui_understanding::{
    UiAccessibilityStateChange, UiAnimationProfile, UiAnimationSource, UiAnimationTimeline,
    UiAnimationTimelineKind, UiAnimationTimelineSource, UiBaselineState, UiBoxEdges, UiBoxModel,
    UiBoxSizing, UiComponentCluster, UiContextScope, UiCoordinateSpace, UiCustomProperty,
    UiCustomPropertySource, UiEvidenceSourceKind, UiFlexLayout, UiGridLayout, UiLayoutEdge,
    UiLayoutEdgeRelation, UiLayoutGraph, UiLayoutNode, UiMotionProfile, UiObservedInteractionState,
    UiObservedToken, UiOverflowMetrics, UiResponsiveCondition, UiResponsiveConditionSource,
    UiStateDiff, UiStyleChange, UiStyleProfile, UiTextDirection, UiTransitionProfile,
    UiTruncationReason, UiTypographyToken, UiUnderstandingBudget, UiUnderstandingBudgetLimits,
    UiUnderstandingBudgetUsed, UiUnderstandingEvidence, UiUnderstandingSnapshot,
    UiUnderstandingValidationError, UiWritingMode, UI_UNDERSTANDING_PROTOCOL,
};
