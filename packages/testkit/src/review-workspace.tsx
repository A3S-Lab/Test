import { DesignAuditCandidates } from "./design-audit-candidates";
import { QualityCandidates } from "./quality-candidates";
import { REVIEW_KEY_SHORTCUTS } from "./review-input-policy";
import { reviewActorLabel, reviewStatusLabel, reviewTargetSummary, useReviewI18n } from "./review-locale";
import type { ReviewDraftItem } from "./review-storage";
import type {
  DesignAuditFinding,
  DesignAuditReportRecord,
  PageContextBridge,
  QualityFinding,
  QualityReportRecord,
  RepairDraft,
  RepairHumanActionKind,
  SubmittedRepair,
} from "./types";

export type ReviewWorkspaceProps = {
  bridge: PageContextBridge;
  drafts: ReviewDraftItem[];
  repairs: SubmittedRepair[];
  qualityReports: QualityReportRecord[];
  designAuditReports: DesignAuditReportRecord[];
  findingCount: number;
  selectedCount: number;
  candidatePresent: boolean;
  replyFindingId: string | null;
  replyMessage: string;
  replyTriggerRefs: { current: Map<string, HTMLButtonElement> };
  onReviewQuality(reportId: string, finding: QualityFinding): void;
  onDismissQuality(reportId: string, findingId: string): void;
  onReviewDesignAudit(reportId: string, finding: DesignAuditFinding): void;
  onDismissDesignAudit(reportId: string, findingId: string): void;
  onSelectDraft(findingId: string, selected: boolean): void;
  onSubmitDraft(draft: RepairDraft): void;
  onEditDraft(item: ReviewDraftItem): void;
  onToggleDraftMarker(findingId: string): void;
  onDeleteDraft(draft: RepairDraft): void;
  onStartReply(findingId: string): void;
  onReplyMessage(message: string): void;
  onCancelReply(findingId: string): void;
  onHumanAction(findingId: string, action: RepairHumanActionKind, message?: string): void;
  onClearDrafts(): void;
  onCopyDrafts(format: "markdown" | "json"): void;
  onSubmitSelected(): void;
};

