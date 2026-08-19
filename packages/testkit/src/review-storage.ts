import type {
  ContextNode,
  LocatorCandidate,
  PageContextBridge,
  RepairDraft,
  RepairTarget,
} from "./types";
import { validDesignReference } from "./design-reference";

const STORAGE_PREFIX = "a3s-test.review-drafts/1/";
const STORAGE_VERSION = 1;
const RETENTION_MS = 7 * 24 * 60 * 60 * 1_000;
const MAX_CLOCK_SKEW_MS = 5 * 60 * 1_000;
const MAX_ENCODED_BYTES = 512 * 1_024;
const MAX_ITEMS = 100;
const MAX_ANCHORS_PER_ITEM = 256;
const MAX_LOCATORS_PER_ANCHOR = 5;
const MAX_LOCATOR_BYTES = 512;

export type ReviewDraftItem = {
  draft: RepairDraft;
  selected: boolean;
  hidden: boolean;
};

export type ReviewScope = {
  pageId: string;
  route: string;
};

type StoredAnchor = {
  locators: LocatorCandidate[];
};

type StoredItem = {
  draft: RepairDraft;
  selected: boolean;
  hidden: boolean;
  anchors: StoredAnchor[];
};

type StoredReviewDrafts = {
  version: typeof STORAGE_VERSION;
  pageId: string;
  route: string;
  savedAt: number;
  items: StoredItem[];
};

export function reviewScope(bridge: PageContextBridge): ReviewScope {
  const page = bridge.snapshot({ detail: "summary", limits: { nodes: 1 } }).page;
  return { pageId: page.id, route: page.route };
}

export function reviewDraftStorageKey(scope: ReviewScope): string {
  return `${STORAGE_PREFIX}${encodeURIComponent(scope.pageId)}/${encodeURIComponent(scope.route)}`;
}

export function saveReviewDrafts(
  bridge: PageContextBridge,
  items: readonly ReviewDraftItem[],
  storage: Storage = window.localStorage,
  now = Date.now(),
): void {
  const snapshot = bridge.snapshot({ detail: "forensic", limits: { nodes: 5_000 } });
  const scope = { pageId: snapshot.page.id, route: snapshot.page.route };
  const key = reviewDraftStorageKey(scope);
  if (items.length === 0) {
    remove(storage, key);
    return;
  }

  const nodes = new Map(snapshot.nodes.map((node) => [node.id, node]));
  const stored: StoredItem[] = [];
  for (const item of items.slice(0, MAX_ITEMS)) {
    const candidate = storedItem(item, nodes);
    if (!candidate) continue;
    const record: StoredReviewDrafts = {
      version: STORAGE_VERSION,
      pageId: scope.pageId,
      route: scope.route,
      savedAt: now,
      items: [...stored, candidate],
    };
    if (encodedBytes(record) > MAX_ENCODED_BYTES) break;
    stored.push(candidate);
  }

  if (stored.length === 0) {
    remove(storage, key);
    return;
  }
  try {
    storage.setItem(key, JSON.stringify({
      version: STORAGE_VERSION,
      pageId: scope.pageId,
      route: scope.route,
      savedAt: now,
      items: stored,
    } satisfies StoredReviewDrafts));
  } catch {
    // Disabled or exhausted browser storage must not break the host page.
  }
}

export function loadReviewDrafts(
  bridge: PageContextBridge,
  storage: Storage = window.localStorage,
  now = Date.now(),
): ReviewDraftItem[] {
  const snapshot = bridge.snapshot({ detail: "forensic", limits: { nodes: 5_000 } });
  const scope = { pageId: snapshot.page.id, route: snapshot.page.route };
  const key = reviewDraftStorageKey(scope);
  let encoded: string | null;
  try {
    encoded = storage.getItem(key);
  } catch {
    return [];
  }
  if (!encoded) return [];
  if (new TextEncoder().encode(encoded).byteLength > MAX_ENCODED_BYTES) {
    remove(storage, key);
    return [];
  }

  let value: unknown;
  try {
    value = JSON.parse(encoded);
  } catch {
    remove(storage, key);
    return [];
  }
  if (!validRecord(value, scope, now)) {
    remove(storage, key);
    return [];
  }

  const restored: ReviewDraftItem[] = [];
  for (const item of value.items) {
    const nodeIds = rebind(item.anchors, snapshot.nodes);
    if (!nodeIds) continue;
    const draft = structuredClone(item.draft);
    draft.target.nodeIds = nodeIds;
    if (!validDraft(draft)) continue;
    restored.push({ draft, selected: item.selected, hidden: item.hidden });
  }
  return restored;
}

