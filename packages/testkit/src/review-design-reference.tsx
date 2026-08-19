import { useState } from "react";
import { DesignBoard } from "./design-board";
import type { OverlayTheme } from "./review-model";
import type { RepairDesignReference } from "./types";

export type ReviewDesignReferenceState = {
  reference: RepairDesignReference | null;
  boardOpen: boolean;
  clear(): void;
  load(reference: RepairDesignReference | null): void;
  open(): void;
  close(): void;
  remove(): void;
  attach(reference: RepairDesignReference): void;
};

export function useReviewDesignReference(): ReviewDesignReferenceState {
  const [reference, setReference] = useState<RepairDesignReference | null>(null);
  const [boardOpen, setBoardOpen] = useState(false);

  return {
    reference,
    boardOpen,
    clear: () => {
      setReference(null);
      setBoardOpen(false);
    },
    load: (nextReference) => {
      setReference(nextReference);
      setBoardOpen(false);
    },
    open: () => setBoardOpen(true),
    close: () => setBoardOpen(false),
    remove: () => setReference(null),
    attach: (nextReference) => {
      setReference(nextReference);
      setBoardOpen(false);
    },
  };
}

export function ReviewDesignReferenceBoard({
  active,
  design,
  idPrefix,
  theme,
  onAnnounce,
}: {
  active: boolean;
  design: ReviewDesignReferenceState;
  idPrefix: string;
  theme: OverlayTheme;
  onAnnounce(message: string): void;
}) {
  if (!active || !design.boardOpen) return null;
  return <DesignBoard
    idPrefix={idPrefix}
    initialReference={design.reference}
    theme={theme}
    onAttach={(reference) => {
      design.attach(reference);
      onAnnounce(`${reference.kind === "sketch" ? "Sketch" : "Screenshot"} attached to finding`);
    }}
    onCancel={design.close}
  />;
}
