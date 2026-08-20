import {
  decodedMappings,
  originalPositionFor,
  TraceMap,
  type EncodedSourceMap as TraceEncodedSourceMap,
} from "@jridgewell/trace-mapping";
import { boundaryElements, composedDistance } from "./boundary";
import { safeCallback } from "./sanitize";
import {
  SOURCE_MAPPING_PROTOCOL,
  type BoundaryRegistration,
  type EncodedSourceMapV3,
  type SourceMapRegistration,
  type SourceMapping,
  type SourceMappingCandidate,
  type SourceRegistration,
  type SourceSpan,
} from "./types";

const MAX_CANDIDATES = 8;
const MAX_FILE_BYTES = 2_048;
const MAX_FRAMEWORK_BYTES = 64;
const MAX_ID_BYTES = 128;
const MAX_MAPS = 32;
const MAX_MAPPINGS_BYTES = 1_000_000;
const MAX_MAP_NAMES = 10_000;
const MAX_MAP_SOURCES = 10_000;
const MAX_POSITION = 10_000_000;
const MAX_REGISTRATIONS = 512;

type StoredSource = Omit<SourceRegistration, "source" | "generated"> & {
  source?: SourceSpan;
  generated?: SourceSpan;
};

type StoredMap = {
  id: string;
  key: string;
  trace: TraceMap;
};

type Ownership = {
  id: string;
  componentId?: string;
  framework?: string;
  source?: SourceSpan;
  generated?: SourceSpan;
  distance: number;
  boundary: boolean;
};

export class SourceMappingStore {
  readonly #sources = new Map<string, StoredSource>();
  readonly #mapsById = new Map<string, StoredMap>();
  readonly #mapsByFile = new Map<string, StoredMap>();

