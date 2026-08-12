import type { JsonValue } from "./types";

const SENSITIVE_KEY = /(authorization|cookie|password|passwd|secret|token|api[-_]?key|session)/i;

export function truncateUtf8(value: string, maxBytes: number): string {
  if (new TextEncoder().encode(value).byteLength <= maxBytes) return value;
  let low = 0;
  let high = value.length;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (new TextEncoder().encode(value.slice(0, middle)).byteLength <= maxBytes) {
      low = middle;
    } else {
      high = middle - 1;
    }
  }
  return value.slice(0, low);
}

export function sanitizeFacts(
  value: unknown,
  maxStringBytes: number,
  depth = 0,
  seen = new WeakSet<object>(),
): JsonValue {
  if (value === null || typeof value === "boolean") return value;
  if (typeof value === "string") return truncateUtf8(value, maxStringBytes);
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (depth >= 5 || typeof value !== "object") return null;
  if (seen.has(value)) return null;
  seen.add(value);

  if (Array.isArray(value)) {
    return value.slice(0, 100).map((item) => sanitizeFacts(item, maxStringBytes, depth + 1, seen));
  }

  const result: Record<string, JsonValue> = {};
  for (const [key, item] of Object.entries(value as Record<string, unknown>).slice(0, 100)) {
    if (SENSITIVE_KEY.test(key)) {
      result[key] = "[redacted]";
    } else {
      result[truncateUtf8(key, 128)] = sanitizeFacts(item, maxStringBytes, depth + 1, seen);
    }
  }
  return result;
}

export function safeCallback<T>(callback: (() => T) | undefined, fallback: T): T {
  if (!callback) return fallback;
  try {
    return callback();
  } catch {
    return fallback;
  }
}
