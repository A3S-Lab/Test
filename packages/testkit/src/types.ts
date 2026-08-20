export const PAGE_CONTEXT_PROTOCOL = "a3s.test.page-context/1" as const;
export const TESTKIT_HANDSHAKE_PROTOCOL =
  "a3s.test.testkit-handshake/1" as const;
export const TESTKIT_PACKAGE_NAME = "@a3s-lab/testkit" as const;
export const UI_UNDERSTANDING_PROTOCOL = "a3s.test.ui-understanding/1" as const;
export const PAGE_CONTEXT_SYMBOL = Symbol.for("a3s.test.page-context");
export const QUALITY_REPORT_PROTOCOL = "a3s.test.quality-report/1" as const;
export const DESIGN_AUDIT_REPORT_PROTOCOL =
  "a3s.test.design-audit-report/1" as const;

export type ContextDetail = "summary" | "scoped" | "diff" | "forensic";

export type ContextScope =
  | { kind: "page" }
  | { kind: "node"; nodeId: string }
  | { kind: "component"; componentId: string }
  | {
      kind: "region";
      space: "viewport" | "document";
      x: number;
      y: number;
      width: number;
      height: number;
    };

export type ContextLimits = {
  nodes: number;
  stringBytes: number;
  encodedBytes: number;
  uiNodes: number;
  uiStateSamples: number;
  uiDurationMs: number;
  uiEncodedBytes: number;
};

export type ContextSnapshotRequest = {
  detail?: ContextDetail;
  scope?: ContextScope;
  sinceRevision?: number | null;
  cursor?: string | null;
  ui?: boolean;
  limits?: Partial<ContextLimits>;
};

export type Rect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type VisualViewportInfo = Rect & {
  scale: number;
};

export type PageViewport = {
  width: number;
  height: number;
  dpr: number;
  visual?: VisualViewportInfo;
};

export type NodeGeometry = {
  viewport: Rect;
  document: Rect;
  normalized: Rect;
  visibleRatio: number;
  occluded: boolean;
  position: "static" | "relative" | "absolute" | "fixed" | "sticky";
  transformed: boolean;
  scrollContainerNodeId?: string;
};

export type LocatorCandidate =
  | { type: "role"; role: string; name: string }
  | { type: "label"; value: string }
  | { type: "test_id"; value: string }
  | { type: "placeholder"; value: string }
  | { type: "text"; value: string; exact: boolean }
  | { type: "css"; value: string };

export type ContextNode = {
  id: string;
  parentId?: string;
  componentId?: string;
  tag: string;
  role?: string;
  name?: string;
  text?: string;
  description?: string;
  testId?: string;
  geometry?: NodeGeometry;
  state: {
    visible: boolean;
    disabled?: boolean;
    checked?: boolean;
    selected?: boolean;
    expanded?: boolean;
    focused?: boolean;
    readonly?: boolean;
    required?: boolean;
    invalid?: boolean;
  };
  locators: LocatorCandidate[];
  classes?: string[];
  attributes?: Record<string, string>;
  computedStyles?: Record<string, string>;
};

export type ContextComponent = {
  id: string;
  name: string;
  parentId?: string;
  source?: { file: string; line?: number; column?: number };
  ready: boolean;
  facts: Record<string, JsonValue>;
  boxes: Rect[];
};

export type UIEvidenceSourceKind =
  | "computed_style"
  | "dom_structure"
  | "layout_geometry"
  | "accessibility_state"
  | "css_stylesheet"
  | "web_animations";

export type UITruncationReason =
  "node_limit" | "state_sample_limit" | "time_limit" | "encoded_size_limit";

export type UIUnderstandingBudget = {
  limits: {
    nodes: number;
    stateSamples: number;
    stringBytes: number;
    encodedBytes: number;
    durationMs: number;
  };
  used: {
    nodes: number;
    stateSamples: number;
    encodedBytes: number;
    durationMs: number;
  };
  truncated: boolean;
  reasons: UITruncationReason[];
};

export type UIObservedToken = {
  value: string;
  properties: string[];
  count: number;
  nodeIds: string[];
  confidence: 1;
};