export function ReviewWorkspace(props: ReviewWorkspaceProps) {
  const { t } = useReviewI18n();
  const isEmpty = props.drafts.length === 0
    && props.repairs.length === 0
    && props.qualityReports.length === 0
    && props.designAuditReports.length === 0
    && !props.candidatePresent;

  return <section className="a3s-workspace" aria-label={t("reviewWorkspace")}>
    <header className="a3s-workspace-header">
      <span>
        <strong>{t("reviewWorkspace")}</strong>
        <small>{props.findingCount > 0 ? t("inThisPage", { count: props.findingCount }) : t("noSavedFindings")}</small>
      </span>
    </header>
    <div className="a3s-workspace-scroll">
      <QualityCandidates
        reports={props.qualityReports}
        onReview={props.onReviewQuality}
        onDismiss={props.onDismissQuality}
      />
      <DesignAuditCandidates
        reports={props.designAuditReports}
        onReview={props.onReviewDesignAudit}
        onDismiss={props.onDismissDesignAudit}
      />
      <section className="a3s-list" aria-label={t("draftAndSubmittedFindings")} tabIndex={0}>
        {props.drafts.map((item) => <article key={item.draft.id} className={`a3s-item${item.hidden ? " is-hidden" : ""}`}>
          <label>
            <input
              type="checkbox"
              aria-label={t("selectDraft", { message: item.draft.instruction })}
              checked={item.selected}
              onChange={(event) => props.onSelectDraft(item.draft.id, event.target.checked)}
            />
            <span>
              <strong>{item.draft.instruction}</strong>
              <small>{reviewTargetSummary(t, item.draft.target)}{item.draft.designReference ? ` · ${t(item.draft.designReference.kind === "sketch" ? "sketchReference" : "screenshotReference")}` : ""} · {t("draft")}</small>
            </span>
          </label>
          <div>
            <button type="button" aria-label={t("sendDraftAutoFix", { message: item.draft.instruction })} onClick={() => props.onSubmitDraft(item.draft)}>{t("sendAndAutoFix")}</button>
            <button type="button" className="quiet" aria-label={t("editDraftAction", { message: item.draft.instruction })} onClick={() => props.onEditDraft(item)}>{t("edit")}</button>
            <button type="button" className="quiet" aria-label={t(item.hidden ? "reopenMarkerForDraft" : "hideMarkerForDraft", { message: item.draft.instruction })} onClick={() => props.onToggleDraftMarker(item.draft.id)}>{t(item.hidden ? "reopenMarker" : "hideMarker")}</button>
            <button type="button" className="quiet" aria-label={t("deleteDraftAction", { message: item.draft.instruction })} onClick={() => props.onDeleteDraft(item.draft)}>{t("delete")}</button>
          </div>
        </article>)}
        {props.repairs.map((repair) => {
          const replies = props.bridge.listRepairReplies(repair.id);
          return <article key={repair.id} className="a3s-item submitted">
            <span className={`a3s-status status-badge status-${repair.status}`}>{reviewStatusLabel(t, repair.status)}</span>
            <strong>{repair.instruction}</strong>
            <small>{reviewTargetSummary(t, repair.target)}{repair.designReference ? ` · ${t(repair.designReference.kind === "sketch" ? "sketchReference" : "screenshotReference")}` : ""} · {t("revision", { revision: repair.contextRevision })}</small>
            {replies.length > 0 && <ol className="a3s-thread" aria-label={t("repairConversation", { message: repair.instruction })}>
              {replies.map((reply) => <li key={reply.requestId}><span>{reviewActorLabel(t, reply.actor)}</span><p>{reply.message}</p></li>)}
            </ol>}
            {repair.status === "needs_input" && <div className="a3s-human-actions">
              {props.replyFindingId === repair.id ? <>
                <label className="a3s-reply-label">{t("replyToCodingAgent")}
                  <textarea aria-label={t("replyToCodingAgentAbout", { message: repair.instruction })} autoFocus maxLength={8192} value={props.replyMessage} onChange={(event) => props.onReplyMessage(event.target.value)} />
                </label>
                <button type="button" disabled={!props.replyMessage.trim()} onClick={() => props.onHumanAction(repair.id, "reply", props.replyMessage)}>{t("sendReply")}</button>
                <button type="button" className="quiet" onClick={() => props.onCancelReply(repair.id)}>{t("cancelReply")}</button>
              </> : <button
                ref={(element) => {
                  if (element) props.replyTriggerRefs.current.set(repair.id, element);
                  else props.replyTriggerRefs.current.delete(repair.id);
                }}
                type="button"
                aria-label={t("replyAboutRepair", { message: repair.instruction })}
                onClick={() => props.onStartReply(repair.id)}
              >{t("reply")}</button>}
            </div>}
            {repair.status === "review_ready" && <div className="a3s-human-actions" aria-label={t("reviewRepair", { message: repair.instruction })}>
              <button type="button" aria-label={t("acceptRepairAction", { message: repair.instruction })} onClick={() => props.onHumanAction(repair.id, "accept")}>{t("acceptRepair")}</button>
              <button type="button" className="quiet" aria-label={t("rejectRepairAction", { message: repair.instruction })} onClick={() => props.onHumanAction(repair.id, "dismiss")}>{t("reject")}</button>
              <button type="button" className="quiet" aria-label={t("reopenRepairAction", { message: repair.instruction })} onClick={() => props.onHumanAction(repair.id, "reopen")}>{t("reopen")}</button>
            </div>}
            {["resolved", "dismissed", "cancelled", "failed", "verification_failed"].includes(repair.status) && <div className="a3s-human-actions">
              <button type="button" className="quiet" aria-label={t("reopenRepairAction", { message: repair.instruction })} onClick={() => props.onHumanAction(repair.id, "reopen")}>{t("reopen")}</button>
            </div>}
          </article>;
        })}
        {isEmpty && <p className="a3s-empty">{t("emptyWorkspace")}</p>}
      </section>
    </div>
    {props.drafts.length > 0 && <footer>
      <div className="a3s-workspace-secondary-actions">
        <button type="button" className="quiet" title={t("clearDraftsTitle")} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.clear} onClick={props.onClearDrafts}>{t("clearDrafts")}</button>
        <button type="button" className="quiet" title={t("copyMarkdownTitle")} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.copy} onClick={() => props.onCopyDrafts("markdown")}>{t("copyMarkdown")}</button>
        <button type="button" className="quiet" onClick={() => props.onCopyDrafts("json")}>{t("copyJson")}</button>
      </div>
      <div className="a3s-workspace-send-actions">
        <button type="button" disabled={props.selectedCount === 0} onClick={props.onSubmitSelected}>{t("sendSelected", { count: props.selectedCount })}</button>
      </div>
    </footer>}
  </section>;
}