  registerSource(registration: SourceRegistration): () => void {
    const normalized = normalizeSourceRegistration(registration);
    if (this.#sources.size >= MAX_REGISTRATIONS)
      throw new Error(
        `source registrations are bounded to ${MAX_REGISTRATIONS}`,
      );
    if (this.#sources.has(normalized.id))
      throw new Error(`source registration '${normalized.id}' already exists`);
    this.#sources.set(normalized.id, normalized);
    return () => {
      if (this.#sources.get(normalized.id) === normalized)
        this.#sources.delete(normalized.id);
    };
  }

  registerSourceMap(registration: SourceMapRegistration): () => void {
    const normalized = normalizeSourceMapRegistration(registration);
    if (this.#mapsById.size >= MAX_MAPS)
      throw new Error(`source maps are bounded to ${MAX_MAPS}`);
    if (this.#mapsById.has(normalized.id))
      throw new Error(`source map '${normalized.id}' already exists`);
    if (this.#mapsByFile.has(normalized.key))
      throw new Error(
        `a source map is already registered for '${registration.generatedFile}'`,
      );
    this.#mapsById.set(normalized.id, normalized);
    this.#mapsByFile.set(normalized.key, normalized);
    return () => {
      if (this.#mapsById.get(normalized.id) !== normalized) return;
      this.#mapsById.delete(normalized.id);
      this.#mapsByFile.delete(normalized.key);
    };
  }

  mappingFor(
    element: Element,
    boundaries: readonly BoundaryRegistration[],
  ): SourceMapping | undefined {
    const owners = [
      ...this.#boundaryOwners(element, boundaries),
      ...this.#sourceOwners(element),
    ];
    const candidates = owners.flatMap((owner) => this.#candidates(owner));
    const ranked = rankCandidates(candidates);
    if (ranked.length === 0) return undefined;
    return {
      protocol: SOURCE_MAPPING_PROTOCOL,
      candidates: ranked.slice(0, MAX_CANDIDATES),
      truncated: ranked.length > MAX_CANDIDATES,
    };
  }

  clear(): void {
    this.#sources.clear();
    this.#mapsById.clear();
    this.#mapsByFile.clear();
  }

  #boundaryOwners(
    element: Element,
    boundaries: readonly BoundaryRegistration[],
  ): Ownership[] {
    return boundaries.flatMap((boundary) => {
      if (
        (!boundary.source && !boundary.generated) ||
        !boundedString(boundary.id, MAX_ID_BYTES)
      )
        return [];
      const distances = boundaryElements(boundary)
        .map((root) => composedDistance(root, element))
        .filter((distance): distance is number => distance !== null);
      if (distances.length === 0) return [];
      return [
        {
          id: boundary.id,
          componentId: boundary.id,
          ...(boundary.source
            ? {
                source: normalizeSourceSpan(boundary.source, "boundary source"),
              }
            : {}),
          ...(boundary.generated
            ? {
                generated: normalizeSourceSpan(
                  boundary.generated,
                  "boundary generated source",
                  true,
                ),
              }
            : {}),
          distance: Math.min(...distances),
          boundary: true,
        },
      ];
    });
  }

  #sourceOwners(element: Element): Ownership[] {
    const owners: Ownership[] = [];
    for (const registration of this.#sources.values()) {
      const distances = sourceElements(registration)
        .map((root) => composedDistance(root, element))
        .filter(
          (distance): distance is number =>
            distance !== null &&
            (registration.includeDescendants !== false || distance === 0),
        );
      if (distances.length === 0) continue;
      owners.push({
        id: registration.id,
        framework: registration.framework,
        ...(registration.source ? { source: registration.source } : {}),
        ...(registration.generated
          ? { generated: registration.generated }
          : {}),
        distance: Math.min(...distances),
        boundary: false,
      });
    }
    return owners;
  }

  #candidates(owner: Ownership): SourceMappingCandidate[] {
    const relation = owner.distance === 0 ? "exact" : "ancestor";
    const candidates: SourceMappingCandidate[] = [];
    if (owner.source) {
      candidates.push({
        span: owner.source,
        ...(owner.generated ? { generatedSpan: owner.generated } : {}),
        confidence: confidence(
          owner.boundary ? 0.9 : 0.99,
          owner.distance,
          owner.boundary ? 0.72 : 0.84,
        ),
        origin: owner.boundary ? "boundary_hint" : "framework_adapter",
        relation,
        registrationId: owner.id,
        ...(owner.componentId ? { componentId: owner.componentId } : {}),
        ...(owner.framework ? { framework: owner.framework } : {}),
      });
    }
    if (!owner.generated) return candidates;

    const traced = this.#trace(owner.generated);
    candidates.push({
      span: traced ?? owner.generated,
      ...(traced ? { generatedSpan: owner.generated } : {}),
      confidence: confidence(
        traced ? (owner.boundary ? 0.94 : 0.97) : 0.68,
        owner.distance,
        traced ? 0.8 : 0.48,
      ),
      origin: traced ? "source_map" : "generated",
      relation,
      registrationId: owner.id,
      ...(owner.componentId ? { componentId: owner.componentId } : {}),
      ...(owner.framework ? { framework: owner.framework } : {}),
    });
    return candidates;
  }

  #trace(generated: SourceSpan): SourceSpan | null {
    if (generated.line === undefined) return null;
    const registered = this.#mapsByFile.get(fileKey(generated.file));
    if (!registered) return null;
    const start = originalPositionFor(registered.trace, {
      line: generated.line,
      column: Math.max(0, (generated.column ?? 1) - 1),
    });
    if (
      start.source === null ||
      start.line === null ||
      start.column === null ||
      start.line < 1 ||
      start.line > MAX_POSITION ||
      start.column < 0 ||
      start.column >= MAX_POSITION ||
      !boundedString(start.source, MAX_FILE_BYTES)
    )
      return null;
    const span: SourceSpan = {
      file: start.source,
      line: start.line,
      column: start.column + 1,
    };
    if (generated.endLine === undefined) return span;
    const end = originalPositionFor(registered.trace, {
      line: generated.endLine,
      column: Math.max(0, (generated.endColumn ?? 1) - 1),
    });
    const endColumn = end.column === null ? null : end.column + 1;
    if (
      end.source === start.source &&
      end.line !== null &&
      endColumn !== null &&
      end.line >= 1 &&
      end.line <= MAX_POSITION &&
      endColumn >= 1 &&
      endColumn <= MAX_POSITION &&
      (end.line > start.line ||
        (end.line === start.line && endColumn >= start.column + 1))
    ) {
      span.endLine = end.line;
      span.endColumn = endColumn;
    }
    return span;
  }
}

export function normalizeSourceSpan(
  value: SourceSpan,
  label: string,
  requirePosition = false,
): SourceSpan {
  if (!boundedString(value.file, MAX_FILE_BYTES))
    throw new Error(`${label} file must be a bounded non-empty string`);
  for (const [field, position] of [
    ["line", value.line],
    ["column", value.column],
    ["endLine", value.endLine],
    ["endColumn", value.endColumn],
  ] as const) {
    if (
      position !== undefined &&
      (!Number.isSafeInteger(position) ||
        position < 1 ||
        position > MAX_POSITION)
    )
      throw new Error(`${label} ${field} must be a bounded positive integer`);
  }
  if (requirePosition && value.line === undefined)
    throw new Error(`${label} requires a generated line`);
  if (value.column !== undefined && value.line === undefined)
    throw new Error(`${label} column requires a line`);
  if (value.endLine !== undefined && value.line === undefined)
    throw new Error(`${label} endLine requires a line`);
  if (value.endColumn !== undefined && value.endLine === undefined)
    throw new Error(`${label} endColumn requires endLine`);
  if (
    value.line !== undefined &&
    value.endLine !== undefined &&
    (value.endLine < value.line ||
      (value.endLine === value.line &&
        (value.endColumn ?? 1) < (value.column ?? 1)))
  )
    throw new Error(`${label} end must not precede its start`);
  return {
    file: value.file,
    ...(value.line !== undefined ? { line: value.line } : {}),
    ...(value.column !== undefined ? { column: value.column } : {}),
    ...(value.endLine !== undefined ? { endLine: value.endLine } : {}),
    ...(value.endColumn !== undefined ? { endColumn: value.endColumn } : {}),
  };
}

function normalizeSourceRegistration(
  registration: SourceRegistration,
): StoredSource {
  if (!boundedIdentifier(registration.id, MAX_ID_BYTES))
    throw new Error("source registration id must be a bounded identifier");
  if (!boundedIdentifier(registration.framework, MAX_FRAMEWORK_BYTES))
    throw new Error(
      "source registration framework must be a bounded identifier",
    );
  if (!registration.source && !registration.generated)
    throw new Error(
      "source registration requires source or generated metadata",
    );
  if (sourceElements(registration).length === 0)
    throw new Error("source registration must contain at least one element");
  return {
    ...registration,
    ...(registration.source
      ? { source: normalizeSourceSpan(registration.source, "source") }
      : {}),
    ...(registration.generated
      ? {
          generated: normalizeSourceSpan(
            registration.generated,
            "generated source",
            true,
          ),
        }
      : {}),
  };
}

function normalizeSourceMapRegistration(
  registration: SourceMapRegistration,
): StoredMap {
  if (!boundedIdentifier(registration.id, MAX_ID_BYTES))
    throw new Error("source map id must be a bounded identifier");
  if (!boundedString(registration.generatedFile, MAX_FILE_BYTES))
    throw new Error("source map generatedFile must be bounded and non-empty");
  if (
    registration.mapUrl !== undefined &&
    !boundedString(registration.mapUrl, MAX_FILE_BYTES)
  )
    throw new Error("source map mapUrl must be bounded and non-empty");
  const map = sanitizeMap(registration.map);
  try {
    const trace = new TraceMap(
      map,
      registration.mapUrl ?? registration.generatedFile,
    );
    decodedMappings(trace);
    return {
      id: registration.id,
      key: fileKey(registration.generatedFile),
      trace,
    };
  } catch {
    throw new Error("source map mappings are invalid");
  }
}

function sanitizeMap(map: EncodedSourceMapV3): TraceEncodedSourceMap {
  if (map.version !== 3) throw new Error("source map version must be 3");
  if (!Array.isArray(map.sources) || map.sources.length > MAX_MAP_SOURCES)
    throw new Error(`source map sources are bounded to ${MAX_MAP_SOURCES}`);
  if (map.sources.some((source) => !boundedString(source, MAX_FILE_BYTES)))
    throw new Error("source map sources must be bounded non-empty strings");
  const names = map.names ?? [];
  if (!Array.isArray(names) || names.length > MAX_MAP_NAMES)
    throw new Error(`source map names are bounded to ${MAX_MAP_NAMES}`);
  if (names.some((name) => !boundedString(name, MAX_FILE_BYTES)))
    throw new Error("source map names must be bounded strings");
  if (
    typeof map.mappings !== "string" ||
    map.mappings.length > MAX_MAPPINGS_BYTES
  )
    throw new Error(
      `source map mappings are bounded to ${MAX_MAPPINGS_BYTES} bytes`,
    );
  if (!/^[A-Za-z0-9+/;,]*$/.test(map.mappings))
    throw new Error("source map mappings contain invalid VLQ characters");
  if (map.file !== undefined && !boundedString(map.file, MAX_FILE_BYTES))
    throw new Error("source map file must be bounded and non-empty");
  if (
    map.sourceRoot !== undefined &&
    !boundedString(map.sourceRoot, MAX_FILE_BYTES)
  )
    throw new Error("source map sourceRoot must be bounded and non-empty");
  return {
    version: 3,
    ...(map.file ? { file: map.file } : {}),
    ...(map.sourceRoot ? { sourceRoot: map.sourceRoot } : {}),
    names: [...names],
    sources: [...map.sources],
    mappings: map.mappings,
  };
}

function sourceElements(registration: {
  elements: () => readonly Element[];
}): Element[] {
  const elements = safeCallback(
    registration.elements,
    [] as readonly Element[],
  );
  return Array.from(
    new Set(
      elements.filter(
        (element): element is Element =>
          element instanceof Element && element.isConnected,
      ),
    ),
  );
}

function rankCandidates(
  candidates: SourceMappingCandidate[],
): SourceMappingCandidate[] {
  const unique = new Map<string, SourceMappingCandidate>();
  for (const candidate of candidates) {
    const key = spanKey(candidate.span);
    const current = unique.get(key);
    if (!current || candidate.confidence > current.confidence)
      unique.set(key, candidate);
  }
  return [...unique.values()].sort(
    (left, right) =>
      right.confidence - left.confidence ||
      left.span.file.localeCompare(right.span.file) ||
      (left.span.line ?? 0) - (right.span.line ?? 0) ||
      (left.span.column ?? 0) - (right.span.column ?? 0) ||
      left.registrationId.localeCompare(right.registrationId),
  );
}

function spanKey(span: SourceSpan): string {
  return [
    span.file,
    span.line ?? "",
    span.column ?? "",
    span.endLine ?? "",
    span.endColumn ?? "",
  ].join("\u0000");
}

function confidence(base: number, distance: number, minimum: number): number {
  return Math.max(minimum, Math.round((base - distance * 0.01) * 100) / 100);
}

function fileKey(file: string): string {
  try {
    const url = new URL(file, document.baseURI);
    url.hash = "";
    url.search = "";
    return url.href;
  } catch {
    return file.replace(/[?#].*$/, "");
  }
}

function boundedString(value: unknown, maxBytes: number): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    new TextEncoder().encode(value).byteLength <= maxBytes &&
    !/[\u0000-\u001f\u007f]/.test(value)
  );
}

function boundedIdentifier(value: unknown, maxBytes: number): value is string {
  return (
    boundedString(value, maxBytes) &&
    /^[A-Za-z0-9][A-Za-z0-9._:/-]*$/.test(value)
  );
}