export type UITypographyToken = {
  family: string;
  size: string;
  weight: string;
  lineHeight: string;
  letterSpacing: string;
  count: number;
  nodeIds: string[];
  confidence: 1;
};

export type UICustomProperty = {
  name: string;
  value: string;
  source: "document_root";
  confidence: 1;
};

export type UIResponsiveCondition = {
  condition: string;
  matches: boolean;
  source: "stylesheet";
  confidence: 1;
};

export type UIStyleProfile = {
  colors: UIObservedToken[];
  typography: UITypographyToken[];
  spacing: UIObservedToken[];
  radii: UIObservedToken[];
  shadows: UIObservedToken[];
  zIndices: UIObservedToken[];
  customProperties: UICustomProperty[];
  responsiveConditions: UIResponsiveCondition[];
};

export type UIFlexLayout = {
  direction: string;
  wrap: string;
  justifyContent: string;
  alignItems: string;
  alignContent: string;
  gap: string;
};

export type UIGridLayout = {
  templateColumns: string;
  templateRows: string;
  autoFlow: string;
  justifyItems: string;
  alignItems: string;
  gap: string;
};

export type UIBoxEdges = {
  top: string;
  right: string;
  bottom: string;
  left: string;
};

export type UIBoxModel = {
  boxSizing: "content-box" | "border-box" | "unknown";
  writingMode:
    | "horizontal-tb"
    | "vertical-rl"
    | "vertical-lr"
    | "sideways-rl"
    | "sideways-lr"
    | "unknown";
  direction: "ltr" | "rtl" | "unknown";
  margin: UIBoxEdges;
  borderWidth: UIBoxEdges;
  padding: UIBoxEdges;
};

export type UIOverflowMetrics = {
  clientWidth: number;
  clientHeight: number;
  scrollWidth: number;
  scrollHeight: number;
  scrollLeft: number;
  scrollTop: number;
  overflowingX: boolean;
  overflowingY: boolean;
  clipsX: boolean;
  clipsY: boolean;
};

export type UILayoutNode = {
  nodeId: string;
  parentNodeId?: string;
  display: string;
  position: string;
  rect?: Rect;
  overflowX: string;
  overflowY: string;
  overflowMetrics: UIOverflowMetrics;
  boxModel: UIBoxModel;
  order: string;
  stackingContextReasons: string[];
  flex?: UIFlexLayout;
  grid?: UIGridLayout;
};

export type UILayoutEdge = {
  fromNodeId: string;
  toNodeId: string;
  relation: "contains" | "scroll_container" | "offset_parent";
};

export type UILayoutGraph = {
  nodes: UILayoutNode[];
  edges: UILayoutEdge[];
};

export type UIComponentCluster = {
  id: string;
  fingerprint: string;
  signature: string;
  representativeNodeId: string;
  memberNodeIds: string[];
  memberCount: number;
  confidence: 1;
};

export type UIInteractionState =
  | "default"
  | "hover"
  | "focus"
  | "focus_visible"
  | "checked"
  | "expanded"
  | "selected"
  | "disabled";

export type UIStyleChange = {
  property: string;
  before: string;
  after: string;
};

export type UIAccessibilityStateChange = {
  state: string;
  before: boolean | null;
  after: boolean | null;
};

export type UIStateDiff = {
  nodeId: string;
  from: "default";
  to: Exclude<UIInteractionState, "default">;
  styleChanges: UIStyleChange[];
  accessibilityChanges: UIAccessibilityStateChange[];
  confidence: 1;
};

export type UITransitionProfile = {
  nodeId: string;
  properties: string[];
  durations: string[];
  delays: string[];
  timingFunctions: string[];
};

export type UIAnimationProfile = {
  nodeId: string;
  names: string[];
  durations: string[];
  delays: string[];
  iterationCounts: string[];
  playStates: string[];
  sources: Array<"css" | "web_animations">;
  timelines: UIAnimationTimeline[];
  rangeStarts: string[];
  rangeEnds: string[];
};

export type UIAnimationTimelineKind =
  "document" | "scroll" | "view" | "named" | "none" | "unknown";

