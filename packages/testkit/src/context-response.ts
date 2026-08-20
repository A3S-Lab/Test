import packageManifest from "../package.json";
import { clamp } from "./runtime-values";
import {
  PAGE_CONTEXT_DIFF_PROTOCOL,
  PAGE_CONTEXT_PROTOCOL,
  type ContextDetail,
  type ContextLimits,
  type ContextNode,
  type ContextScope,
  type ContextSnapshotRequest,
  type PageContextDelta,
  type PageContextSnapshot,
} from "./types";

const CURSOR_VERSION = 1;
const MAX_CURSOR_BYTES = 4_096;
export const MAX_CONTEXT_WAIT_MS = 300_000;

export type ContextLimitCeilings = ContextLimits;

export type ContextCursorRequest = {
  detail: ContextDetail;
  scope: ContextScope;
  sinceRevision: number | null;
  ui: boolean;
  limits: ContextLimits;
};

export type ContextResponseInput = {
  revision: number;
  sourceNodes: ContextNode[];
  components: PageContextSnapshot["components"];
  removedNodeIds: string[];
  page: PageContextSnapshot["page"];
  facts: PageContextSnapshot["facts"];
  ui: PageContextSnapshot["ui"];
  delta: PageContextSnapshot["delta"];
  offset: number;
  cursorRequest: ContextCursorRequest;
  limits: ContextLimits;
};

export function normalizeContextLimits(
  requested: ContextSnapshotRequest["limits"],
  ceilings: ContextLimitCeilings,
): ContextLimits {
  if (
    requested &&
    Object.values(requested).some(
      (value) => value !== undefined && !Number.isFinite(value),
    )
  )
    throw new Error("page context limits must contain finite numbers");
  return {
    nodes: clamp(requested?.nodes ?? ceilings.nodes, 1, ceilings.nodes),
    stringBytes: clamp(
      requested?.stringBytes ?? ceilings.stringBytes,
      32,
      ceilings.stringBytes,
    ),
    encodedBytes: clamp(
      requested?.encodedBytes ?? ceilings.encodedBytes,
      1_024,
      ceilings.encodedBytes,
    ),
    uiNodes: clamp(requested?.uiNodes ?? ceilings.uiNodes, 1, ceilings.uiNodes),
    uiStateSamples: clamp(
      requested?.uiStateSamples ?? ceilings.uiStateSamples,
      1,
      ceilings.uiStateSamples,
    ),
    uiDurationMs: clamp(
      requested?.uiDurationMs ?? ceilings.uiDurationMs,
      1,
      ceilings.uiDurationMs,
    ),
    uiEncodedBytes: clamp(
      requested?.uiEncodedBytes ?? ceilings.uiEncodedBytes,
      Math.min(8_192, ceilings.uiEncodedBytes, ceilings.encodedBytes),
      Math.min(ceilings.uiEncodedBytes, ceilings.encodedBytes),
    ),
  };
}

export function normalizeContextScope(scope: ContextScope): ContextScope {
  if (!scope || typeof scope !== "object" || Array.isArray(scope))
    throw new Error("page context scope is invalid");
  if (scope.kind === "page" && hasOnlyKeys(scope, ["kind"]))
    return { kind: "page" };
  if (
    scope.kind === "node" &&
    hasOnlyKeys(scope, ["kind", "nodeId"]) &&
    isValidContextId(scope.nodeId)
  )
    return { kind: "node", nodeId: scope.nodeId };
  if (
    scope.kind === "component" &&
    hasOnlyKeys(scope, ["kind", "componentId"]) &&
    isValidContextId(scope.componentId)
  )
    return { kind: "component", componentId: scope.componentId };
  if (
    scope.kind === "region" &&
    hasOnlyKeys(scope, [
      "kind",
      "space",
      "x",
      "y",
      "width",
      "height",
    ]) &&
    (scope.space === "viewport" || scope.space === "document") &&
    [scope.x, scope.y, scope.width, scope.height].every(Number.isFinite) &&
    scope.width >= 0 &&
    scope.height >= 0
  )
    return {
      kind: "region",
      space: scope.space,
      x: scope.x,
      y: scope.y,
      width: scope.width,
      height: scope.height,
    };
  throw new Error("page context scope is invalid");
}

export function validateContextWaitTimeout(timeoutMs: number): number {
  if (
    !Number.isSafeInteger(timeoutMs) ||
    timeoutMs < 0 ||
    timeoutMs > MAX_CONTEXT_WAIT_MS
  )
    throw new Error(
      `page context wait timeout must be an integer from 0 through ${MAX_CONTEXT_WAIT_MS}`,
    );
  return timeoutMs;
}

export function validateContextSnapshotRequest(
  detail: ContextDetail,
  sinceRevision: number | null | undefined,
  currentRevision: number,
): void {
  if (!(["summary", "scoped", "diff", "forensic"] as const).includes(detail))
    throw new Error("page context detail profile is unsupported");
  if (detail === "diff") {
    if (sinceRevision === undefined || sinceRevision === null)
      throw new Error("diff snapshot requires sinceRevision");
    validateContextRevision(sinceRevision, currentRevision);
  } else if (sinceRevision !== undefined && sinceRevision !== null) {
    throw new Error("sinceRevision requires the diff detail profile");
  }
}

