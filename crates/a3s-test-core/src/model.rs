use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod assertions;

pub use assertions::{
    AssertionMode, AssertionStability, ElementState, Expectation, LayoutRect, LayoutRelation,
    ViewportCoverageComparison, WaitMode, DEFAULT_ASSERTION_SAMPLE_INTERVAL_MS,
    MAX_ASSERTION_STABILITY_MS, MAX_ASSERTION_STABILITY_SAMPLES, MAX_LAYOUT_COORDINATE_ABS,
    MAX_LAYOUT_TOLERANCE_PX, MAX_RENDERED_TEXT_ITEMS, MAX_VIEWPORT_COVERAGE_PERCENT,
    MIN_ASSERTION_STABILITY_MS,
};

pub const ACTION_PROTOCOL_REVISION: u32 = 15;
pub const PAGE_CONTEXT_PROTOCOL: &str = "a3s.test.page-context/1";
pub const SOURCE_MAPPING_PROTOCOL: &str = "a3s.test.source-mapping/1";
pub const REPAIR_PROTOCOL: &str = "a3s.test.repair/1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    Web,
    Gui,
    Tui,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TestSuite {
    pub name: String,
    pub version: u32,
    pub scenarios: Vec<TestScenario>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TestScenario {
    pub id: String,
    pub name: String,
    pub surface: Surface,
    pub timeout_ms: u64,
    pub steps: Vec<TestStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TestStep {
    pub id: String,
    pub action: Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability: Option<AssertionStability>,
    #[serde(default, skip_serializing_if = "AssertionMode::is_positive")]
    pub assertion_mode: AssertionMode,
    #[serde(default, skip_serializing_if = "WaitMode::is_positive")]
    pub wait_mode: WaitMode,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Navigate {
        url: String,
    },
    Snapshot {
        interactive: bool,
    },
    Click {
        target: Target,
    },
    Hover {
        target: Target,
    },
    Focus {
        target: Target,
    },
    DoubleClick {
        target: Target,
    },
    ContextClick {
        target: Target,
    },
    Fill {
        target: Target,
        value: String,
    },
    Type {
        target: Target,
        value: String,
    },
    InsertText {
        value: String,
    },
    Check {
        target: Target,
    },
    Uncheck {
        target: Target,
    },
    Select {
        target: Target,
        values: Vec<String>,
    },
    Drag {
        source: Target,
        target: Target,
    },
    Press {
        key: String,
    },
    TerminalPaste {
        text: String,
    },
    TerminalResize {
        columns: u16,
        rows: u16,
    },
    TerminalRecording {
        path: String,
    },
    Wheel {
        target: Option<Target>,
        delta_x: i32,
        delta_y: i32,
        modifiers: Vec<ModifierKey>,
    },
    Viewport {
        width: u32,
        height: u32,
        scale: Option<u32>,
    },
    Wait {
        condition: WaitCondition,
    },
    Assert {
        expectation: Expectation,
    },
    Screenshot {
        path: String,
    },
    Tab {
        operation: TabOperation,
    },
    Frame {
        target: FrameTarget,
    },
    Dialog {
        operation: DialogOperation,
    },
    Upload {
        target: Target,
        paths: Vec<String>,
    },
    Download {
        target: Target,
        path: String,
    },
    NetworkRoute {
        pattern: String,
        route: NetworkRoute,
    },
    NetworkUnroute {
        pattern: Option<String>,
    },
    Har {
        operation: CaptureOperation,
    },
    Trace {
        operation: CaptureOperation,
    },
    Video {
        operation: VideoOperation,
    },
    Accessibility {
        path: String,
        interactive: bool,
    },
    Console {
        path: String,
        clear: bool,
    },
    PageErrors {
        path: String,
        clear: bool,
    },
    #[schemars(skip)]
    VerifyContract {
        contract: String,
        variant: String,
        state: String,
    },
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ModifierKey {
    Alt,
    Control,
    Meta,
    Shift,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum TabOperation {
    List,
    New {
        url: Option<String>,
        label: Option<String>,
    },
    Switch {
        tab: String,
    },
    Close {
        tab: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FrameTarget {
    Main,
    Selector(String),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DialogOperation {
    Status,
    Accept { text: Option<String> },
    Dismiss,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum NetworkRoute {
    Abort,
    Body(String),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaptureOperation {
    Start,
    Stop { path: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum VideoOperation {
    Start { path: String, url: Option<String> },
    Stop,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Ref { value: String },
    Css { selector: String },
    Role { role: String, name: String },
    Text { value: String, exact: bool },
    AutomationId { value: String },
    VisualPoint { snapshot: String, x: u32, y: u32 },
    TestId { value: String },
    Label { value: String },
    Placeholder { value: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WaitCondition {
    Load(LoadState),
    Text(String),
    Regex(String),
    Url(String),
    Visible(Target),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadState {
    NetworkIdle,
    DomContentLoaded,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Evidence {
    pub name: String,
    pub path: String,
    pub media_type: String,
}

/// Screenshot evidence captured without changing the observed surface state.
///
/// `surface_revision` is supplied when the surface exposes a revision that can
/// be revalidated around capture. It is intentionally distinct from an agent
/// session's observation sequence number.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroundingScreenshot {
    pub evidence: Evidence,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub surface_revision: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct StepOutput {
    pub summary: String,
    pub data: Value,
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_context: Option<PageContextObservation>,
}

impl StepOutput {
    #[must_use]
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            data: Value::Null,
            evidence: Vec::new(),
            page_context: None,
        }
    }

    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }

    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    #[must_use]
    pub fn with_page_context(mut self, page_context: PageContextObservation) -> Self {
        self.page_context = Some(page_context);
        self
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct SurfaceObservation {
    pub summary: String,
    pub data: Value,
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_context: Option<PageContextObservation>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextObservation {
    pub present: bool,
    pub protocol: Option<String>,
    pub sdk_version: Option<String>,
    pub revision: Option<u64>,
    pub snapshot: Option<PageContextSnapshot>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextSnapshot {
    pub protocol: Option<String>,
    #[serde(rename = "sdkVersion")]
    pub sdk_version: Option<String>,
    pub revision: Option<u64>,
    pub page: Option<PageContextPage>,
    pub components: Vec<PageContextComponent>,
    pub nodes: Vec<PageContextNode>,
    pub facts: serde_json::Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<crate::UiUnderstandingSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<crate::PageContextDelta>,
    #[serde(rename = "removedNodeIds")]
    pub removed_node_ids: Vec<String>,
    pub truncated: bool,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextPage {
    pub id: String,
    pub url: String,
    pub route: String,
    pub title: String,
    pub ready: bool,
    pub viewport: PageContextViewport,
    pub document: PageContextSize,
    pub scroll: PageContextPoint,
    pub language: String,
    pub theme: PageContextTheme,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextViewport {
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual: Option<PageContextVisualViewport>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextVisualViewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PageContextTheme {
    Light,
    Dark,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextComponent {
    pub id: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub source: Option<PageContextSource>,
    pub ready: bool,
    pub facts: serde_json::Map<String, Value>,
    pub boxes: Vec<PageContextRect>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextSource {
    pub file: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    #[serde(rename = "endLine", default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(rename = "endColumn", default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextSourceMapping {
    pub protocol: String,
    pub candidates: Vec<PageContextSourceCandidate>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextSourceCandidate {
    pub span: PageContextSource,
    #[serde(
        rename = "generatedSpan",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub generated_span: Option<PageContextSource>,
    pub confidence: f64,
    pub origin: PageContextSourceOrigin,
    pub relation: PageContextSourceRelation,
    #[serde(rename = "registrationId")]
    pub registration_id: String,
    #[serde(
        rename = "componentId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub component_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageContextSourceOrigin {
    BoundaryHint,
    FrameworkAdapter,
    SourceMap,
    Generated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageContextSourceRelation {
    Exact,
    Ancestor,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextNode {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(rename = "componentId")]
    pub component_id: Option<String>,
    pub tag: String,
    pub role: Option<String>,
    pub name: Option<String>,
    pub text: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "testId")]
    pub test_id: Option<String>,
    pub geometry: Option<PageContextGeometry>,
    pub state: PageContextNodeState,
    pub locators: Vec<PageContextLocator>,
    pub classes: Option<Vec<String>>,
    pub attributes: Option<serde_json::Map<String, Value>>,
    #[serde(rename = "computedStyles")]
    pub computed_styles: Option<serde_json::Map<String, Value>>,
    #[serde(
        rename = "sourceMapping",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub source_mapping: Option<PageContextSourceMapping>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextGeometry {
    pub viewport: PageContextRect,
    pub document: PageContextRect,
    pub normalized: PageContextRect,
    #[serde(rename = "visibleRatio")]
    pub visible_ratio: f64,
    pub occluded: bool,
    pub position: PageContextPosition,
    pub transformed: bool,
    #[serde(rename = "scrollContainerNodeId")]
    pub scroll_container_node_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PageContextPosition {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageContextNodeState {
    pub visible: bool,
    pub disabled: Option<bool>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub focused: Option<bool>,
    pub readonly: Option<bool>,
    pub required: Option<bool>,
    pub invalid: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PageContextLocator {
    Role { role: String, name: String },
    Label { value: String },
    TestId { value: String },
    Placeholder { value: String },
    Text { value: String, exact: bool },
    Css { value: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairFinding {
    pub id: String,
    #[serde(rename = "batchId")]
    pub batch_id: String,
    pub instruction: String,
    #[serde(rename = "successCriteria")]
    pub success_criteria: Option<String>,
    pub intent: RepairIntent,
    pub severity: RepairSeverity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RepairRelation>,
    #[serde(
        rename = "designReference",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub design_reference: Option<RepairDesignReference>,
    pub target: RepairTarget,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "pageId")]
    pub page_id: String,
    pub url: String,
    #[serde(rename = "contextRevision")]
    pub context_revision: u64,
    pub context: Value,
    pub status: RepairStatus,
    #[serde(rename = "submittedAt")]
    pub submitted_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepairRelation {
    ConflictsWith {
        #[serde(rename = "findingId")]
        finding_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairDesignReference {
    pub kind: RepairDesignReferenceKind,
    pub width: u32,
    pub height: u32,
    pub image: RepairDesignReferenceImage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairDesignReferenceKind {
    Sketch,
    Screenshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepairDesignReferenceImage {
    Inline {
        #[serde(rename = "mediaType")]
        media_type: String,
        #[serde(rename = "dataUrl")]
        data_url: String,
    },
    Artifact {
        evidence: Evidence,
        sha256: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairIntent {
    Fix,
    Change,
    Question,
    Approve,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairSeverity {
    Blocking,
    Important,
    Suggestion,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairTarget {
    pub kind: RepairTargetKind,
    #[serde(rename = "nodeIds")]
    pub node_ids: Vec<String>,
    #[serde(rename = "selectedText")]
    pub selected_text: Option<String>,
    pub region: Option<PageContextRect>,
    pub drawing: Option<Vec<PageContextPoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<RepairLayoutIntent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepairLayoutIntent {
    Placement {
        #[serde(rename = "componentType")]
        component_type: String,
        canvas: RepairLayoutCanvas,
        purpose: Option<String>,
    },
    Rearrange {
        #[serde(rename = "originalRegion")]
        original_region: PageContextRect,
        purpose: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairLayoutCanvas {
    Page,
    Wireframe,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairTargetKind {
    Node,
    Text,
    Region,
    Drawing,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairStatus {
    Draft,
    Queued,
    Claimed,
    Repairing,
    Verifying,
    NeedsInput,
    VerificationFailed,
    ReviewReady,
    Resolved,
    Dismissed,
    Cancelled,
    Failed,
    Reopened,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairActor {
    Human,
    Agent,
    #[serde(rename = "a3s-test", alias = "a3s_test")]
    A3sTest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairStatusEvent {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "findingId")]
    pub finding_id: String,
    pub sequence: u64,
    pub status: RepairStatus,
    pub actor: RepairActor,
    pub timestamp: String,
    pub summary: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairHumanActionKind {
    Reply,
    Accept,
    Dismiss,
    Reopen,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairHumanAction {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "findingId")]
    pub finding_id: String,
    pub action: RepairHumanActionKind,
    pub timestamp: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairCheckResult {
    pub command: String,
    pub status: RepairCheckStatus,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairEvidencePhase {
    Before,
    After,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairEvidenceRequest {
    #[serde(rename = "findingId")]
    pub finding_id: String,
    #[serde(rename = "attemptId")]
    pub attempt_id: Option<String>,
    pub phase: RepairEvidencePhase,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairEvidenceBundle {
    #[serde(rename = "capturedAtMs")]
    pub captured_at_ms: u64,
    #[serde(rename = "contextRevision")]
    pub context_revision: u64,
    #[serde(rename = "contextSha256")]
    pub context_sha256: String,
    pub context: PageContextSnapshot,
    #[serde(rename = "consoleErrors")]
    pub console_errors: u32,
    #[serde(rename = "pageErrors")]
    pub page_errors: u32,
    pub screenshot: Evidence,
    #[serde(rename = "screenshotSha256")]
    pub screenshot_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairAclProof {
    pub path: String,
    pub passed: bool,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairCheckStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairVerification {
    #[serde(rename = "findingId")]
    pub finding_id: String,
    #[serde(rename = "attemptId")]
    pub attempt_id: String,
    #[serde(rename = "beforeRevision")]
    pub before_revision: u64,
    #[serde(rename = "afterRevision")]
    pub after_revision: u64,
    #[serde(rename = "targetFound")]
    pub target_found: bool,
    #[serde(rename = "successCriteriaPassed")]
    #[serde(default)]
    pub success_criteria_passed: Option<bool>,
    #[serde(rename = "newConsoleErrors")]
    pub new_console_errors: u32,
    #[serde(rename = "newPageErrors")]
    pub new_page_errors: u32,
    #[serde(rename = "changedFiles")]
    pub changed_files: Vec<String>,
    pub checks: Vec<RepairCheckResult>,
    #[serde(rename = "aclCandidate")]
    #[serde(default)]
    pub acl_candidate: Option<String>,
    #[serde(rename = "aclProof")]
    #[serde(default)]
    pub acl_proof: Option<RepairAclProof>,
    #[serde(rename = "beforeEvidence")]
    #[serde(default)]
    pub before_evidence: Option<RepairEvidenceBundle>,
    #[serde(rename = "afterEvidence")]
    #[serde(default)]
    pub after_evidence: Option<RepairEvidenceBundle>,
    pub passed: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairAttempt {
    pub id: String,
    #[serde(rename = "startedAtMs")]
    pub started_at_ms: u64,
    #[serde(rename = "finishedAtMs")]
    pub finished_at_ms: Option<u64>,
    pub status: RepairStatus,
    pub replies: Vec<RepairThreadMessage>,
    pub verification: Option<RepairVerification>,
    #[serde(rename = "beforeEvidence")]
    pub before_evidence: Option<RepairEvidenceBundle>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairThreadMessage {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub actor: RepairActor,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: u64,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepairBatch {
    pub id: String,
    #[serde(rename = "findingIds")]
    pub finding_ids: Vec<String>,
    pub status: RepairBatchStatus,
    pub results: Vec<RepairBatchItemResult>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairBatchStatus {
    Queued,
    InProgress,
    NeedsInput,
    ReviewReady,
    Resolved,
    CompletedWithFailures,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairBatchItemResult {
    #[serde(rename = "findingId")]
    pub finding_id: String,
    pub status: RepairStatus,
}

impl SurfaceObservation {
    #[must_use]
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            data: Value::Null,
            evidence: Vec::new(),
            page_context: None,
        }
    }

    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }

    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    #[must_use]
    pub fn with_page_context(mut self, page_context: PageContextObservation) -> Self {
        self.page_context = Some(page_context);
        self
    }
}