export type UIAnimationTimeline = {
  value: string;
  kind: UIAnimationTimelineKind;
  source: "computed_style" | "web_animations";
};

export type UIMotionProfile = {
  prefersReducedMotion: boolean;
  transitions: UITransitionProfile[];
  animations: UIAnimationProfile[];
  keyframeNames: string[];
  stickyNodeIds: string[];
  scrollContainerNodeIds: string[];
  canvasNodeIds: string[];
  mediaNodeIds: string[];
};

export type UIUnderstandingEvidence = {
  sourceKinds: UIEvidenceSourceKind[];
  sampledNodeIds: string[];
  totalCandidateNodes: number;
  omittedNodes: number;
  inaccessibleStyleSheets: number;
};

export type UIUnderstandingSnapshot = {
  protocol: typeof UI_UNDERSTANDING_PROTOCOL;
  observationId: string;
  pageRevision: number;
  viewport: PageViewport;
  scope: ContextScope;
  budget: UIUnderstandingBudget;
  evidence: UIUnderstandingEvidence;
  style: UIStyleProfile;
  layout: UILayoutGraph;
  components: UIComponentCluster[];
  stateDiffs: UIStateDiff[];
  motion: UIMotionProfile;
};

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue =
  JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export type PageContextSnapshot = {
  protocol: typeof PAGE_CONTEXT_PROTOCOL;
  sdkVersion: string;
  revision: number;
  page: {
    id: string;
    url: string;
    route: string;
    title: string;
    ready: boolean;
    viewport: PageViewport;
    document: { width: number; height: number };
    scroll: { x: number; y: number };
    language: string;
    theme: "light" | "dark" | "unknown";
  };
  components: ContextComponent[];
  nodes: ContextNode[];
  facts: Record<string, JsonValue>;
  ui?: UIUnderstandingSnapshot;
  removedNodeIds: string[];
  truncated: boolean;
  nextCursor: string | null;
};

export type RepairIntent = "fix" | "change" | "question" | "approve";
export type RepairSeverity = "blocking" | "important" | "suggestion";
export type RepairStatus =
  | "draft"
  | "queued"
  | "claimed"
  | "repairing"
  | "verifying"
  | "needs_input"
  | "verification_failed"
  | "review_ready"
  | "resolved"
  | "dismissed"
  | "cancelled"
  | "failed"
  | "reopened";

export type RepairTarget = {
  kind: "node" | "text" | "region" | "drawing";
  nodeIds: string[];
  selectedText?: string;
  region?: Rect;
  regionScroll?: { x: number; y: number };
  drawing?: Array<{ x: number; y: number }>;
  layout?: RepairLayoutIntent;
};

export type RepairLayoutIntent =
  | {
      kind: "placement";
      componentType: string;
      canvas: "page" | "wireframe";
      purpose?: string;
    }
  | {
      kind: "rearrange";
      originalRegion: Rect;
      purpose?: string;
    };

export type RepairRelation = {
  kind: "conflicts_with";
  findingId: string;
};

export type RepairDesignReference = {
  kind: "sketch" | "screenshot";
  width: number;
  height: number;
  image:
    | {
        kind: "inline";
        mediaType: "image/png" | "image/jpeg";
        dataUrl: string;
      }
    | {
        kind: "artifact";
        evidence: {
          name: string;
          path: string;
          media_type: "image/png" | "image/jpeg";
        };
        sha256: string;
      };
};

export type StructuredRepairExport = {
  protocol: "a3s.test.repair/1";
  page: {
    id: string;
    url: string;
    route: string;
    revision: number;
    viewport: PageViewport;
  };
  findings: Array<{
    id: string;
    instruction: string;
    successCriteria?: string;
    intent: RepairIntent;
    severity: RepairSeverity;
    relations?: RepairRelation[];
    designReference?: RepairDesignReference;
    target: RepairTarget;
    context: RepairContext;
  }>;
};

