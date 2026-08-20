import {
  describeElement,
  visualViewportInfo,
  type NodeIdentity,
} from "./dom";
import packageManifest from "../package.json";
import { boundaryDepth, boundaryElements, composedContains } from "./boundary";
import { DesignAuditStore, validDesignAuditReport } from "./design-audit-store";
import { QualityStore } from "./quality-store";
import { RepairStore } from "./repair-store";
import { structuredRepairMarkdown } from "./repair-markdown";
import {
  clamp,
  DEFAULT_CONTEXT_LIMITS,
  isJsonObject,
  MAX_CONTEXT_LIMITS,
  pageTheme,
} from "./runtime-values";
import { installPageObserver } from "./page-observer";
import { ContextHistory } from "./context-history";
import { elementsForContextScope } from "./context-scope";
import {
  buildContextResponse,
  decodeContextCursor,
  isValidContextId,
  normalizeContextLimits,
  normalizeContextScope,
  validateContextRevision,
  validateContextSnapshotRequest,
  validateContextWaitTimeout,
  type ContextCursorRequest,
} from "./context-response";
import { safeCallback, sanitizeFacts, truncateUtf8 } from "./sanitize";
import { normalizeSourceSpan, SourceMappingStore } from "./source-mapping";
import { captureUIUnderstanding } from "./ui-understanding";
import { UIStateTracker } from "./ui-understanding-state";
import {
  PAGE_CONTEXT_PROTOCOL,
  PAGE_CONTEXT_SYMBOL,
  TESTKIT_HANDSHAKE_PROTOCOL,
  TESTKIT_PACKAGE_NAME,
  type BoundaryRegistration,
  type ContextComponent,
  type ContextDiffRequest,
  type ContextNode,
  type ContextSnapshotRequest,
  type DesignAuditReport,
  type PageContextBridge,
  type PageContextSnapshot,
  type PageViewport,
  type QualityReport,
  type RepairEvent,
  type RepairHumanAction,
  type RepairHumanActionInput,
  type RepairContext,
  type RepairDraft,
  type RepairSubmission,
  type RepairThreadMessage,
  type SourceMapRegistration,
  type SourceRegistration,
  type StructuredRepairExport,
  type SubmittedRepair,
  type TestKitEvent,
  type TestKitHandshake,
  type TestKitOptions,
  type TestKitRuntime,
} from "./types";

const SDK_VERSION = packageManifest.version;
const TESTKIT_CAPABILITIES = Object.freeze([
  "bounded_snapshot",
  "component_boundaries",
  "design_audit_reports",
  "design_references",
  "diff",
  "geometry",
  "layout_intents",
  "open_shadow_dom",
  "quality_reports",
  "repair_queue",
  "revision_diff",
  "revision_wait",
  "scoped_inspection",
  "source_mapping",
  "ui_component_clusters",
  "ui_layout_graph",
  "ui_motion_profile",
  "ui_state_diffs",
  "ui_style_profile",
]);
type NormalizedOptions = Required<
  Pick<
    TestKitOptions,
    | "enabled"
    | "redact"
    | "maxNodes"
    | "maxStringBytes"
    | "maxEncodedBytes"
    | "uiUnderstanding"
    | "maxUiNodes"
    | "maxUiStateSamples"
    | "maxUiDurationMs"
    | "maxUiEncodedBytes"
    | "repairStorage"
    | "maxQualityReports"
    | "maxDesignAuditReports"
  >
> & {
  page: TestKitOptions["page"];
  repairEndpoint: string | undefined;
  ready: (() => boolean) | undefined;
  facts: (() => Record<string, unknown>) | undefined;
};

