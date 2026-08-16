import type { ReviewCallback } from "./review-integration";
import type { A3SReviewLocale, A3SReviewMessageOverrides } from "./review-locale";
import type { RepairDraft, SubmittedRepair } from "./types";

export type A3SReviewCopyEvent = {
  format: "markdown" | "json";
  text: string;
  drafts: RepairDraft[];
};

export type A3SReviewOverlayProps = {
  enabled?: boolean;
  defaultOpen?: boolean;
  autoSend?: boolean;
  locale?: A3SReviewLocale;
  messages?: A3SReviewMessageOverrides;
  copyToClipboard?: (text: string) => void | Promise<void>;
  onCopied?: ReviewCallback<A3SReviewCopyEvent>;
  onDraftAdded?: ReviewCallback<RepairDraft>;
  onDraftUpdated?: ReviewCallback<RepairDraft>;
  onDraftDeleted?: ReviewCallback<RepairDraft>;
  onDraftsCleared?: ReviewCallback<RepairDraft[]>;
  onSubmitted?: ReviewCallback<SubmittedRepair[]>;
};
