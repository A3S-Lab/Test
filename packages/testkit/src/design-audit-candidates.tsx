import type {
  DesignAuditFinding,
  DesignAuditReportRecord,
  PageContextBridge,
  RepairIntent,
  RepairSeverity,
  RepairTarget,
} from "./types";
import { englishReviewTranslator, useReviewI18n, type A3SReviewMessageKey, type ReviewTranslator } from "./review-locale";

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
  const { t } = useReviewI18n();
  if (reports.length === 0) return null;
  return <section className="a3s-quality a3s-design-audit" aria-label={t("designAuditSuggestions")}>
    <div className="a3s-section-heading">
      <strong>{t("designAudit")}</strong>
      <small>{t("advisoryHumanReview")}</small>
    </div>
    {reports.flatMap((report) => report.findings.map((finding) => (
      <article key={`${report.id}:${finding.id}`} className="a3s-quality-item">
        <span className={`a3s-status status-badge status-${prioritySeverity(finding.priority)}`}>{t(finding.priority === "high" ? "priorityHigh" : finding.priority === "medium" ? "priorityMedium" : "priorityLow")}</span>
        <strong>{finding.summary}</strong>
        <p>{finding.rationale}</p>
        <small>
          {dimensionLabel(finding.dimension, t)} · {t("confidence", { confidence: finding.confidence })} · {targetLabel(finding, t)}
        </small>
        <div>
          <button
            type="button"
            aria-label={t("reviewDesignSuggestion", { message: finding.summary })}
            onClick={() => onReview(report.id, finding)}
          >
            {t(finding.target.kind === "node" ? "reviewSuggestion" : "reviewRegion")}
          </button>
          <button
            type="button"
            className="quiet"
            aria-label={t("dismissDesignSuggestion", { message: finding.summary })}
            onClick={() => onDismiss(report.id, finding.id)}
          >
            {t("dismiss")}
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
  t: ReviewTranslator = englishReviewTranslator,
): DesignAuditCandidate | null {
  const target = designAuditRepairTarget(bridge, selection.finding, selectedNodeId);
  if (!target) return null;
  return {
    selection,
    target,
    label: dimensionLabel(selection.finding.dimension, t),
    instruction: selection.finding.recommendation,
    successCriteria: t("designCriteria", { summary: selection.finding.summary }),
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

const DIMENSION_KEYS: Record<DesignAuditFinding["dimension"], A3SReviewMessageKey> = {
  visual_hierarchy: "dimensionVisualHierarchy",
  layout_composition: "dimensionLayoutComposition",
  spacing_rhythm: "dimensionSpacingRhythm",
  typography: "dimensionTypography",
  color_use: "dimensionColorUse",
  consistency: "dimensionConsistency",
  interaction_clarity: "dimensionInteractionClarity",
  content_clarity: "dimensionContentClarity",
  responsive_composition: "dimensionResponsiveComposition",
};

function dimensionLabel(dimension: DesignAuditFinding["dimension"], t: ReviewTranslator = englishReviewTranslator): string {
  return t(DIMENSION_KEYS[dimension]);
}

function targetLabel(finding: DesignAuditFinding, t: ReviewTranslator): string {
  if (finding.target.kind === "node") return t("targetCurrentElement");
  if (finding.target.kind === "region") return t("targetNormalizedRegion");
  return t("targetWholePage");
}