export function clearReviewDrafts(
  scope: ReviewScope,
  storage: Storage = window.localStorage,
): void {
  remove(storage, reviewDraftStorageKey(scope));
}

function storedItem(
  item: ReviewDraftItem,
  nodes: ReadonlyMap<string, ContextNode>,
): StoredItem | null {
  if (!validDraft(item.draft) || typeof item.selected !== "boolean" || typeof item.hidden !== "boolean") {
    return null;
  }
  if (item.draft.target.nodeIds.length > MAX_ANCHORS_PER_ITEM) return null;
  const anchors: StoredAnchor[] = [];
  for (const nodeId of item.draft.target.nodeIds) {
    const node = nodes.get(nodeId);
    const locators = node?.locators.filter(isSemanticLocator).slice(0, MAX_LOCATORS_PER_ANCHOR) ?? [];
    if (locators.length === 0) return null;
    anchors.push({ locators: structuredClone(locators) });
  }
  const draft = structuredClone(item.draft);
  draft.target.nodeIds = [];
  return { draft, selected: item.selected, hidden: item.hidden, anchors };
}

function rebind(anchors: readonly StoredAnchor[], nodes: readonly ContextNode[]): string[] | null {
  const result: string[] = [];
  for (const anchor of anchors) {
    const nodeId = resolveAnchor(anchor, nodes);
    if (!nodeId || result.includes(nodeId)) return null;
    result.push(nodeId);
  }
  return result;
}

function resolveAnchor(anchor: StoredAnchor, nodes: readonly ContextNode[]): string | null {
  for (const locator of anchor.locators) {
    const matches = nodes.filter((node) => node.locators.some((candidate) => sameLocator(locator, candidate)));
    if (matches.length > 1) return null;
    if (matches.length === 1) return matches[0]!.id;
  }
  return null;
}

function sameLocator(left: LocatorCandidate, right: LocatorCandidate): boolean {
  if (left.type !== right.type) return false;
  if (left.type === "role" && right.type === "role") {
    return left.role === right.role && left.name === right.name;
  }
  if (left.type === "text" && right.type === "text") {
    return left.value === right.value && left.exact === right.exact;
  }
  return "value" in left && "value" in right && left.value === right.value;
}

function validRecord(value: unknown, scope: ReviewScope, now: number): value is StoredReviewDrafts {
  if (!isObject(value) || !onlyKeys(value, ["version", "pageId", "route", "savedAt", "items"])) return false;
  if (
    value.version !== STORAGE_VERSION ||
    value.pageId !== scope.pageId ||
    value.route !== scope.route ||
    typeof value.savedAt !== "number" ||
    !Number.isFinite(value.savedAt) ||
    value.savedAt < now - RETENTION_MS ||
    value.savedAt > now + MAX_CLOCK_SKEW_MS ||
    !Array.isArray(value.items) ||
    value.items.length > MAX_ITEMS
  ) return false;
  return value.items.every(validStoredItem);
}

function validStoredItem(value: unknown): value is StoredItem {
  if (!isObject(value) || !onlyKeys(value, ["draft", "selected", "hidden", "anchors"])) return false;
  if (typeof value.selected !== "boolean" || typeof value.hidden !== "boolean" || !Array.isArray(value.anchors)) return false;
  if (value.anchors.length > MAX_ANCHORS_PER_ITEM || !validStoredDraft(value.draft)) return false;
  return value.anchors.every((anchor) => {
    if (!isObject(anchor) || !onlyKeys(anchor, ["locators"]) || !Array.isArray(anchor.locators)) return false;
    return anchor.locators.length > 0 && anchor.locators.length <= MAX_LOCATORS_PER_ANCHOR && anchor.locators.every(validSemanticLocator);
  });
}

function validStoredDraft(value: unknown): value is RepairDraft {
  if (!validDraft(value)) {
    if (!isObject(value) || !isObject(value.target)) return false;
    const restored = structuredClone(value) as RepairDraft;
    restored.target.nodeIds = Array.from({ length: 1 }, () => "stored-anchor");
    if (restored.target.layout?.kind === "placement") restored.target.nodeIds = [];
    return validDraft(restored);
  }
  return value.target.nodeIds.length === 0;
}