export function validateContextRevision(
  revision: number,
  currentRevision: number,
): void {
  if (
    !Number.isSafeInteger(revision) ||
    revision < 1 ||
    revision > currentRevision
  )
    throw new Error(
      "page context revision must identify a current or prior revision",
    );
}

export function decodeContextCursor(
  cursor: string | null | undefined,
  revision: number,
  request: ContextCursorRequest,
): number {
  if (cursor === undefined || cursor === null) return 0;
  if (!cursor || cursor.length > MAX_CURSOR_BYTES)
    throw new Error("page context cursor is invalid");
  let value: unknown;
  try {
    value = JSON.parse(atob(cursor));
  } catch {
    throw new Error("page context cursor is invalid");
  }
  if (!isCursorValue(value)) throw new Error("page context cursor is invalid");
  if (value.revision !== revision)
    throw new Error("page context cursor is stale for the current revision");
  if (value.request !== contextRequestDigest(request))
    throw new Error("page context cursor does not match the snapshot request");
  return value.offset;
}

export function buildContextResponse(
  input: ContextResponseInput,
): PageContextSnapshot {
  const total = input.sourceNodes.length;
  const end = Math.min(total, input.offset + input.limits.nodes);
  let nodes = input.sourceNodes.slice(input.offset, end);
  let components = input.components;
  let removedNodeIds = input.removedNodeIds;
  let facts = input.facts;
  let ui = input.ui;
  let delta = input.delta;
  let page = { ...input.page };
  let metadataTruncated = false;

  let response = responseFor({
    ...input,
    nodes,
    components,
    removedNodeIds,
    facts,
    ui,
    delta,
    page,
    metadataTruncated,
  });
  if (encodedBytes(response) <= input.limits.encodedBytes) return response;

  if (ui !== undefined) {
    ui = undefined;
    metadataTruncated = true;
  }
  response = responseFor({
    ...input,
    nodes,
    components,
    removedNodeIds,
    facts,
    ui,
    delta,
    page,
    metadataTruncated,
  });

  while (nodes.length > 0 && encodedBytes(response) > input.limits.encodedBytes) {
    nodes = nodes.slice(0, -1);
    response = responseFor({
      ...input,
      nodes,
      components,
      removedNodeIds,
      facts,
      ui,
      delta,
      page,
      metadataTruncated,
    });
  }
  if (fitsWithProgress(response, input.offset, total, input.limits.encodedBytes))
    return response;

  if (components.length > 0) {
    components = [];
    metadataTruncated = true;
  }
  if (Object.keys(facts).length > 0) {
    facts = {};
    metadataTruncated = true;
  }
  response = responseFor({
    ...input,
    nodes,
    components,
    removedNodeIds,
    facts,
    ui,
    delta,
    page,
    metadataTruncated,
  });
  if (fitsWithProgress(response, input.offset, total, input.limits.encodedBytes))
    return response;

  if (removedNodeIds.length > 0) {
    removedNodeIds = [];
    metadataTruncated = true;
  }
  page = compactPage(page, false);
  metadataTruncated = true;
  response = responseFor({
    ...input,
    nodes,
    components,
    removedNodeIds,
    facts,
    ui,
    delta,
    page,
    metadataTruncated,
  });
  if (fitsWithProgress(response, input.offset, total, input.limits.encodedBytes))
    return response;

  if (input.offset < total) {
    nodes = [compactNode(input.sourceNodes[input.offset]!)];
    response = responseFor({
      ...input,
      nodes,
      components,
      removedNodeIds,
      facts,
      ui,
      delta,
      page,
      metadataTruncated: true,
    });
    if (encodedBytes(response) <= input.limits.encodedBytes) return response;
  }

  if (delta?.status === "complete") {
    delta = resetDelta(delta);
    nodes = [];
    removedNodeIds = [];
    page = compactPage(page, true);
    response = responseFor({
      ...input,
      nodes,
      components: [],
      removedNodeIds,
      facts: {},
      ui: undefined,
      delta,
      page,
      metadataTruncated: true,
      stopPagination: true,
    });
    if (encodedBytes(response) <= input.limits.encodedBytes) return response;
    throw new Error("page context reset cannot fit the encoded byte limit");
  }

  page = compactPage(page, true);
  nodes =
    input.offset < total ? [minimalNode(input.sourceNodes[input.offset]!)] : [];
  response = responseFor({
    ...input,
    nodes,
    components: [],
    removedNodeIds: [],
    facts: {},
    ui: undefined,
    delta,
    page,
    metadataTruncated: true,
  });
  if (encodedBytes(response) <= input.limits.encodedBytes) return response;

  response = responseFor({
    ...input,
    nodes: [],
    components: [],
    removedNodeIds: [],
    facts: {},
    ui: undefined,
    delta,
    page,
    metadataTruncated: true,
    stopPagination: true,
  });
  if (encodedBytes(response) <= input.limits.encodedBytes) return response;
  throw new Error("page context response cannot fit the encoded byte limit");
}

