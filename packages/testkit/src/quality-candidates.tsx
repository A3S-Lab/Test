import type {
  PageContextBridge,
  QualityFinding,
  QualityReportRecord,
  RepairIntent,
  RepairSeverity,
  RepairTarget,
} from "./types";
import { englishReviewTranslator, useReviewI18n, type ReviewTranslator } from "./review-locale";

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
  const { t } = useReviewI18n();
  if (reports.length === 0) return null;
  return <section className="a3s-quality" aria-label={t("contractFindings")}>
    <div className="a3s-section-heading">
      <strong>{t("contractFindings")}</strong>
      <small>{t("reviewBeforeSending")}</small>
    </div>
    {reports.flatMap((report) => report.findings.map((finding) => (
      <article key={finding.id} className="a3s-quality-item">
        <span className={`a3s-status status-${finding.severity}`}>{t(finding.severity === "blocking" ? "severityBlocking" : finding.severity === "important" ? "severityImportant" : "severitySuggestion")}</span>
        <strong>{finding.message}</strong>
        <small>
          {report.contract} · {finding.rule_id}
          {` · ${t(finding.observed_node_id ? "targetFound" : "chooseTarget")}`}
        </small>
        <div>
          <button
            type="button"
            aria-label={t("reviewContractFinding", { message: finding.message })}
            onClick={() => onReview(report.id, finding)}
          >
            {t(finding.observed_node_id ? "reviewFinding" : "chooseTargetAction")}
          </button>
          <button
            type="button"
            className="quiet"
            aria-label={t("dismissContractFinding", { message: finding.message })}
            onClick={() => onDismiss(report.id, finding.id)}
          >
            {t("dismiss")}
          </button>
        </div>
      </article>
    )))}
  </section>;
}

export function qualitySuccessCriteria(finding: QualityFinding, t: ReviewTranslator = englishReviewTranslator): string {
  const expected = JSON.stringify(finding.expected);
  const summary = expected.length > 240 ? `${expected.slice(0, 237)}...` : expected;
  return t("contractCriteria", { rule: finding.rule_id, expected: summary });
}

export function resolveQualityCandidate(
  bridge: PageContextBridge,
  selection: QualitySelection,
  selectedNodeId?: string,
  t: ReviewTranslator = englishReviewTranslator,
): QualityCandidate | null {
  const nodeId = selectedNodeId ?? selection.finding.observed_node_id;
  if (!nodeId || !bridge.resolve(nodeId)) return null;
  return {
    selection,
    target: { kind: "node", nodeIds: [nodeId] },
    label: selection.finding.element_id ?? selection.finding.rule_id,
    instruction: selection.finding.message,
    successCriteria: qualitySuccessCriteria(selection.finding, t),
    intent: "fix",
    severity: selection.finding.severity,
  };
}
