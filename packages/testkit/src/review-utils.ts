import type { CSSProperties } from "react";
import type {
  PageContextBridge,
  RepairDraft,
  RepairTarget,
  Rect,
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

export function viewportScroll(): { x: number; y: number } {
  if (typeof window === "undefined") return { x: 0, y: 0 };
  return { x: window.scrollX, y: window.scrollY };
}

export function currentTargetRegion(target: RepairTarget): Rect | undefined {
  if (!target.region || !target.regionScroll || typeof window === "undefined") return target.region;
  const scroll = viewportScroll();
  return {
    ...target.region,
    x: target.region.x + target.regionScroll.x - scroll.x,
    y: target.region.y + target.regionScroll.y - scroll.y,
  };
}

export function validLayoutRect(rect: Rect): boolean {
  return [rect.x, rect.y, rect.width, rect.height].every(Number.isFinite)
    && rect.width >= 8
    && rect.height >= 8;
}

export function markerRects(target: RepairTarget, bridge: PageContextBridge | null): RectLike[] {
  if (!bridge) return [];
  const region = currentTargetRegion(target);
  if (target.layout && region) return [region];
  const nodeRects = target.nodeIds.flatMap((nodeId) => {
    const element = bridge.resolve(nodeId);
    return element ? [element.getBoundingClientRect()] : [];
  });
  if ((target.kind === "node" || target.kind === "text") && nodeRects.length > 0) {
    return [boundingRect(nodeRects)];
  }
  if (region) return [region];
  return nodeRects.length > 0 ? [boundingRect(nodeRects)] : [];
}

function boundingRect(rects: RectLike[]): Rect {
  const left = Math.min(...rects.map((rect) => rect.x));
  const top = Math.min(...rects.map((rect) => rect.y));
  const right = Math.max(...rects.map((rect) => rect.x + rect.width));
  const bottom = Math.max(...rects.map((rect) => rect.y + rect.height));
  return { x: left, y: top, width: right - left, height: bottom - top };
}

export function repairId(prefix: string): string {
  return `finding-${prefix}-${globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`}`;
}

export type RectLike = Pick<DOMRect, "x" | "y" | "width" | "height">;

export function stableList(values: readonly string[] | undefined): string {
  return JSON.stringify(values ?? []);
}

export function structuredDraft(value: RepairDraft): RepairDraft {
  return structuredClone(value);
}