function validDraft(value: unknown): value is RepairDraft {
  if (!isObject(value) || !onlyKeys(value, ["id", "instruction", "successCriteria", "intent", "severity", "relations", "designReference", "target", "createdAt"])) return false;
  if (
    !boundedString(value.id, 1, 128) ||
    !boundedString(value.instruction, 1, 8_192, true) ||
    (value.successCriteria !== undefined && !boundedString(value.successCriteria, 1, 4_096, true)) ||
    !["fix", "change", "question", "approve"].includes(String(value.intent)) ||
    !["blocking", "important", "suggestion"].includes(String(value.severity)) ||
    !boundedString(value.createdAt, 1, 64) ||
    (value.designReference !== undefined && (
      !validDesignReference(value.designReference) || value.designReference.image.kind !== "inline"
    )) ||
    !validTarget(value.target)
  ) return false;
  if (value.relations === undefined) return true;
  if (!Array.isArray(value.relations) || value.relations.length > 100) return false;
  const ids = new Set<string>();
  return value.relations.every((relation) => {
    if (!isObject(relation) || !onlyKeys(relation, ["kind", "findingId"])) return false;
    if (relation.kind !== "conflicts_with" || !boundedString(relation.findingId, 1, 128) || relation.findingId === value.id || ids.has(relation.findingId)) return false;
    ids.add(relation.findingId);
    return true;
  });
}

function validTarget(value: unknown): value is RepairTarget {
  if (!isObject(value) || !onlyKeys(value, ["kind", "nodeIds", "selectedText", "region", "drawing", "layout"])) return false;
  if (!["node", "text", "region", "drawing"].includes(String(value.kind)) || !Array.isArray(value.nodeIds)) return false;
  if (value.nodeIds.length > 5_000 || value.nodeIds.some((id) => !boundedString(id, 1, 128))) return false;
  if (value.selectedText !== undefined && !boundedString(value.selectedText, 0, 4_096)) return false;
  if (value.region !== undefined && !validRect(value.region)) return false;
  if (value.drawing !== undefined) {
    if (!Array.isArray(value.drawing) || value.drawing.length > 2_000) return false;
    if (value.drawing.some((point) => !isObject(point) || !onlyKeys(point, ["x", "y"]) || !finite(point.x) || !finite(point.y))) return false;
  }
  if (value.layout === undefined) return true;
  if (!isObject(value.layout) || (value.layout.purpose !== undefined && !boundedString(value.layout.purpose, 0, 2_048))) return false;
  if (value.layout.kind === "placement") {
    return onlyKeys(value.layout, ["kind", "componentType", "canvas", "purpose"])
      && boundedString(value.layout.componentType, 1, 128, true)
      && ["page", "wireframe"].includes(String(value.layout.canvas))
      && value.nodeIds.length === 0
      && value.region !== undefined;
  }
  return value.layout.kind === "rearrange"
    && onlyKeys(value.layout, ["kind", "originalRegion", "purpose"])
    && validRect(value.layout.originalRegion)
    && value.nodeIds.length > 0
    && value.region !== undefined;
}

function validRect(value: unknown): boolean {
  return isObject(value)
    && onlyKeys(value, ["x", "y", "width", "height"])
    && finite(value.x)
    && finite(value.y)
    && finite(value.width)
    && finite(value.height)
    && value.width > 0
    && value.height > 0;
}

function isSemanticLocator(locator: LocatorCandidate): boolean {
  return locator.type !== "css" && validSemanticLocator(locator);
}

function validSemanticLocator(value: unknown): value is LocatorCandidate {
  if (!isObject(value) || typeof value.type !== "string" || value.type === "css") return false;
  if (value.type === "role") {
    return onlyKeys(value, ["type", "role", "name"])
      && boundedString(value.role, 1, MAX_LOCATOR_BYTES)
      && boundedString(value.name, 1, MAX_LOCATOR_BYTES);
  }
  if (value.type === "text") {
    return onlyKeys(value, ["type", "value", "exact"])
      && boundedString(value.value, 1, MAX_LOCATOR_BYTES)
      && typeof value.exact === "boolean";
  }
  return ["label", "test_id", "placeholder"].includes(value.type)
    && onlyKeys(value, ["type", "value"])
    && boundedString(value.value, 1, MAX_LOCATOR_BYTES);
}

function boundedString(value: unknown, min: number, max: number, trimmed = false): value is string {
  return typeof value === "string"
    && value.length >= min
    && value.length <= max
    && (!trimmed || value.trim().length >= min);
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function onlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  const keys = new Set(allowed);
  return Object.keys(value).every((key) => keys.has(key));
}

function encodedBytes(value: unknown): number {
  return new TextEncoder().encode(JSON.stringify(value)).byteLength;
}

function remove(storage: Storage, key: string): void {
  try {
    storage.removeItem(key);
  } catch {
    // Disabled browser storage must not break the host page.
  }
}
