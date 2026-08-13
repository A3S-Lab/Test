import type { CSSProperties } from "react";
import type {
  PageContextBridge,
  RepairDraft,
  RepairStatus,
  RepairTarget,
  Rect,
  SubmittedRepair,
} from "./types";
import type { ReviewDraftItem } from "./review-storage";

export function removeDraft(items: ReviewDraftItem[], findingId: string): ReviewDraftItem[] {
  return items
    .filter((item) => item.draft.id !== findingId)
    .map((item) => {
      const relations = item.draft.relations?.filter((relation) => relation.findingId !== findingId);
      if (relations?.length === item.draft.relations?.length) return item;
      const draft = { ...item.draft };
      if (relations?.length) draft.relations = relations;
      else delete draft.relations;
      return { ...item, draft };
    });
}

export function normalizedArea(startX: number, startY: number, endX: number, endY: number): Rect {
  return { x: Math.min(startX, endX), y: Math.min(startY, endY), width: Math.abs(endX - startX), height: Math.abs(endY - startY) };
}

export function appendDrawingPoint(points: Array<{ x: number; y: number }>, x: number, y: number) {
  const previous = points.at(-1);
  if (previous && Math.hypot(previous.x - x, previous.y - y) < 2) return points;
  return [...points, { x, y }];
}

export function drawingBounds(points: Array<{ x: number; y: number }>): Rect {
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);
  return { x: left, y: top, width: Math.max(1, Math.max(...xs) - left), height: Math.max(1, Math.max(...ys) - top) };
}

export function rectStyle(rect: Pick<DOMRect, "x" | "y" | "width" | "height">): CSSProperties {
  return { left: rect.x, top: rect.y, width: rect.width, height: rect.height };
}

export function rectValue(rect: Pick<DOMRect, "x" | "y" | "width" | "height">): Rect {
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
}

export function validLayoutRect(rect: Rect): boolean {
  return [rect.x, rect.y, rect.width, rect.height].every(Number.isFinite)
    && rect.width >= 8
    && rect.height >= 8;
}

export function markerRects(target: RepairTarget, bridge: PageContextBridge | null): RectLike[] {
  if (!bridge) return [];
  if (target.layout && target.region) return [target.region];
  const nodeRects = target.nodeIds.flatMap((nodeId) => {
    const element = bridge.resolve(nodeId);
    return element ? [element.getBoundingClientRect()] : [];
  });
  return nodeRects.length > 0 ? nodeRects : target.region ? [target.region] : [];
}

export function repairId(prefix: string): string {
  return `finding-${prefix}-${globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`}`;
}

export function targetSummary(target: RepairTarget): string {
  if (target.layout?.kind === "placement") return `layout placement · ${target.layout.componentType} · ${target.layout.canvas}`;
  if (target.layout?.kind === "rearrange") return `layout rearrangement · ${target.nodeIds.length} element${target.nodeIds.length === 1 ? "" : "s"}`;
  if (target.kind === "text") return `text · ${target.selectedText?.slice(0, 36) ?? "selection"}`;
  if (target.kind === "region") return `area · ${target.nodeIds.length} elements`;
  if (target.kind === "drawing") return `drawing · ${target.nodeIds.length} elements`;
  return `${target.nodeIds.length} element${target.nodeIds.length === 1 ? "" : "s"}`;
}

export function statusLabel(status: RepairStatus): string {
  return status.replaceAll("_", " ");
}

export function repairAnnouncement(repair: SubmittedRepair): string {
  if (repair.status === "needs_input") return `Repair needs input: ${repair.instruction}`;
  if (repair.status === "review_ready") return `Repair ready for review: ${repair.instruction}`;
  return `Repair ${statusLabel(repair.status)}: ${repair.instruction}`;
}

export type RectLike = Pick<DOMRect, "x" | "y" | "width" | "height">;

export function stableList(values: readonly string[] | undefined): string {
  return JSON.stringify(values ?? []);
}

export function structuredDraft(value: RepairDraft): RepairDraft {
  return structuredClone(value);
}
