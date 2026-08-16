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