export type RepairContext = {
  route: string;
  title: string;
  viewport: PageViewport;
  component?: ContextComponent;
  nodes: ContextNode[];
  nearbyNodes: ContextNode[];
  facts: Record<string, JsonValue>;
  ui?: UIUnderstandingSnapshot;
  untrusted: true;
};

export type RepairDraft = {
  id: string;
  instruction: string;
  successCriteria?: string;
  intent: RepairIntent;
  severity: RepairSeverity;
  relations?: RepairRelation[];
  designReference?: RepairDesignReference;
  target: RepairTarget;
  createdAt: string;
};

export type SubmittedRepair = RepairDraft & {
  batchId: string;
  pageId: string;
  url: string;
  contextRevision: number;
  context: RepairContext;
  status: RepairStatus;
  submittedAt: string;
};

export type RepairAttempt = {
  id: string;
  startedAtMs: number;
  finishedAtMs?: number;
  status: RepairStatus;
  replies: RepairThreadMessage[];
};

export type RepairBatchStatus =
  | "queued"
  | "in_progress"
  | "needs_input"
  | "review_ready"
  | "resolved"
  | "completed_with_failures";

export type RepairBatch = {
  id: string;
  findingIds: string[];
  status: RepairBatchStatus;
  results: Array<{ findingId: string; status: RepairStatus }>;
};

export type RepairEvent = {
  requestId: string;
  findingId: string;
  sequence: number;
  status: RepairStatus;
  actor: "human" | "agent" | "a3s-test";
  timestamp: string;
  summary?: string;
  message?: string;
};

export type RepairThreadMessage = {
  requestId: string;
  findingId: string;
  actor: "human" | "agent" | "a3s-test";
  timestamp: string;
  message: string;
};

export type RepairHumanActionKind = "reply" | "accept" | "dismiss" | "reopen";

export type RepairHumanAction = {
  requestId: string;
  findingId: string;
  action: RepairHumanActionKind;
  timestamp: string;
  message?: string;
};

export type RepairHumanActionInput = {
  findingId: string;
  action: RepairHumanActionKind;
  message?: string;
};

export type RepairSubmission = {
  batchId?: string;
  findings: RepairDraft[];
};

export type QualityOutcome = "passed" | "failed" | "inconclusive";
export type QualitySeverity = "blocking" | "important" | "suggestion";

export type QualityFinding = {
  id: string;
  dimension: string;
  rule_id: string;
  severity: QualitySeverity;
  message: string;
  expected: JsonValue;
  actual: JsonValue;
  element_id?: string;
  observed_node_id?: string;
  confidence: number;
};

export type QualityReport = {
  contract: string;
  variant: string;
  state: string;
  outcome: QualityOutcome;
  observation_revision?: number | null;
  matches: Array<{
    element_id: string;
    node_id: string;
    strategy: "test_id" | "component" | "role_and_name" | "role";
  }>;
  findings: QualityFinding[];
};

export type QualityReportRecord = QualityReport & {
  id: string;
  protocol: typeof QUALITY_REPORT_PROTOCOL;
  reportedAt: string;
};

export type DesignAuditDimension =
  | "visual_hierarchy"
  | "layout_composition"
  | "spacing_rhythm"
  | "typography"
  | "color_use"
  | "consistency"
  | "interaction_clarity"
  | "content_clarity"
  | "responsive_composition";

export type DesignAuditPriority = "high" | "medium" | "low";

export type DesignAuditTarget =
  | { kind: "page" }
  | { kind: "node"; node_id: string }
  | { kind: "region"; region: Rect };

export type DesignAuditFinding = {
  id: string;
  dimension: DesignAuditDimension;
  priority: DesignAuditPriority;
  summary: string;
  rationale: string;
  recommendation: string;
  confidence: number;
  target: DesignAuditTarget;
};

export type DesignAuditReport = {
  protocol: typeof DESIGN_AUDIT_REPORT_PROTOCOL;
  provenance: {
    identity: { provider: string; model: string };
    observation_id: number;
    surface_revision: number;
    screenshot_sha256: string;
    page_context_sha256: string;
    width: number;
    height: number;
    usage: { input_units: number; output_units: number; cost_microusd: number };
    request_id?: string | null;
    authority: "advisory";
  };
  dimensions: DesignAuditDimension[];
  findings: DesignAuditFinding[];
};

