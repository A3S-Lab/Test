import {
  PAGE_CONTEXT_DIFF_PROTOCOL,
  type ContextComponent,
  type ContextDetail,
  type ContextNode,
  type ContextScope,
  type JsonValue,
  type PageContextDelta,
  type PageContextSnapshot,
} from "./types";

const MAX_HISTORY_PROJECTIONS = 8;
const MAX_REVISIONS_PER_PROJECTION = 12;
const MAX_INVALIDATED_IDS = 10_000;
const UTF8_ENCODER = new TextEncoder();

type RevisionState = {
  nodeHashes: Map<string, string>;
  componentHashes: Map<string, string>;
  pageHash: string;
  factsHash: string;
};

export type ContextProjection = {
  revision: number;
  detail: ContextDetail;
  scope: ContextScope;
  maxStringBytes: number;
  page: PageContextSnapshot["page"];
  components: ContextComponent[];
  nodes: ContextNode[];
  facts: Record<string, JsonValue>;
};

export type ContextHistoryResult = {
  components: ContextComponent[];
  nodes: ContextNode[];
  removedNodeIds: string[];
  delta?: PageContextDelta;
};

export class ContextHistory {
  readonly #projections = new Map<string, Map<number, RevisionState>>();

  project(
    projection: ContextProjection,
    sinceRevision: number | null | undefined,
  ): ContextHistoryResult {
    const key = projectionKey(projection);
    const current = revisionState(projection);
    let result: ContextHistoryResult = {
      components: projection.components,
      nodes: projection.nodes,
      removedNodeIds: [],
    };

    if (
      projection.detail === "diff" &&
      sinceRevision !== undefined &&
      sinceRevision !== null
    ) {
      const baseline =
        sinceRevision === projection.revision
          ? current
          : this.#projections.get(key)?.get(sinceRevision);
      result = baseline
        ? completeDiff(projection, baseline, current, sinceRevision)
        : resetDiff(projection, sinceRevision);
    }

    this.#remember(key, projection.revision, current);
    return result;
  }

  clear(): void {
    this.#projections.clear();
  }

  #remember(key: string, revision: number, state: RevisionState): void {
    let revisions = this.#projections.get(key);
    if (!revisions) {
      revisions = new Map<number, RevisionState>();
      this.#projections.set(key, revisions);
    } else {
      this.#projections.delete(key);
      this.#projections.set(key, revisions);
    }
    revisions.delete(revision);
    revisions.set(revision, state);
    while (revisions.size > MAX_REVISIONS_PER_PROJECTION) {
      const oldest = revisions.keys().next().value as number | undefined;
      if (oldest === undefined) break;
      revisions.delete(oldest);
    }
    while (this.#projections.size > MAX_HISTORY_PROJECTIONS) {
      const oldest = this.#projections.keys().next().value as
        | string
        | undefined;
      if (oldest === undefined) break;
      this.#projections.delete(oldest);
    }
  }
}

function completeDiff(
  projection: ContextProjection,
  baseline: RevisionState,
  current: RevisionState,
  sinceRevision: number,
): ContextHistoryResult {
  const nodes = projection.nodes.filter(
    (node) =>
      baseline.nodeHashes.get(node.id) !== current.nodeHashes.get(node.id),
  );
  const removedNodeIds = Array.from(baseline.nodeHashes.keys())
    .filter((id) => !current.nodeHashes.has(id))
    .sort(compareUtf8);
  const components = projection.components.filter(
    (component) =>
      baseline.componentHashes.get(component.id) !==
      current.componentHashes.get(component.id),
  );
  const changedComponentIds = components.map((component) => component.id);
  const removedComponentIds = Array.from(baseline.componentHashes.keys())
    .filter((id) => !current.componentHashes.has(id))
    .sort(compareUtf8);
  const nodeIds = canonicalIds([
    ...nodes.map((node) => node.id),
    ...removedNodeIds,
  ]);
  const componentIds = canonicalIds([
    ...changedComponentIds,
    ...removedComponentIds,
  ]);
  if (
    nodeIds.length > MAX_INVALIDATED_IDS ||
    componentIds.length > MAX_INVALIDATED_IDS
  )
    return resetDiff(projection, sinceRevision);
  return {
    components,
    nodes,
    removedNodeIds,
    delta: {
      protocol: PAGE_CONTEXT_DIFF_PROTOCOL,
      fromRevision: sinceRevision,
      toRevision: projection.revision,
      status: "complete",
      invalidated: {
        all: false,
        page: baseline.pageHash !== current.pageHash,
        facts: baseline.factsHash !== current.factsHash,
        ui: sinceRevision !== projection.revision,
        nodeIds,
        componentIds,
      },
    },
  };
}

function resetDiff(
  projection: ContextProjection,
  sinceRevision: number,
): ContextHistoryResult {
  return {
    components: projection.components,
    nodes: projection.nodes,
    removedNodeIds: [],
    delta: {
      protocol: PAGE_CONTEXT_DIFF_PROTOCOL,
      fromRevision: sinceRevision,
      toRevision: projection.revision,
      status: "reset_required",
      invalidated: {
        all: true,
        page: true,
        facts: true,
        ui: true,
        nodeIds: [],
        componentIds: [],
      },
    },
  };
}

function revisionState(projection: ContextProjection): RevisionState {
  return {
    nodeHashes: new Map(
      projection.nodes.map((node) => [node.id, JSON.stringify(node)]),
    ),
    componentHashes: new Map(
      projection.components.map((component) => [
        component.id,
        JSON.stringify(component),
      ]),
    ),
    pageHash: JSON.stringify(projection.page),
    factsHash: JSON.stringify(projection.facts),
  };
}

function projectionKey(projection: ContextProjection): string {
  const profile = projection.detail === "forensic" ? "forensic" : "semantic";
  return JSON.stringify({
    profile,
    scope: projection.scope,
    maxStringBytes: projection.maxStringBytes,
  });
}

function compareUtf8(left: string, right: string): number {
  const leftBytes = UTF8_ENCODER.encode(left);
  const rightBytes = UTF8_ENCODER.encode(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = leftBytes[index]! - rightBytes[index]!;
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

function canonicalIds(values: string[]): string[] {
  return Array.from(new Set(values)).sort(compareUtf8);
}
