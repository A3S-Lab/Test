//! Typed test specifications and surface-driver contracts for A3S Test.

mod adopt;
mod candidate;
mod contract;
mod design_audit;
mod driver;
mod error;
mod manifest;
mod model;
mod page_context;
mod page_context_diff;
mod page_context_observation;
mod reconcile;
mod secret;
mod source_mapping;
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

pub use adopt::{adopted_suite_acl, AdoptError};
pub use candidate::{verified_session_candidate, RecordedStep};
pub use driver::{
    DriverSession, PageContextInspectRequest, PageContextInspectScope, ScenarioContext,
    SurfaceDriver,
};
pub use error::{DriverError, SpecError};
pub use model::{
    admit_text_pattern, compare_pixels, decode_png_bounded, encode_png, paint_pixel_masks,
    pixel_baseline_decision, render_pixel_diff, text_pattern_matches, Action, AssertionMode,
    AssertionStability, CaptureOperation, CookieAssignment, DecodedPng, DialogOperation,
    ElementState, Evidence, Expectation, FrameTarget, GroundingScreenshot, LayoutRect,
    LayoutRelation, LoadState, ModifierKey, NamedViewport, NetworkRoute, OwnedApp,
    PageContextComponent, PageContextGeometry, PageContextLocator, PageContextNode,
    PageContextNodeState, PageContextObservation, PageContextPage, PageContextPoint,
    PageContextPosition, PageContextRect, PageContextSize, PageContextSnapshot, PageContextSource,
    PageContextSourceCandidate, PageContextSourceMapping, PageContextSourceOrigin,
    PageContextSourceRelation, PageContextTheme, PageContextViewport, PageContextVisualViewport,
    PixelBaselineDecision, PixelCompareFailure, PixelDecodeError, PixelExpectation, PixelLimits,
    PixelMask, PreviewHeader, RepairAclProof, RepairActor, RepairAttempt, RepairBatch,
    RepairBatchItemResult, RepairBatchStatus, RepairChange, RepairCheckResult, RepairCheckStatus,
    RepairDesignReference, RepairDesignReferenceImage, RepairDesignReferenceKind,
    RepairEvidenceBundle, RepairEvidencePhase, RepairEvidenceRequest, RepairFinding,
    RepairHumanAction, RepairHumanActionKind, RepairIntent, RepairLayoutCanvas, RepairLayoutIntent,
    RepairRelation, RepairSeverity, RepairStatus, RepairStatusEvent, RepairTarget,
    RepairTargetKind, RepairThreadMessage, RepairVerification, RepairVerificationExpansionReason,
    RepairVerificationScope, RepairVerificationSlice, ScrollDirection, SecretPurpose, StepOutput,
    Surface, SurfaceObservation, TabOperation, Target, TestScenario, TestStep, TestSuite,
    VideoOperation, ViewportCoverageComparison, WaitCondition, WaitMode, ACTION_PROTOCOL_REVISION,
    DEFAULT_ASSERTION_SAMPLE_INTERVAL_MS, DEFAULT_LONG_PRESS_MS, DEFAULT_PIXEL_THRESHOLD_MILLIS,
    DEFAULT_RESPONSE_WAIT_MS, DEFAULT_SCROLL_UNTIL_TIMEOUT_MS, MAX_ASSERTION_STABILITY_MS,
    MAX_ASSERTION_STABILITY_SAMPLES, MAX_LAYOUT_COORDINATE_ABS, MAX_LAYOUT_TOLERANCE_PX,
    MAX_LONG_PRESS_MS, MAX_PAGE_TEXT_BYTES, MAX_PIXEL_RATIO_MICROS, MAX_PIXEL_THRESHOLD_MILLIS,
    MAX_RENDERED_TEXT_ITEMS, MAX_REPAIR_CHECK_COMMAND_BYTES, MAX_REPAIR_CHECK_SUMMARY_BYTES,
    MAX_RESPONSE_WAIT_MS, MAX_SCROLL_UNTIL_TIMEOUT_MS, MAX_TEXT_PATTERN_CHARS,
    MAX_VIEWPORT_COVERAGE_PERCENT, MIN_ASSERTION_STABILITY_MS, MIN_LONG_PRESS_MS,
    MIN_RESPONSE_WAIT_MS, MIN_SCROLL_UNTIL_TIMEOUT_MS, PAGE_CONTEXT_PROTOCOL, REPAIR_PROTOCOL,
    REPAIR_VERIFICATION_SLICE_PROTOCOL, SCROLL_UNTIL_SAMPLE_INTERVAL_MS, SCROLL_UNTIL_STEP_PX,
    SOURCE_MAPPING_PROTOCOL,
};
pub use page_context::{
    action_uses_observation_target, action_uses_page_context_ref,
    bind_page_context_observation_refs, bind_page_context_refs, preferred_page_context_target,
    refresh_page_context_bindings, resolve_page_context_refs, validate_action_page_context_refs,
    PageContextBindings, PageContextRefError,
};
pub use page_context_diff::{
    PageContextDelta, PageContextDeltaStatus, PageContextDeltaValidationError,
    PageContextInvalidation, PAGE_CONTEXT_DIFF_PROTOCOL,
};
pub use reconcile::{
    ContractFinding, ContractMatch, ContractMatchStrategy, ContractOutcome, ContractReport,
};
pub use secret::{admit_secret_sink, redact_secret_text, SecretSink, SecretSinkError};
pub use source_mapping::SourceMappingValidationError;
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
