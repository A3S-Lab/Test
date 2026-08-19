import type { RepairDesignReference } from "./types";

export const DESIGN_BOARD_WIDTH = 960;
export const DESIGN_BOARD_HEIGHT = 600;
export const MAX_DESIGN_REFERENCE_DATA_URL_BYTES = 384 * 1_024;
export const MAX_DESIGN_REFERENCE_SOURCE_BYTES = 8 * 1_024 * 1_024;

const MEDIA_TYPES = ["image/png", "image/jpeg"] as const;

export function validDesignReference(value: unknown): value is RepairDesignReference {
  if (!isObject(value) || !onlyKeys(value, ["kind", "width", "height", "image"])) return false;
  if (!(["sketch", "screenshot"] as const).includes(value.kind as "sketch" | "screenshot")) return false;
  if (!boundedDimension(value.width, 1_600) || !boundedDimension(value.height, 1_200)) return false;
  if (value.width * value.height > 1_920_000 || !isObject(value.image)) return false;

  if (value.image.kind === "inline") {
    if (!onlyKeys(value.image, ["kind", "mediaType", "dataUrl"])) return false;
    if (!isMediaType(value.image.mediaType) || typeof value.image.dataUrl !== "string") return false;
    if (value.image.dataUrl.length === 0 || value.image.dataUrl.length > MAX_DESIGN_REFERENCE_DATA_URL_BYTES) return false;
    const prefix = `data:${value.image.mediaType};base64,`;
    if (!value.image.dataUrl.startsWith(prefix)) return false;
    return /^[A-Za-z0-9+/]+={0,2}$/.test(value.image.dataUrl.slice(prefix.length));
  }

  if (value.image.kind !== "artifact" || !onlyKeys(value.image, ["kind", "evidence", "sha256"])) return false;
  if (typeof value.image.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(value.image.sha256)) return false;
  const evidence = value.image.evidence;
  return isObject(evidence)
    && onlyKeys(evidence, ["name", "path", "media_type"])
    && boundedString(evidence.name, 1, 256)
    && boundedString(evidence.path, 1, 4_096)
    && isMediaType(evidence.media_type);
}

function boundedDimension(value: unknown, maximum: number): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0 && value <= maximum;
}

function boundedString(value: unknown, minimum: number, maximum: number): value is string {
  return typeof value === "string" && value.length >= minimum && value.length <= maximum;
}

function isMediaType(value: unknown): value is (typeof MEDIA_TYPES)[number] {
  return (MEDIA_TYPES as readonly unknown[]).includes(value);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function onlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  const keys = new Set(allowed);
  return Object.keys(value).every((key) => keys.has(key));
}
