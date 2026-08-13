import type { Rect } from "./types";

export type SelectionMode =
  | "element"
  | "text"
  | "multi"
  | "area"
  | "draw"
  | "layout_place"
  | "layout_source"
  | "layout_destination";

export type OverlayTheme = "system" | "light" | "dark";
export type LayoutCanvas = "page" | "wireframe";
export type LayoutSource = { nodeId: string; label: string; originalRegion: Rect };

export const MODE_LABEL: Record<SelectionMode, string> = {
  element: "Element",
  text: "Text",
  multi: "Multi",
  area: "Area",
  draw: "Draw",
  layout_place: "Layout placement",
  layout_source: "Layout source",
  layout_destination: "Layout destination",
};

export const MODE_HINT: Record<SelectionMode, string> = {
  element: "Click one element, or focus it and press Enter, to create a finding.",
  text: "Select text, then release the pointer.",
  multi: "Drag across elements, or focus each element and press Enter to add it; press Shift+Enter to finish.",
  area: "Optional pointer mode: drag a rectangle over the page.",
  draw: "Optional pointer mode: draw a freehand mark around the relevant page area.",
  layout_place: "Drag the intended component region in viewport CSS pixels.",
  layout_source: "Click a section, or focus it and press Enter, to choose what should move.",
  layout_destination: "Drag the intended destination region in viewport CSS pixels.",
};
