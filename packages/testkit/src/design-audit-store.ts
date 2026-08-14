import {
  DESIGN_AUDIT_REPORT_PROTOCOL,
  type DesignAuditFinding,
  type DesignAuditReport,
  type DesignAuditReportRecord,
} from "./types";

const MAX_FINDINGS = 500;
const MAX_ENCODED_BYTES = 1_048_576;
const DIMENSIONS = new Set([
  "visual_hierarchy",
  "layout_composition",
  "spacing_rhythm",
  "typography",
  "color_use",
  "consistency",
  "interaction_clarity",
  "content_clarity",
  "responsive_composition",
]);

export class DesignAuditStore {
  readonly #maxReports: number;
  readonly #reports = new Map<string, DesignAuditReportRecord>();

  constructor(maxReports: number) {
    this.#maxReports = maxReports;
  }

  report(report: DesignAuditReport): DesignAuditReportRecord | null {
    if (!validDesignAuditReport(report)) return null;
    const record: DesignAuditReportRecord = {
      ...structuredClone(report),
      id: reportId(report),
      reportedAt: new Date().toISOString(),
    };
    for (const [id, existing] of this.#reports) {
      if (sameScope(existing, report)) this.#reports.delete(id);
    }
    if (record.findings.length > 0) {
      this.#reports.set(record.id, record);
      while (this.#reports.size > this.#maxReports) {
        const oldest = this.#reports.keys().next().value as string | undefined;
        if (!oldest) break;
        this.#reports.delete(oldest);
      }
    }
    return structuredClone(record);
  }

  list(): DesignAuditReportRecord[] {
    return Array.from(this.#reports.values()).map((report) => structuredClone(report));
  }

  dismissFinding(reportId: string, findingId: string): boolean {
    const report = this.#reports.get(reportId);
    if (!report) return false;
    const findings = report.findings.filter((finding) => finding.id !== findingId);
    if (findings.length === report.findings.length) return false;
    if (findings.length === 0) this.#reports.delete(reportId);
    else this.#reports.set(reportId, { ...report, findings });
    return true;
  }

  dismissReport(reportId: string): boolean {
    return this.#reports.delete(reportId);
  }

  clear(): string[] {
    const reportIds = Array.from(this.#reports.keys());
    this.#reports.clear();
    return reportIds;
  }
}

function sameScope(left: DesignAuditReportRecord, right: DesignAuditReport): boolean {
  return left.provenance.identity.provider === right.provenance.identity.provider
    && left.provenance.identity.model === right.provenance.identity.model
    && left.provenance.observation_id === right.provenance.observation_id;
}

function reportId(report: DesignAuditReport): string {
  const parts = [
    report.provenance.identity.provider,
    report.provenance.identity.model,
    String(report.provenance.observation_id),
    String(report.provenance.surface_revision),
    report.provenance.screenshot_sha256,
  ].map((value) => encodeURIComponent(value));
  return `design-audit:${parts.join(":")}`;
}

export function validDesignAuditReport(report: DesignAuditReport): boolean {
  if (!report || typeof report !== "object" || report.protocol !== DESIGN_AUDIT_REPORT_PROTOCOL) return false;
  if (!onlyKeys(report, ["protocol", "provenance", "dimensions", "findings"])) return false;
  const provenance = report.provenance;
  if (!provenance || provenance.authority !== "advisory") return false;
  if (!onlyKeys(provenance, ["identity", "observation_id", "surface_revision", "screenshot_sha256", "page_context_sha256", "width", "height", "usage", "request_id", "authority"])) return false;
  if (!onlyKeys(provenance.identity, ["provider", "model"])) return false;
  if (!boundedText(provenance.identity?.provider, 1_024) || !boundedText(provenance.identity?.model, 1_024)) return false;
  if (!positiveSafeInteger(provenance.observation_id) || !positiveSafeInteger(provenance.surface_revision)) return false;
  if (!validSha256(provenance.screenshot_sha256) || !validSha256(provenance.page_context_sha256)) return false;
  if (!positiveSafeInteger(provenance.width) || !positiveSafeInteger(provenance.height)) return false;
  if (provenance.width > 32_768 || provenance.height > 32_768) return false;
  if (!validUsage(provenance.usage)) return false;
  if (!onlyKeys(provenance.usage, ["input_units", "output_units", "cost_microusd"])) return false;
  if (provenance.request_id !== undefined && provenance.request_id !== null && !boundedText(provenance.request_id, 4_096)) return false;
  if (!Array.isArray(report.dimensions) || report.dimensions.length < 1 || report.dimensions.length > DIMENSIONS.size) return false;
  if (!report.dimensions.every((dimension) => DIMENSIONS.has(dimension))) return false;
  if (new Set(report.dimensions).size !== report.dimensions.length) return false;
  if (!Array.isArray(report.findings) || report.findings.length > MAX_FINDINGS) return false;
  if (!report.findings.every((finding) => validFinding(finding, new Set(report.dimensions)))) return false;
  if (new Set(report.findings.map((finding) => finding.id)).size !== report.findings.length) return false;
  try {
    return new TextEncoder().encode(JSON.stringify(report)).byteLength <= MAX_ENCODED_BYTES;
  } catch {
    return false;
  }
}

function validFinding(finding: DesignAuditFinding, dimensions: Set<string>): boolean {
  if (!finding || !boundedText(finding.id, 1_024)) return false;
  if (!onlyKeys(finding, ["id", "dimension", "priority", "summary", "rationale", "recommendation", "confidence", "target"])) return false;
  if (!dimensions.has(finding.dimension)) return false;
  if (!(new Set(["high", "medium", "low"])).has(finding.priority)) return false;
  if (!boundedText(finding.summary, 2_048)) return false;
  if (!boundedText(finding.rationale, 8_192)) return false;
  if (!boundedText(finding.recommendation, 8_192)) return false;
  if (!Number.isInteger(finding.confidence) || finding.confidence < 0 || finding.confidence > 100) return false;
  if (!finding.target || typeof finding.target !== "object") return false;
  if (finding.target.kind === "page") return onlyKeys(finding.target, ["kind"]);
  if (finding.target.kind === "node") {
    return onlyKeys(finding.target, ["kind", "node_id"])
      && boundedText(finding.target.node_id, 1_024);
  }
  if (finding.target.kind !== "region") return false;
  const region = finding.target.region;
  return onlyKeys(finding.target, ["kind", "region"])
    && onlyKeys(region, ["x", "y", "width", "height"])
    && [region?.x, region?.y, region?.width, region?.height].every(Number.isFinite)
    && region.x >= 0 && region.y >= 0 && region.width > 0 && region.height > 0
    && region.x + region.width <= 1 && region.y + region.height <= 1;
}

function validUsage(value: DesignAuditReport["provenance"]["usage"]): boolean {
  return Boolean(value)
    && [value.input_units, value.output_units, value.cost_microusd]
      .every((number) => Number.isSafeInteger(number) && number >= 0);
}

function positiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}

function validSha256(value: unknown): value is string {
  return typeof value === "string" && /^sha256:[0-9a-f]{64}$/.test(value);
}

function boundedText(value: unknown, maximum: number): value is string {
  return typeof value === "string" && value.trim().length > 0 && value.length <= maximum;
}

function onlyKeys(value: unknown, allowed: string[]): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const keys = Object.keys(value);
  return keys.every((key) => allowed.includes(key));
}
