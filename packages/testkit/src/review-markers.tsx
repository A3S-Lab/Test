import { markerRects, rectStyle } from "./review-utils";
import { designAuditRepairTarget } from "./design-audit-candidates";
import { useReviewI18n } from "./review-locale";
import type { ReviewDraftItem } from "./review-storage";
import type {
  PageContextBridge,
  DesignAuditReportRecord,
  QualityReportRecord,
  Rect,
  SubmittedRepair,
} from "./types";

export type ReviewMarkersProps = {
  visible: boolean;
  bridge: PageContextBridge;
  drafts: ReviewDraftItem[];
  repairs: SubmittedRepair[];
  qualityReports: QualityReportRecord[];
  designAuditReports: DesignAuditReportRecord[];
  onEditDraft(item: ReviewDraftItem): void;
};

export function ReviewMarkers(props: ReviewMarkersProps) {
  const { t } = useReviewI18n();
  if (!props.visible) return <div className="a3s-markers" />;
  const markers = [
    ...props.drafts
      .filter((item) => !item.hidden)
      .map((item) => ({ id: item.draft.id, target: item.draft.target, status: "draft" as const })),
    ...props.repairs.map((repair) => ({ id: repair.id, target: repair.target, status: repair.status })),
    ...props.qualityReports.flatMap((report) => report.findings.flatMap((finding) => (
      finding.observed_node_id
        ? [{
            id: finding.id,
            target: { kind: "node" as const, nodeIds: [finding.observed_node_id] },
            status: "quality" as const,
          }]
        : []
    ))),
    ...props.designAuditReports.flatMap((report) => report.findings.flatMap((finding) => {
      const target = designAuditRepairTarget(props.bridge, finding);
      return target ? [{ id: finding.id, target, status: "design-audit" as const }] : [];
    })),
  ];
  return <div className="a3s-markers">
    {markers.flatMap((marker, markerIndex) => markerRects(marker.target, props.bridge).map((rect, index) => {
      if (marker.status !== "draft") {
        return <span
          key={`${marker.id}-${index}`}
          className={`a3s-marker status-${marker.status}`}
          style={rectStyle(rect)}
          aria-hidden="true"
        >{index === 0 && <span className="a3s-marker-index">{markerIndex + 1}</span>}</span>;
      }
      const draft = props.drafts.find((item) => item.draft.id === marker.id);
      const tooltip = markerTooltipPlacement(rect);
      return <span key={`${marker.id}-${index}`} className="a3s-marker status-draft" style={rectStyle(rect)}>
        <button
          type="button"
          className="a3s-marker-action"
          data-tooltip={draft?.draft.instruction ?? t("openFinding")}
          data-tooltip-align={tooltip.align}
          data-tooltip-side={tooltip.side}
          aria-label={t("editDraftMarker", { message: draft?.draft.instruction ?? marker.id })}
          onClick={() => { if (draft) props.onEditDraft(draft); }}
        >
          <span className="a3s-marker-index" aria-hidden="true">{markerIndex + 1}</span>
        </button>
      </span>;
    }))}
  </div>;
}

function markerTooltipPlacement(rect: Rect): {
  align: "start" | "end";
  side: "bottom" | "top";
} {
  const viewport = typeof window === "undefined" ? null : window.visualViewport;
  const width = viewport?.width ?? (typeof window === "undefined" ? 1024 : window.innerWidth);
  const height = viewport?.height ?? (typeof window === "undefined" ? 768 : window.innerHeight);
  return {
    align: rect.x > width / 2 ? "end" : "start",
    side: rect.y > height - 96 ? "top" : "bottom",
  };
}
