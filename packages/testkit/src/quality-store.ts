import {
  QUALITY_REPORT_PROTOCOL,
  type JsonValue,
  type QualityFinding,
  type QualityReport,
  type QualityReportRecord,
} from "./types";

const MAX_MATCHES = 5_000;
const MAX_FINDINGS = 500;
const MAX_ENCODED_BYTES = 1_048_576;

export class QualityStore {
  readonly #maxReports: number;
  readonly #reports = new Map<string, QualityReportRecord>();

  constructor(maxReports: number) {
    this.#maxReports = maxReports;
  }

  report(report: QualityReport): QualityReportRecord | null {
    if (!validQualityReport(report)) return null;
    const record: QualityReportRecord = {
      ...structuredClone(report),
      id: qualityReportId(report),
      protocol: QUALITY_REPORT_PROTOCOL,
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

  list(): QualityReportRecord[] {
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

  clear(): void {
    this.#reports.clear();
  }
}

function sameScope(left: QualityReportRecord, right: QualityReport): boolean {
  return left.contract === right.contract && left.variant === right.variant && left.state === right.state;
}

function qualityReportId(report: QualityReport): string {
  const parts = [report.contract, report.variant, report.state, String(report.observation_revision ?? "unknown")]
    .map((value) => encodeURIComponent(value));
  return `quality:${parts.join(":")}`;
}

function validQualityReport(report: QualityReport): boolean {
  if (!report || typeof report !== "object") return false;
  if (!boundedText(report.contract, 128, true)) return false;
  if (!boundedText(report.variant, 128, true)) return false;
  if (!boundedText(report.state, 128, true)) return false;
  if (!(["passed", "failed", "inconclusive"] as const).includes(report.outcome)) return false;
  if (report.observation_revision !== undefined && report.observation_revision !== null && (!Number.isSafeInteger(report.observation_revision) || report.observation_revision < 0)) return false;
  if (!Array.isArray(report.matches) || report.matches.length > MAX_MATCHES) return false;
  if (!Array.isArray(report.findings) || report.findings.length > MAX_FINDINGS) return false;
  if (!report.matches.every(validMatch) || !report.findings.every(validFinding)) return false;
  const findingIds = new Set(report.findings.map((finding) => finding.id));
  if (findingIds.size !== report.findings.length) return false;
  try {
    return new TextEncoder().encode(JSON.stringify(report)).byteLength <= MAX_ENCODED_BYTES;
  } catch {
    return false;
  }
}

function validMatch(match: QualityReport["matches"][number]): boolean {
  return Boolean(
    match &&
    boundedText(match.element_id, 128, true) &&
    boundedText(match.node_id, 128, true) &&
    (["test_id", "component", "role_and_name", "role"] as const).includes(match.strategy),
  );
}

function validFinding(finding: QualityFinding): boolean {
  return Boolean(
    finding &&
    boundedText(finding.id, 128, true) &&
    boundedText(finding.dimension, 128, true) &&
    boundedText(finding.rule_id, 256, true) &&
    (["blocking", "important", "suggestion"] as const).includes(finding.severity) &&
    boundedText(finding.message, 8_192, true) &&
    validJson(finding.expected) &&
    validJson(finding.actual) &&
    (finding.element_id === undefined || boundedText(finding.element_id, 128, true)) &&
    (finding.observed_node_id === undefined || boundedText(finding.observed_node_id, 128, true)) &&
    Number.isInteger(finding.confidence) && finding.confidence >= 0 && finding.confidence <= 100
  );
}

function validJson(value: JsonValue, depth = 0): boolean {
  if (depth > 32) return false;
  if (value === null || typeof value === "string" || typeof value === "boolean") return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.length <= 5_000 && value.every((item) => validJson(item, depth + 1));
  if (typeof value !== "object") return false;
  const entries = Object.entries(value);
  return entries.length <= 5_000 && entries.every(([key, item]) => boundedText(key, 256, false) && validJson(item, depth + 1));
}

function boundedText(value: unknown, maximum: number, required: boolean): value is string {
  return typeof value === "string" && value.length <= maximum && (!required || value.trim().length > 0);
}
