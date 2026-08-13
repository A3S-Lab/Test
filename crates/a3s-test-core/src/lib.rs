//! Typed test specifications and surface-driver contracts for A3S Test.

mod driver;
mod error;
mod manifest;
mod model;

pub use driver::{
    DriverSession, PageContextInspectRequest, PageContextInspectScope, ScenarioContext,
    SurfaceDriver,
};
pub use error::{DriverError, SpecError};
pub use model::{
    Action, CaptureOperation, DialogOperation, Evidence, Expectation, FrameTarget, LoadState,
    ModifierKey, NetworkRoute, PageContextComponent, PageContextGeometry, PageContextLocator,
    PageContextNode, PageContextNodeState, PageContextObservation, PageContextPage,
    PageContextPoint, PageContextPosition, PageContextRect, PageContextSize, PageContextSnapshot,
    PageContextSource, PageContextTheme, PageContextViewport, PageContextVisualViewport,
    RepairAclProof, RepairActor, RepairAttempt, RepairBatch, RepairBatchItemResult,
    RepairBatchStatus, RepairCheckResult, RepairCheckStatus, RepairEvidenceBundle,
    RepairEvidencePhase, RepairEvidenceRequest, RepairFinding, RepairHumanAction,
    RepairHumanActionKind, RepairIntent, RepairRelation, RepairSeverity, RepairStatus,
    RepairStatusEvent, RepairTarget, RepairTargetKind, RepairThreadMessage, RepairVerification,
    StepOutput, Surface, SurfaceObservation, TabOperation, Target, TestScenario, TestStep,
    TestSuite, VideoOperation, WaitCondition, ACTION_PROTOCOL_REVISION, PAGE_CONTEXT_PROTOCOL,
    REPAIR_PROTOCOL,
};
