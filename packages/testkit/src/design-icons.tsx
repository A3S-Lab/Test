export type DesignGlyphName =
  | "board"
  | "capture"
  | "check"
  | "close"
  | "draw"
  | "image"
  | "rectangle"
  | "redo"
  | "select"
  | "text"
  | "trash"
  | "undo"
  | "upload";

export function DesignGlyph({ name }: { name: DesignGlyphName }) {
  const common = {
    viewBox: "0 0 20 20",
    "aria-hidden": true,
    focusable: "false",
  } as const;
  if (name === "board") return <svg {...common}><rect x="3" y="3" width="14" height="14" rx="2" /><path d="M6.5 13.5 9 11l2 1.8 2.8-3.3" /></svg>;
  if (name === "select") return <svg {...common}><path d="M5 3.5 15.5 11l-5 .8-2.6 4.4Z" /><path d="m11 12 3.3 4" /></svg>;
  if (name === "draw") return <svg {...common}><path d="m4 14.8 2.8-.6 8.1-8.1a1.6 1.6 0 0 0-2.3-2.3l-8.1 8.1-.5 2.9Z" /><path d="m11.5 4.9 2.3 2.3" /></svg>;
  if (name === "rectangle") return <svg {...common}><rect x="3.5" y="4.5" width="13" height="11" rx="1.5" /></svg>;
  if (name === "text") return <svg {...common}><path d="M4 5h12M10 5v10M7.5 15h5" /></svg>;
  if (name === "capture") return <svg {...common}><path d="M6 3.5H3.5V6M14 3.5h2.5V6M16.5 14v2.5H14M6 16.5H3.5V14" /><rect x="6.2" y="7" width="7.6" height="6" rx="1.2" /><circle cx="10" cy="10" r="1.4" /></svg>;
  if (name === "upload") return <svg {...common}><path d="M10 13V3.8M6.7 7.1 10 3.8l3.3 3.3" /><path d="M4 12.5v3h12v-3" /></svg>;
  if (name === "undo") return <svg {...common}><path d="M7.2 6H3.5v-3" /><path d="M4 6a7 7 0 1 1-.2 7.7" /></svg>;
  if (name === "redo") return <svg {...common}><path d="M12.8 6h3.7v-3" /><path d="M16 6a7 7 0 1 0 .2 7.7" /></svg>;
  if (name === "trash") return <svg {...common}><path d="M4.5 6h11M8 3.5h4M6 6l.7 10h6.6L14 6M8.3 8.5v5M11.7 8.5v5" /></svg>;
  if (name === "check") return <svg {...common}><path d="m4 10.3 3.7 3.7L16 5.8" /></svg>;
  if (name === "image") return <svg {...common}><rect x="3" y="4" width="14" height="12" rx="2" /><circle cx="7.3" cy="8" r="1.2" /><path d="m5 14 3.6-3.5 2.4 2 1.7-1.7L15 14" /></svg>;
  return <svg {...common}><path d="m5 5 10 10M15 5 5 15" /></svg>;
}