class Runtime implements TestKitRuntime, NodeIdentity {
  readonly #options: NormalizedOptions;
  readonly #nodeIds = new WeakMap<Element, string>();
  readonly #nodes = new Map<string, WeakRef<Element>>();
  readonly #boundaries = new Map<string, BoundaryRegistration>();
  readonly #listeners = new Set<(event: TestKitEvent) => void>();
  readonly #waiters = new Set<{
    revision: number;
    finish(value: number | null): void;
    timer: ReturnType<typeof setTimeout>;
  }>();
  readonly #history = new ContextHistory();
  readonly #cleanup: Array<() => void> = [];
  readonly #repairStore: RepairStore;
  readonly #qualityStore: QualityStore;
  readonly #designAuditStore: DesignAuditStore;
  readonly #sourceMappingStore = new SourceMappingStore();
  readonly #uiStateTracker = new UIStateTracker();
  #nodeSequence = 0;
  #revision = 1;
  #disposed = false;
  #pendingRevision = false;
  #animationsPaused = false;
  #pausedAnimations = new Set<Animation>();
  #pausedMedia = new Set<HTMLMediaElement>();
  #motionPauseFrame: number | null = null;

  constructor(options: NormalizedOptions) {
    this.#options = options;
    this.#repairStore = new RepairStore({
      pageId: options.page.id,
      storage: options.repairStorage,
      pageRevision: () => this.#revision,
      pageUrl: () => location.href,
      contextFor: (draft) => this.#repairContext(draft),
      emit: (event) => this.#emit(event),
      ...(options.repairEndpoint
        ? { repairEndpoint: options.repairEndpoint }
        : {}),
    });
    this.#qualityStore = new QualityStore(options.maxQualityReports);
    this.#designAuditStore = new DesignAuditStore(
      options.maxDesignAuditReports,
    );
    this.#cleanup.push(installPageObserver(() => this.#markChanged()));
  }

  handshake(): TestKitHandshake {
    return {
      protocol: TESTKIT_HANDSHAKE_PROTOCOL,
      packageName: TESTKIT_PACKAGE_NAME,
      sdkVersion: SDK_VERSION,
      pageContextProtocol: PAGE_CONTEXT_PROTOCOL,
      capabilities: [...TESTKIT_CAPABILITIES],
    };
  }

  probe() {
    return {
      protocol: PAGE_CONTEXT_PROTOCOL,
      sdkVersion: SDK_VERSION,
      capabilities: [...TESTKIT_CAPABILITIES],
    };
  }

  idFor(element: Element): string {
    const current = this.#nodeIds.get(element);
    if (current) return current;
    const id = `n${++this.#nodeSequence}`;
    this.#nodeIds.set(element, id);
    this.#nodes.set(id, new WeakRef(element));
    return id;
  }

  resolve(nodeId: string): Element | null {
    const element = this.#nodes.get(nodeId)?.deref() ?? null;
    return element?.isConnected ? element : null;
  }

  snapshot(request: ContextSnapshotRequest = {}): PageContextSnapshot {
    this.#ensureActive();
    const detail = request.detail ?? "summary";
    const scope = normalizeContextScope(request.scope ?? { kind: "page" });
    validateContextSnapshotRequest(detail, request.sinceRevision, this.#revision);
    const limits = normalizeContextLimits(request.limits, {
      nodes: Math.min(this.#options.maxNodes, MAX_CONTEXT_LIMITS.nodes),
      stringBytes: Math.min(
        this.#options.maxStringBytes,
        MAX_CONTEXT_LIMITS.stringBytes,
      ),
      encodedBytes: Math.min(
        this.#options.maxEncodedBytes,
        MAX_CONTEXT_LIMITS.encodedBytes,
      ),
      uiNodes: Math.min(
        this.#options.maxUiNodes,
        MAX_CONTEXT_LIMITS.uiNodes,
      ),
      uiStateSamples: Math.min(
        this.#options.maxUiStateSamples,
        MAX_CONTEXT_LIMITS.uiStateSamples,
      ),
      uiDurationMs: Math.min(
        this.#options.maxUiDurationMs,
        MAX_CONTEXT_LIMITS.uiDurationMs,
      ),
      uiEncodedBytes: Math.min(
        this.#options.maxUiEncodedBytes,
        this.#options.maxEncodedBytes,
        MAX_CONTEXT_LIMITS.uiEncodedBytes,
      ),
    });
    const uiEnabled = request.ui !== false && this.#options.uiUnderstanding;
    const cursorRequest: ContextCursorRequest = {
      detail,
      scope,
      sinceRevision: request.sinceRevision ?? null,
      ui: uiEnabled,
      limits,
    };
    const offset = decodeContextCursor(
      request.cursor,
      this.#revision,
      cursorRequest,
    );
    const elements = elementsForContextScope(
      scope,
      (nodeId) => this.resolve(nodeId),
      this.#boundaries,
    );
    const allNodes = elements
      .map((element) =>
        describeElement(
          element,
          this,
          detail,
          limits.stringBytes,
          this.#options.redact,
        ),
      )
      .filter((node): node is ContextNode => node !== null)
      .slice(0, MAX_CONTEXT_LIMITS.nodes);
    this.#associateComponents(allNodes);
    this.#associateSourceMappings(allNodes);
    const ui = uiEnabled
      ? captureUIUnderstanding({
            elements,
            nodes: allNodes,
            identity: this,
            pageRevision: this.#revision,
            viewport: this.#viewport(),
            scope,
            limits: {
              nodes: limits.uiNodes,
              stateSamples: limits.uiStateSamples,
              stringBytes: limits.stringBytes,
              encodedBytes: limits.uiEncodedBytes,
              durationMs: limits.uiDurationMs,
            },
            stateTracker: this.#uiStateTracker,
        })
      : undefined;

    const components = this.#components(limits.stringBytes);
    const page = this.#page();
    const facts = this.#facts(limits.stringBytes);
    const projected = this.#history.project(
      {
        revision: this.#revision,
        detail,
        scope,
        maxStringBytes: limits.stringBytes,
        page,
        components,
        nodes: allNodes,
        facts,
      },
      request.sinceRevision,
    );
    return buildContextResponse({
      revision: this.#revision,
      sourceNodes: projected.nodes,
      components: projected.components,
      removedNodeIds: projected.removedNodeIds,
      page,
      facts,
      ui,
      delta: projected.delta,
      offset,
      cursorRequest,
      limits,
    });
  }

  waitForChange(revision: number, timeoutMs: number): Promise<number | null> {
    this.#ensureActive();
    validateContextRevision(revision, this.#revision);
    const bounded = validateContextWaitTimeout(timeoutMs);
    if (this.#revision > revision) return Promise.resolve(this.#revision);
    if (bounded === 0) return Promise.resolve(null);
    return new Promise((resolve) => {
      const waiter = {
        revision,
        finish: resolve,
        timer: setTimeout(() => {
          this.#waiters.delete(waiter);
          resolve(null);
        }, bounded),
      };
      this.#waiters.add(waiter);
    });
  }

  async waitForDiff(
    request: ContextDiffRequest,
  ): Promise<PageContextSnapshot | null> {
    this.#ensureActive();
    validateContextRevision(request.sinceRevision, this.#revision);
    const { timeoutMs, sinceRevision, ...snapshotRequest } = request;
    const changed = await this.waitForChange(sinceRevision, timeoutMs);
    if (changed === null) return null;
    return this.snapshot({
      ...snapshotRequest,
      detail: "diff",
      sinceRevision,
    });
  }

  subscribe(listener: (event: TestKitEvent) => void): () => void {
    this.#ensureActive();
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  submitRepair(submission: RepairSubmission): SubmittedRepair[] {
    this.#ensureActive();
    return this.#repairStore.submit(submission);
  }

  takeRepairBatch(limit?: number): SubmittedRepair[] {
    this.#ensureActive();
    return this.#repairStore.queued(limit);
  }

  peekRepairBatch(limit?: number): SubmittedRepair[] {
    this.#ensureActive();
    return this.#repairStore.peek(limit);
  }

  listRepairs(): SubmittedRepair[] {
    return this.#repairStore.list();
  }

  listRepairBatches() {
    return this.#repairStore.batches();
  }

  exportRepairs(findings: RepairDraft[]): StructuredRepairExport {
    this.#ensureActive();
    const snapshot = this.snapshot({ detail: "summary", limits: { nodes: 1 } });
    return {
      protocol: "a3s.test.repair/1",
      page: {
        id: snapshot.page.id,
        url: snapshot.page.url,
        route: snapshot.page.route,
        revision: snapshot.revision,
        viewport: snapshot.page.viewport,
      },
      findings: findings.slice(0, 100).map((finding) => {
        const captured = this.#repairContext(finding);
        return {
          id: finding.id,
          instruction: finding.instruction,
          ...(finding.successCriteria
            ? { successCriteria: finding.successCriteria }
            : {}),
          intent: finding.intent,
          severity: finding.severity,
          ...(finding.relations?.length
            ? { relations: structuredClone(finding.relations) }
            : {}),
          ...(finding.designReference
            ? { designReference: structuredClone(finding.designReference) }
            : {}),
          target: structuredClone(finding.target),
          context: captured.context,
        };
      }),
    };
  }

  exportRepairsMarkdown(findings: RepairDraft[]): string {
    return structuredRepairMarkdown(this.exportRepairs(findings));
  }

  applyRepairEvent(event: RepairEvent): SubmittedRepair | null {
    this.#ensureActive();
    return this.#repairStore.apply(event);
  }

  submitRepairAction(action: RepairHumanActionInput): RepairHumanAction | null {
    this.#ensureActive();
    return this.#repairStore.submitAction(action);
  }

  takeRepairActions(limit?: number): RepairHumanAction[] {
    this.#ensureActive();
    return this.#repairStore.takeActions(limit);
  }

  addRepairReply(reply: RepairThreadMessage): boolean {
    this.#ensureActive();
    return this.#repairStore.addReply(reply);
  }

  listRepairReplies(findingId: string): RepairThreadMessage[] {
    this.#ensureActive();
    return this.#repairStore.replies(findingId);
  }

  reportQuality(report: QualityReport): boolean {
    this.#ensureActive();
    const record = this.#qualityStore.report(report);
    if (!record) return false;
    this.#emit({ type: "quality.reported", report: record });
    return true;
  }

  listQualityReports() {
    return this.#qualityStore.list();
  }

  dismissQualityFinding(reportId: string, findingId: string): boolean {
    this.#ensureActive();
    const dismissed = this.#qualityStore.dismissFinding(reportId, findingId);
    if (dismissed)
      this.#emit({ type: "quality.dismissed", reportId, findingId });
    return dismissed;
  }

  dismissQualityReport(reportId: string): boolean {
    this.#ensureActive();
    const dismissed = this.#qualityStore.dismissReport(reportId);
    if (dismissed) this.#emit({ type: "quality.dismissed", reportId });
    return dismissed;
  }

  reportDesignAudit(report: DesignAuditReport): boolean {
    this.#ensureActive();
    if (!validDesignAuditReport(report)) return false;
    if (report?.provenance?.surface_revision !== this.#revision) return false;
    if (
      report.findings.some(
        (finding) =>
          finding.target.kind === "node" &&
          this.resolve(finding.target.node_id) === null,
      )
    )
      return false;
    const record = this.#designAuditStore.report(report);
    if (!record) return false;
    this.#emit({ type: "design_audit.reported", report: record });
    return true;
  }

  listDesignAuditReports() {
    return this.#designAuditStore.list();
  }

  dismissDesignAuditFinding(reportId: string, findingId: string): boolean {
    this.#ensureActive();
    const dismissed = this.#designAuditStore.dismissFinding(
      reportId,
      findingId,
    );
    if (dismissed)
      this.#emit({ type: "design_audit.dismissed", reportId, findingId });
    return dismissed;
  }

  dismissDesignAuditReport(reportId: string): boolean {
    this.#ensureActive();
    const dismissed = this.#designAuditStore.dismissReport(reportId);
    if (dismissed) this.#emit({ type: "design_audit.dismissed", reportId });
    return dismissed;
  }

  setAnimationsPaused(paused: boolean): void {
    this.#ensureActive();
    if (this.#animationsPaused === paused) return;
    this.#animationsPaused = paused;
    document.documentElement.toggleAttribute(
      "data-a3s-testkit-animations-paused",
      paused,
    );
    if (paused) {
      this.#pauseActiveMotion();
    } else {
      if (this.#motionPauseFrame !== null) {
        window.cancelAnimationFrame(this.#motionPauseFrame);
        this.#motionPauseFrame = null;
      }
      for (const animation of this.#pausedAnimations) {
        if (animation.playState !== "paused") continue;
        try {
          animation.play();
        } catch {
          // The host may cancel or detach an animation while review motion is paused.
        }
      }
      this.#pausedAnimations.clear();
      for (const media of this.#pausedMedia) {
        if (!media.isConnected || media.ended) continue;
        void media.play().catch(() => undefined);
      }
      this.#pausedMedia.clear();
    }
    this.#markChanged();
  }

  animationsPaused(): boolean {
    return this.#animationsPaused;
  }

  #pauseActiveMotion(): void {
    if (!this.#animationsPaused) return;
    for (const animation of document.getAnimations?.() ?? []) {
      if (animation.playState !== "running") continue;
      try {
        animation.pause();
        this.#pausedAnimations.add(animation);
      } catch {
        // One host animation must not prevent the remaining page motion from pausing.
      }
    }
    for (const media of document.querySelectorAll<HTMLMediaElement>(
      "video, audio",
    )) {
      if (media.paused) continue;
      try {
        media.pause();
        this.#pausedMedia.add(media);
      } catch {
        // Media implementations can reject control while their source is changing.
      }
    }
    this.#motionPauseFrame = window.requestAnimationFrame(() => {
      this.#motionPauseFrame = null;
      this.#pauseActiveMotion();
    });
  }

  register(registration: BoundaryRegistration): () => void {
    this.#ensureActive();
    if (!isValidContextId(registration.id) || !registration.name.trim())
      throw new Error("boundary id must be bounded and name must not be empty");
    if (boundaryElements(registration).length === 0)
      throw new Error("boundary must contain at least one element");
    if (this.#boundaries.has(registration.id))
      throw new Error(`boundary '${registration.id}' is already registered`);
    const normalized: BoundaryRegistration = {
      ...registration,
      ...(registration.source
        ? {
            source: normalizeSourceSpan(registration.source, "boundary source"),
          }
        : {}),
      ...(registration.generated
        ? {
            generated: normalizeSourceSpan(
              registration.generated,
              "boundary generated source",
              true,
            ),
          }
        : {}),
    };
    this.#boundaries.set(normalized.id, normalized);
    this.#markChanged();
    return () => {
      if (this.#boundaries.get(normalized.id) === normalized) {
        this.#boundaries.delete(normalized.id);
        this.#markChanged();
      }
    };
  }

  registerBoundary(registration: BoundaryRegistration): () => void {
    return this.register(registration);
  }

  registerSource(registration: SourceRegistration): () => void {
    this.#ensureActive();
    const unregister = this.#sourceMappingStore.registerSource(registration);
    let active = true;
    this.#markChanged();
    return () => {
      if (!active) return;
      active = false;
      unregister();
      this.#markChanged();
    };
  }

  registerSourceMap(registration: SourceMapRegistration): () => void {
    this.#ensureActive();
    const unregister = this.#sourceMappingStore.registerSourceMap(registration);
    let active = true;
    this.#markChanged();
    return () => {
      if (!active) return;
      active = false;
      unregister();
      this.#markChanged();
    };
  }

  dispose(): void {
    if (this.#disposed) return;
    if (this.#animationsPaused) this.setAnimationsPaused(false);
    this.#disposed = true;
    for (const cleanup of this.#cleanup.splice(0)) cleanup();
    for (const waiter of this.#waiters) {
      clearTimeout(waiter.timer);
      waiter.finish(null);
    }
    this.#waiters.clear();
    this.#listeners.clear();
    this.#boundaries.clear();
    this.#history.clear();
    this.#sourceMappingStore.clear();
    this.#qualityStore.clear();
    this.#designAuditStore.clear();
    this.#uiStateTracker.clear();
    const host = window as unknown as Record<PropertyKey, unknown>;
    if (host[PAGE_CONTEXT_SYMBOL] === this) delete host[PAGE_CONTEXT_SYMBOL];
    if (currentRuntime === this) currentRuntime = null;
  }

  #markChanged(): void {
    if (this.#disposed || this.#pendingRevision) return;
    this.#pendingRevision = true;
    queueMicrotask(() => {
      if (this.#disposed) return;
      this.#pendingRevision = false;
      this.#revision += 1;
      const expiredDesignAuditReports = this.#designAuditStore.clear();
      this.#emit({ type: "context.revision", revision: this.#revision });
      for (const reportId of expiredDesignAuditReports) {
        this.#emit({ type: "design_audit.dismissed", reportId });
      }
      for (const waiter of Array.from(this.#waiters)) {
        if (this.#revision > waiter.revision) {
          clearTimeout(waiter.timer);
          this.#waiters.delete(waiter);
          waiter.finish(this.#revision);
        }
      }
    });
  }

  #emit(event: TestKitEvent): void {
    for (const listener of this.#listeners) {
      try {
        listener(structuredClone(event));
      } catch {
        // One integration listener must not break the page bridge.
      }
    }
  }

  #repairContext(draft: RepairDraft): {
    revision: number;
    context: RepairContext;
  } {
    const snapshot = this.snapshot({
      detail: "scoped",
      limits: { nodes: 200 },
    });
    const targetIds = new Set(draft.target.nodeIds);
    const nodes = snapshot.nodes.filter((node) => targetIds.has(node.id));
    const parentIds = new Set(
      nodes.flatMap((node) => (node.parentId ? [node.parentId] : [])),
    );
    const componentIds = new Set(
      nodes.flatMap((node) => (node.componentId ? [node.componentId] : [])),
    );
    const nearbyNodes = snapshot.nodes
      .filter(
        (node) =>
          !targetIds.has(node.id) &&
          (parentIds.has(node.id) ||
            (node.parentId != null && parentIds.has(node.parentId))),
      )
      .slice(0, 20);
    const component = snapshot.components.find((candidate) =>
      componentIds.has(candidate.id),
    );
    return {
      revision: snapshot.revision,
      context: {
        route: snapshot.page.route,
        title: snapshot.page.title,
        viewport: snapshot.page.viewport,
        ...(component ? { component } : {}),
        nodes,
        nearbyNodes,
        facts: snapshot.facts,
        ...(snapshot.ui ? { ui: snapshot.ui } : {}),
        untrusted: true,
      },
    };
  }

  #associateComponents(nodes: ContextNode[]): void {
    const boundaries = Array.from(this.#boundaries.values());
    for (const node of nodes) {
      const element = this.resolve(node.id);
      if (!element) continue;
      const owners = boundaries.filter((boundary) =>
        boundaryElements(boundary).some((root) =>
          composedContains(root, element),
        ),
      );
      const owner = owners.sort(
        (left, right) => boundaryDepth(right) - boundaryDepth(left),
      )[0];
      if (owner) node.componentId = owner.id;
    }
  }

  #associateSourceMappings(nodes: ContextNode[]): void {
    const boundaries = Array.from(this.#boundaries.values());
    for (const node of nodes) {
      const element = this.resolve(node.id);
      if (!element) continue;
      const sourceMapping = this.#sourceMappingStore.mappingFor(
        element,
        boundaries,
      );
      if (sourceMapping) node.sourceMapping = sourceMapping;
    }
  }

  #components(maxStringBytes: number): ContextComponent[] {
    return Array.from(this.#boundaries.values()).map((boundary) => {
      const boxes = boundaryElements(boundary).flatMap((element) =>
        Array.from(element.getClientRects()).map((rect) => ({
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
        })),
      );
      const facts = sanitizeFacts(
        safeCallback(boundary.facts, {}),
        maxStringBytes,
      );
      return {
        id: boundary.id,
        name: truncateUtf8(boundary.name, maxStringBytes),
        ...(boundary.source ? { source: boundary.source } : {}),
        ready: safeCallback(boundary.ready, true),
        facts: isJsonObject(facts) ? facts : {},
        boxes,
      };
    });
  }

  #page(): PageContextSnapshot["page"] {
    const root = document.documentElement;
    return {
      id: this.#options.page.id,
      url: location.href,
      route: `${location.pathname}${location.search}${location.hash}`,
      title: document.title,
      ready: safeCallback(
        this.#options.ready,
        document.readyState !== "loading",
      ),
      viewport: this.#viewport(),
      document: { width: root.scrollWidth, height: root.scrollHeight },
      scroll: { x: scrollX, y: scrollY },
      language: document.documentElement.lang || navigator.language || "unknown",
      theme: pageTheme(),
    };
  }

  #facts(maxStringBytes: number): PageContextSnapshot["facts"] {
    const facts = sanitizeFacts(
      safeCallback(this.#options.facts, {}),
      maxStringBytes,
    );
    return isJsonObject(facts) ? facts : {};
  }

  #viewport(): PageViewport {
    return {
      width: innerWidth,
      height: innerHeight,
      dpr: devicePixelRatio || 1,
      visual: visualViewportInfo(),
    };
  }

  #ensureActive(): void {
    if (this.#disposed) throw new Error("A3S Test Kit is disposed");
  }
}

let currentRuntime: Runtime | null = null;

export function installTestKit(options: TestKitOptions): TestKitRuntime {
  if (options.enabled !== true) return disabledBridge();
  if (typeof window === "undefined" || typeof document === "undefined") {
    throw new Error("A3S Test Kit can only be enabled in a browser");
  }
  if (currentRuntime) currentRuntime.dispose();
  const runtime = new Runtime({
    page: options.page,
    enabled: true,
    ready: options.ready,
    facts: options.facts,
    redact: options.redact ?? [],
    maxNodes: clamp(
      options.maxNodes ?? DEFAULT_CONTEXT_LIMITS.nodes,
      1,
      MAX_CONTEXT_LIMITS.nodes,
    ),
    maxStringBytes: clamp(
      options.maxStringBytes ?? DEFAULT_CONTEXT_LIMITS.stringBytes,
      32,
      MAX_CONTEXT_LIMITS.stringBytes,
    ),
    maxEncodedBytes: clamp(
      options.maxEncodedBytes ?? DEFAULT_CONTEXT_LIMITS.encodedBytes,
      1_024,
      MAX_CONTEXT_LIMITS.encodedBytes,
    ),
    uiUnderstanding: options.uiUnderstanding ?? true,
    maxUiNodes: clamp(
      options.maxUiNodes ?? DEFAULT_CONTEXT_LIMITS.uiNodes,
      1,
      MAX_CONTEXT_LIMITS.uiNodes,
    ),
    maxUiStateSamples: clamp(
      options.maxUiStateSamples ?? DEFAULT_CONTEXT_LIMITS.uiStateSamples,
      1,
      MAX_CONTEXT_LIMITS.uiStateSamples,
    ),
    maxUiDurationMs: clamp(
      options.maxUiDurationMs ?? DEFAULT_CONTEXT_LIMITS.uiDurationMs,
      1,
      MAX_CONTEXT_LIMITS.uiDurationMs,
    ),
    maxUiEncodedBytes: clamp(
      options.maxUiEncodedBytes ?? DEFAULT_CONTEXT_LIMITS.uiEncodedBytes,
      Math.min(
        8_192,
        MAX_CONTEXT_LIMITS.uiEncodedBytes,
        options.maxEncodedBytes ?? DEFAULT_CONTEXT_LIMITS.encodedBytes,
      ),
      Math.min(
        MAX_CONTEXT_LIMITS.uiEncodedBytes,
        options.maxEncodedBytes ?? DEFAULT_CONTEXT_LIMITS.encodedBytes,
      ),
    ),
    repairStorage: options.repairStorage ?? "session",
    maxQualityReports: clamp(options.maxQualityReports ?? 5, 1, 20),
    maxDesignAuditReports: clamp(options.maxDesignAuditReports ?? 5, 1, 20),
    repairEndpoint: options.repairEndpoint,
  });
  currentRuntime = runtime;
  Object.defineProperty(window, PAGE_CONTEXT_SYMBOL, {
    value: runtime,
    configurable: true,
    enumerable: false,
  });
  return runtime;
}

export function getPageContextBridge(): PageContextBridge | null {
  if (typeof window === "undefined") return null;
  return (
    ((window as unknown as Record<PropertyKey, unknown>)[
      PAGE_CONTEXT_SYMBOL
    ] as PageContextBridge | undefined) ?? null
  );
}

export function registerBoundary(
  registration: BoundaryRegistration,
): () => void {
  const bridge = getPageContextBridge();
  if (!bridge || !("registerBoundary" in bridge))
    throw new Error(
      "A3S Test Kit must be installed before registering a boundary",
    );
  return (bridge as TestKitRuntime).registerBoundary(registration);
}

export function registerSource(registration: SourceRegistration): () => void {
  const bridge = getPageContextBridge();
  if (!bridge || !("registerSource" in bridge))
    throw new Error(
      "A3S Test Kit must be installed before registering a source owner",
    );
  return (bridge as TestKitRuntime).registerSource(registration);
}

export function registerSourceMap(
  registration: SourceMapRegistration,
): () => void {
  const bridge = getPageContextBridge();
  if (!bridge || !("registerSourceMap" in bridge))
    throw new Error(
      "A3S Test Kit must be installed before registering a source map",
    );
  return (bridge as TestKitRuntime).registerSourceMap(registration);
}

function disabledBridge(): TestKitRuntime {
  const unavailable = () => {
    throw new Error("A3S Test Kit is disabled");
  };
  return {
    handshake: unavailable,
    probe: unavailable,
    snapshot: unavailable,
    resolve: () => null,
    waitForChange: async () => null,
    waitForDiff: async () => null,
    subscribe: () => () => undefined,
    submitRepair: () => [],
    takeRepairBatch: () => [],
    peekRepairBatch: () => [],
    listRepairs: () => [],
    listRepairBatches: () => [],
    exportRepairs: () => ({
      protocol: "a3s.test.repair/1",
      page: {
        id: "",
        url: "",
        route: "",
        revision: 0,
        viewport: { width: 0, height: 0, dpr: 1 },
      },
      findings: [],
    }),
    exportRepairsMarkdown: () => "",
    applyRepairEvent: () => null,
    submitRepairAction: () => null,
    takeRepairActions: () => [],
    addRepairReply: () => false,
    listRepairReplies: () => [],
    reportQuality: () => false,
    listQualityReports: () => [],
    dismissQualityFinding: () => false,
    dismissQualityReport: () => false,
    reportDesignAudit: () => false,
    listDesignAuditReports: () => [],
    dismissDesignAuditFinding: () => false,
    dismissDesignAuditReport: () => false,
    setAnimationsPaused: () => undefined,
    animationsPaused: () => false,
    registerBoundary: () => () => undefined,
    registerSource: () => () => undefined,
    registerSourceMap: () => () => undefined,
    dispose: () => undefined,
  };
}
