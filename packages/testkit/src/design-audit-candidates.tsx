import type {
  DesignAuditFinding,
  DesignAuditReportRecord,
  PageContextBridge,
  RepairIntent,
  RepairSeverity,
  RepairTarget,
} from "./types";

export type DesignAuditSelection = {
  reportId: string;
  finding: DesignAuditFinding;
};

export type DesignAuditCandidate = {
  selection: DesignAuditSelection;
  target: RepairTarget;
  label: string;
  instruction: string;
  successCriteria: string;
  intent: RepairIntent;
  severity: RepairSeverity;
};

export type DesignAuditCandidatesProps = {
  reports: DesignAuditReportRecord[];
  onReview(reportId: string, finding: DesignAuditFinding): void;
  onDismiss(reportId: string, findingId: string): void;
};

export function DesignAuditCandidates({ reports, onReview, onDismiss }: DesignAuditCandidatesProps) {
  if (reports.length === 0) return null;
  return <section className="a3s-quality a3s-design-audit" aria-label="Design audit suggestions">
    <div className="a3s-section-heading">
      <strong>Design audit</strong>
      <small>Advisory · human review required</small>
    </div>
    {reports.flatMap((report) => report.findings.map((finding) => (
      <article key={`${report.id}:${finding.id}`} className="a3s-quality-item">
        <span className={`a3s-status status-${prioritySeverity(finding.priority)}`}>{finding.priority}</span>
        <strong>{finding.summary}</strong>
        <p>{finding.rationale}</p>
        <small>
          {dimensionLabel(finding.dimension)} · {finding.confidence}% confidence · {targetLabel(finding)}
        </small>
        <div>
          <button
            type="button"
            aria-label={`Review design suggestion: ${finding.summary}`}
            onClick={() => onReview(report.id, finding)}
          >
            {finding.target.kind === "node" ? "Review suggestion" : "Review region"}
          </button>
          <button
            type="button"
            className="quiet"
            aria-label={`Dismiss design suggestion: ${finding.summary}`}
            onClick={() => onDismiss(report.id, finding.id)}
          >
            Dismiss
          </button>
        </div>
      </article>
    )))}
  </section>;
}

export function resolveDesignAuditCandidate(
  bridge: PageContextBridge,
  selection: DesignAuditSelection,
  selectedNodeId?: string,
): DesignAuditCandidate | null {
  const target = designAuditRepairTarget(bridge, selection.finding, selectedNodeId);
  if (!target) return null;
  return {
    selection,
    target,
    label: dimensionLabel(selection.finding.dimension),
    instruction: selection.finding.recommendation,
    successCriteria: `The reviewed design concern “${selection.finding.summary}” is addressed while preserving semantics, accessibility, and behavior`,
    intent: "change",
    severity: prioritySeverity(selection.finding.priority),
  };
}

export function designAuditRepairTarget(
  bridge: PageContextBridge,
  finding: DesignAuditFinding,
  selectedNodeId?: string,
): RepairTarget | null {
  const target = selectedNodeId
    ? { kind: "node" as const, node_id: selectedNodeId }
    : finding.target;
  if (target.kind === "node") {
    if (!bridge.resolve(target.node_id)) return null;
    return { kind: "node", nodeIds: [target.node_id] };
  }
  const snapshot = bridge.snapshot({ detail: "summary", limits: { nodes: 1 } });
  const visual = snapshot.page.viewport.visual;
  const viewport = visual
    ? { x: visual.x, y: visual.y, width: visual.width, height: visual.height }
    : { x: 0, y: 0, width: snapshot.page.viewport.width, height: snapshot.page.viewport.height };
  if (target.kind === "page") {
    return { kind: "region", nodeIds: [], region: viewport };
  }
  return {
    kind: "region",
    nodeIds: [],
    region: {
      x: viewport.x + target.region.x * viewport.width,
      y: viewport.y + target.region.y * viewport.height,
      width: target.region.width * viewport.width,
      height: target.region.height * viewport.height,
    },
  };
}

function prioritySeverity(priority: DesignAuditFinding["priority"]): RepairSeverity {
  return priority === "high" ? "important" : "suggestion";
}

function dimensionLabel(dimension: DesignAuditFinding["dimension"]): string {
  return dimension.replaceAll("_", " ");
}

function targetLabel(finding: DesignAuditFinding): string {
  if (finding.target.kind === "node") return "current element";
  if (finding.target.kind === "region") return "normalized region";
  return "whole page";
}