export function encodedBytes(value: unknown): number {
  return new TextEncoder().encode(JSON.stringify(value)).byteLength;
}

function responseFor(
  input: ContextResponseInput & {
    nodes: ContextNode[];
    metadataTruncated: boolean;
    stopPagination?: boolean;
  },
): PageContextSnapshot {
  const nextOffset = input.offset + input.nodes.length;
  const moreNodes =
    !input.stopPagination && nextOffset < input.sourceNodes.length;
  return {
    protocol: PAGE_CONTEXT_PROTOCOL,
    sdkVersion: packageManifest.version,
    revision: input.revision,
    page: input.page,
    components: input.components,
    nodes: input.nodes,
    facts: input.facts,
    ...(input.ui ? { ui: input.ui } : {}),
    ...(input.delta ? { delta: input.delta } : {}),
    removedNodeIds: input.removedNodeIds,
    truncated: input.metadataTruncated || moreNodes,
    nextCursor: moreNodes
      ? encodeContextCursor(input.revision, nextOffset, input.cursorRequest)
      : null,
  };
}

function fitsWithProgress(
  response: PageContextSnapshot,
  offset: number,
  total: number,
  limit: number,
): boolean {
  return (
    encodedBytes(response) <= limit &&
    (offset >= total || response.nodes.length > 0 || response.nextCursor === null)
  );
}

function encodeContextCursor(
  revision: number,
  offset: number,
  request: ContextCursorRequest,
): string {
  return btoa(
    JSON.stringify({
      version: CURSOR_VERSION,
      revision,
      offset,
      request: contextRequestDigest(request),
    }),
  );
}

function contextRequestDigest(request: ContextCursorRequest): string {
  const value = JSON.stringify({
    detail: request.detail,
    scope: normalizeContextScope(request.scope),
    sinceRevision: request.sinceRevision,
    ui: request.ui,
    limits: request.limits,
  });
  const bytes = new TextEncoder().encode(value);
  let left = 0x811c9dc5;
  let right = 0x9e3779b9;
  for (const byte of bytes) {
    left = Math.imul(left ^ byte, 0x01000193) >>> 0;
    right = Math.imul(right ^ byte, 0x85ebca6b) >>> 0;
  }
  return `${left.toString(16).padStart(8, "0")}${right
    .toString(16)
    .padStart(8, "0")}:${bytes.length}`;
}

export function isValidContextId(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    new TextEncoder().encode(value).byteLength <= 256 &&
    !Array.from(value).some((character) => /\p{Cc}/u.test(character))
  );
}

function hasOnlyKeys(value: object, keys: readonly string[]): boolean {
  const admitted = new Set(keys);
  const actual = Object.keys(value);
  return actual.length === admitted.size && actual.every((key) => admitted.has(key));
}

function isCursorValue(
  value: unknown,
): value is {
  version: number;
  revision: number;
  offset: number;
  request: string;
} {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  return (
    Object.keys(candidate).length === 4 &&
    candidate.version === CURSOR_VERSION &&
    Number.isSafeInteger(candidate.revision) &&
    Number.isSafeInteger(candidate.offset) &&
    (candidate.offset as number) >= 0 &&
    typeof candidate.request === "string"
  );
}

function compactPage(
  page: PageContextSnapshot["page"],
  includeId: boolean,
): PageContextSnapshot["page"] {
  return {
    ...page,
    id: includeId ? "" : page.id,
    url: "",
    route: "",
    title: "",
    language: "",
  };
}

function compactNode(node: ContextNode): ContextNode {
  return {
    id: node.id,
    ...(node.parentId ? { parentId: node.parentId } : {}),
    ...(node.componentId ? { componentId: node.componentId } : {}),
    tag: node.tag,
    ...(node.role ? { role: node.role.slice(0, 128) } : {}),
    ...(node.testId ? { testId: node.testId.slice(0, 128) } : {}),
    state: node.state,
    locators: node.locators.slice(0, 1).map((locator) =>
      "value" in locator
        ? { ...locator, value: locator.value.slice(0, 128) }
        : {
            ...locator,
            role: locator.role.slice(0, 128),
            name: locator.name.slice(0, 128),
          },
    ),
  };
}

function minimalNode(node: ContextNode): ContextNode {
  return {
    id: node.id,
    tag: node.tag.slice(0, 64),
    state: { visible: node.state.visible },
    locators: [],
  };
}

function resetDelta(delta: PageContextDelta): PageContextDelta {
  return {
    protocol: PAGE_CONTEXT_DIFF_PROTOCOL,
    fromRevision: delta.fromRevision,
    toRevision: delta.toRevision,
    status: "reset_required",
    invalidated: {
      all: true,
      page: true,
      facts: true,
      ui: true,
      nodeIds: [],
      componentIds: [],
    },
  };
}
