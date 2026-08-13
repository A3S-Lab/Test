import type {
  PageContextBridge,
  QualityFinding,
  QualityReportRecord,
  RepairIntent,
  RepairSeverity,
  RepairTarget,
} from "./types";

export type QualitySelection = {
  reportId: string;
  finding: QualityFinding;
};

export type QualityCandidate = {
  selection: QualitySelection;
  target: RepairTarget;
  label: string;
  instruction: string;
  successCriteria: string;
  intent: RepairIntent;
  severity: RepairSeverity;
};

export type QualityCandidatesProps = {
  reports: QualityReportRecord[];
  onReview(reportId: string, finding: QualityFinding): void;
  onDismiss(reportId: string, findingId: string): void;
};

export function QualityCandidates({ reports, onReview, onDismiss }: QualityCandidatesProps) {
  if (reports.length === 0) return null;
  return <section className="a3s-quality" aria-label="Contract findings">
    <div className="a3s-section-heading">
      <strong>Contract findings</strong>
      <small>Review before sending</small>
    </div>
    {reports.flatMap((report) => report.findings.map((finding) => (
      <article key={finding.id} className="a3s-quality-item">
        <span className={`a3s-status status-${finding.severity}`}>{finding.severity}</span>
        <strong>{finding.message}</strong>
        <small>
          {report.contract} · {finding.rule_id}
          {finding.observed_node_id ? " · target found" : " · choose target"}
        </small>
        <div>
          <button
            type="button"
            aria-label={`Review contract finding: ${finding.message}`}
            onClick={() => onReview(report.id, finding)}
          >
            {finding.observed_node_id ? "Review finding" : "Choose target"}
          </button>
          <button
            type="button"
            className="quiet"
            aria-label={`Dismiss contract finding: ${finding.message}`}
            onClick={() => onDismiss(report.id, finding.id)}
          >
            Dismiss
          </button>
        </div>
      </article>
    )))}
  </section>;
}

export function qualitySuccessCriteria(finding: QualityFinding): string {
  const expected = JSON.stringify(finding.expected);
  const summary = expected.length > 240 ? `${expected.slice(0, 237)}...` : expected;
  return `Contract ${finding.rule_id} matches the expected value ${summary}`;
}

export function resolveQualityCandidate(
  bridge: PageContextBridge,
  selection: QualitySelection,
  selectedNodeId?: string,
): QualityCandidate | null {
  const nodeId = selectedNodeId ?? selection.finding.observed_node_id;
  if (!nodeId || !bridge.resolve(nodeId)) return null;
  return {
    selection,
    target: { kind: "node", nodeIds: [nodeId] },
    label: selection.finding.element_id ?? selection.finding.rule_id,
    instruction: selection.finding.message,
    successCriteria: qualitySuccessCriteria(selection.finding),
    intent: "fix",
    severity: selection.finding.severity,
  };
}