export type DesignAuditReportRecord = DesignAuditReport & {
  id: string;
  reportedAt: string;
};

export type TestKitEvent =
  | { type: "context.revision"; revision: number }
  | { type: "quality.reported"; report: QualityReportRecord }
  | { type: "quality.dismissed"; reportId: string; findingId?: string }
  | { type: "design_audit.reported"; report: DesignAuditReportRecord }
  | { type: "design_audit.dismissed"; reportId: string; findingId?: string }
  | { type: "repair.submitted"; repairs: SubmittedRepair[] }
  | { type: "repair.action_submitted"; action: RepairHumanAction }
  | { type: "repair.updated"; repair: SubmittedRepair; event: RepairEvent };

export type PageContextProbe = {
  protocol: typeof PAGE_CONTEXT_PROTOCOL;
  sdkVersion: string;
  capabilities: string[];
};

export type TestKitHandshake = {
  protocol: typeof TESTKIT_HANDSHAKE_PROTOCOL;
  packageName: typeof TESTKIT_PACKAGE_NAME;
  sdkVersion: string;
  pageContextProtocol: typeof PAGE_CONTEXT_PROTOCOL;
  capabilities: string[];
};

export type PageContextBridge = {
  handshake(): TestKitHandshake;
  probe(): PageContextProbe;
  snapshot(request?: ContextSnapshotRequest): PageContextSnapshot;
  resolve(nodeId: string): Element | null;
  waitForChange(revision: number, timeoutMs: number): Promise<number | null>;
  subscribe(listener: (event: TestKitEvent) => void): () => void;
  submitRepair(submission: RepairSubmission): SubmittedRepair[];
  takeRepairBatch(limit?: number): SubmittedRepair[];
  peekRepairBatch(limit?: number): SubmittedRepair[];
  listRepairs(): SubmittedRepair[];
  listRepairBatches(): RepairBatch[];
  exportRepairs(findings: RepairDraft[]): StructuredRepairExport;
  exportRepairsMarkdown(findings: RepairDraft[]): string;
  applyRepairEvent(event: RepairEvent): SubmittedRepair | null;
  submitRepairAction(action: RepairHumanActionInput): RepairHumanAction | null;
  takeRepairActions(limit?: number): RepairHumanAction[];
  addRepairReply(reply: RepairThreadMessage): boolean;
  listRepairReplies(findingId: string): RepairThreadMessage[];
  reportQuality(report: QualityReport): boolean;
  listQualityReports(): QualityReportRecord[];
  dismissQualityFinding(reportId: string, findingId: string): boolean;
  dismissQualityReport(reportId: string): boolean;
  reportDesignAudit(report: DesignAuditReport): boolean;
  listDesignAuditReports(): DesignAuditReportRecord[];
  dismissDesignAuditFinding(reportId: string, findingId: string): boolean;
  dismissDesignAuditReport(reportId: string): boolean;
  setAnimationsPaused(paused: boolean): void;
  animationsPaused(): boolean;
  dispose(): void;
};

export type BoundaryRegistration = {
  id: string;
  name: string;
  elements: () => readonly Element[];
  source?: { file: string; line?: number; column?: number };
  ready?: () => boolean;
  facts?: () => Record<string, unknown>;
};

export type TestKitOptions = {
  page: { id: string };
  enabled: boolean;
  ready?: () => boolean;
  facts?: () => Record<string, unknown>;
  redact?: string[];
  maxNodes?: number;
  maxStringBytes?: number;
  maxEncodedBytes?: number;
  uiUnderstanding?: boolean;
  maxUiNodes?: number;
  maxUiStateSamples?: number;
  maxUiDurationMs?: number;
  maxUiEncodedBytes?: number;
  repairEndpoint?: string;
  repairStorage?: "local" | "session" | "memory";
  maxQualityReports?: number;
  maxDesignAuditReports?: number;
};

export type TestKitRuntime = PageContextBridge & {
  registerBoundary(registration: BoundaryRegistration): () => void;
};
