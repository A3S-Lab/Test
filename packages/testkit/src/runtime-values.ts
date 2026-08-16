import type { ContextLimits, JsonValue } from "./types";

export const DEFAULT_CONTEXT_LIMITS: ContextLimits = {
  nodes: 500,
  stringBytes: 4_096,
  encodedBytes: 1_048_576,
  uiNodes: 200,
  uiStateSamples: 200,
  uiDurationMs: 32,
  uiEncodedBytes: 262_144,
};

export const MAX_CONTEXT_LIMITS: ContextLimits = {
  nodes: 5_000,
  stringBytes: 16_384,
  encodedBytes: 8_388_608,
  uiNodes: 1_000,
  uiStateSamples: 1_000,
  uiDurationMs: 100,
  uiEncodedBytes: 1_048_576,
};

export function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, Math.trunc(value)));
}

export function pageTheme(): "light" | "dark" | "unknown" {
  const declared =
    document.documentElement.dataset.theme ??
    document.documentElement.style.colorScheme;
  if (/dark/i.test(declared)) return "dark";
  if (/light/i.test(declared)) return "light";
  if (typeof matchMedia === "function")
    return matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  return "unknown";
}

export function isJsonObject(
  value: JsonValue,
): value is Record<string, JsonValue> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}
