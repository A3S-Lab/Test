import type { OverlayTheme } from "./review-model";

export const REVIEW_PREFERENCES_KEY = "a3s-test.review-preferences/1";
const REVIEW_TAB_HIDDEN_KEY = "a3s-test.review-hidden/1";
const MAX_ENCODED_BYTES = 2_048;

export type ReviewDock = "left" | "right";

export type ReviewPreferences = {
  theme: OverlayTheme;
  markerColor: string;
  clearOnCopy: boolean;
  blockInteractions: boolean;
  dock: ReviewDock;
  wireframeFade: number;
};

export const DEFAULT_REVIEW_PREFERENCES: ReviewPreferences = {
  theme: "system",
  markerColor: "#7157c9",
  clearOnCopy: false,
  blockInteractions: false,
  dock: "right",
  wireframeFade: 0.16,
};

export function loadReviewPreferences(
  storage?: Storage,
): ReviewPreferences {
  let target: Storage;
  let encoded: string | null;
  try {
    target = storage ?? window.localStorage;
    encoded = target.getItem(REVIEW_PREFERENCES_KEY);
  } catch {
    return { ...DEFAULT_REVIEW_PREFERENCES };
  }
  if (!encoded) return { ...DEFAULT_REVIEW_PREFERENCES };
  if (new TextEncoder().encode(encoded).byteLength > MAX_ENCODED_BYTES) {
    remove(target, REVIEW_PREFERENCES_KEY);
    return { ...DEFAULT_REVIEW_PREFERENCES };
  }
  try {
    const value = JSON.parse(encoded) as unknown;
    if (!validPreferences(value)) throw new Error("invalid preferences");
    return value;
  } catch {
    remove(target, REVIEW_PREFERENCES_KEY);
    return { ...DEFAULT_REVIEW_PREFERENCES };
  }
}

export function saveReviewPreferences(
  value: ReviewPreferences,
  storage?: Storage,
): void {
  if (!validPreferences(value)) return;
  try {
    (storage ?? window.localStorage).setItem(
      REVIEW_PREFERENCES_KEY,
      JSON.stringify(value),
    );
  } catch {
    // Disabled or exhausted storage must not break the host page.
  }
}

export function loadReviewTabHidden(
  storage?: Storage,
): boolean {
  try {
    return (storage ?? window.sessionStorage).getItem(REVIEW_TAB_HIDDEN_KEY) === "1";
  } catch {
    return false;
  }
}

export function saveReviewTabHidden(
  hidden: boolean,
  storage?: Storage,
): void {
  try {
    const target = storage ?? window.sessionStorage;
    if (hidden) target.setItem(REVIEW_TAB_HIDDEN_KEY, "1");
    else target.removeItem(REVIEW_TAB_HIDDEN_KEY);
  } catch {
    // Disabled storage must not break the host page.
  }
}

function validPreferences(value: unknown): value is ReviewPreferences {
  if (!isObject(value)) return false;
  const keys = Object.keys(value);
  if (keys.length !== 6 || keys.some((key) => !["theme", "markerColor", "clearOnCopy", "blockInteractions", "dock", "wireframeFade"].includes(key))) return false;
  return ["system", "light", "dark"].includes(String(value.theme))
    && typeof value.markerColor === "string"
    && /^#[0-9a-fA-F]{6}$/.test(value.markerColor)
    && typeof value.clearOnCopy === "boolean"
    && typeof value.blockInteractions === "boolean"
    && ["left", "right"].includes(String(value.dock))
    && typeof value.wireframeFade === "number"
    && Number.isFinite(value.wireframeFade)
    && value.wireframeFade >= 0
    && value.wireframeFade <= 0.8;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function remove(storage: Storage, key: string): void {
  try {
    storage.removeItem(key);
  } catch {
    // Disabled storage must not break the host page.
  }
}
