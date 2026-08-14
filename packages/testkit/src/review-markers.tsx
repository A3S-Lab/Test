import { markerRects, rectStyle } from "./review-utils";
import { designAuditRepairTarget } from "./design-audit-candidates";
import type { ReviewDraftItem } from "./review-storage";
import type {
  PageContextBridge,
  DesignAuditReportRecord,
  QualityReportRecord,
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
    {markers.flatMap((marker) => markerRects(marker.target, props.bridge).map((rect, index) => {
      if (marker.status !== "draft") {
        return <span
          key={`${marker.id}-${index}`}
          className={`a3s-marker status-${marker.status}`}
          style={rectStyle(rect)}
          aria-hidden="true"
        />;
      }
      const draft = props.drafts.find((item) => item.draft.id === marker.id);
      return <span key={`${marker.id}-${index}`} className="a3s-marker status-draft" style={rectStyle(rect)}>
        <button
          type="button"
          className="a3s-marker-action"
          aria-label={`Edit draft marker: ${draft?.draft.instruction ?? marker.id}`}
          onClick={() => { if (draft) props.onEditDraft(draft); }}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M3 11.8 3.5 9l6.7-6.7a1.4 1.4 0 0 1 2 0l1.5 1.5a1.4 1.4 0 0 1 0 2L7 12.5l-2.8.5Z" />
            <path d="m9.3 3.2 3.5 3.5" />
          </svg>
        </button>
      </span>;
    }))}
  </div>;
}
