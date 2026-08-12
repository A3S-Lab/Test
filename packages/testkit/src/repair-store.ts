import type {
  RepairDraft,
  RepairBatch,
  RepairEvent,
  RepairHumanAction,
  RepairHumanActionInput,
  RepairStatus,
  RepairSubmission,
  RepairContext,
  RepairThreadMessage,
  SubmittedRepair,
  TestKitEvent,
} from "./types";

const VALID_TRANSITIONS: Record<RepairStatus, ReadonlySet<RepairStatus>> = {
  draft: new Set(["queued", "cancelled"]),
  queued: new Set(["claimed", "cancelled", "failed"]),
  claimed: new Set(["queued", "repairing", "cancelled", "needs_input", "failed"]),
  repairing: new Set(["verifying", "needs_input", "failed"]),
  verifying: new Set(["review_ready", "verification_failed", "needs_input", "failed"]),
  needs_input: new Set(["queued", "cancelled", "failed"]),
  verification_failed: new Set(["queued", "cancelled", "failed"]),
  review_ready: new Set(["resolved", "reopened", "dismissed"]),
  resolved: new Set(["reopened"]),
  dismissed: new Set(["reopened"]),
  cancelled: new Set(["reopened"]),
  failed: new Set(["reopened"]),
  reopened: new Set(["queued", "cancelled"]),
};

type RepairStoreOptions = {
  pageId: string;
  storage: "local" | "session" | "memory";
  pageRevision(): number;
  pageUrl(): string;
  contextFor(draft: RepairDraft): { revision: number; context: RepairContext };
  repairEndpoint?: string;
  emit(event: TestKitEvent): void;
};

function id(prefix: string): string {
  const suffix = globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  return `${prefix}-${suffix}`;
}

function storageFor(mode: RepairStoreOptions["storage"]): Storage | null {
  try {
    if (mode === "local") return window.localStorage;
    if (mode === "session") return window.sessionStorage;
  } catch {
    return null;
  }
  return null;
}

function validDraft(value: RepairDraft): boolean {
  return Boolean(
    value &&
      typeof value.id === "string" &&
      value.id.length > 0 &&
      typeof value.instruction === "string" &&
      value.instruction.trim().length > 0 &&
      value.instruction.length <= 8_192 &&
      value.target &&
      Array.isArray(value.target.nodeIds) &&
      validRelations(value),
  );
}

function validRelations(value: RepairDraft): boolean {
  if (value.relations === undefined) return true;
  if (!Array.isArray(value.relations) || value.relations.length > 100) return false;
  const ids = new Set<string>();
  return value.relations.every((relation) => {
    if (
      !relation ||
      relation.kind !== "conflicts_with" ||
      typeof relation.findingId !== "string" ||
      relation.findingId.length === 0 ||
      relation.findingId.length > 128 ||
      relation.findingId === value.id ||
      ids.has(relation.findingId)
    ) return false;
    ids.add(relation.findingId);
    return true;
  });
}

export class RepairStore {
  readonly #options: RepairStoreOptions;
  readonly #repairs = new Map<string, SubmittedRepair>();
  readonly #sequences = new Map<string, number>();
  readonly #requests = new Set<string>();
  readonly #claimed = new Set<string>();
  readonly #replies = new Map<string, RepairThreadMessage[]>();
  readonly #actions: RepairHumanAction[] = [];
  readonly #claimedActions = new Set<string>();
  readonly #storage: Storage | null;
  readonly #storageKey: string;

  constructor(options: RepairStoreOptions) {
    this.#options = options;
    this.#storage = storageFor(options.storage);
    this.#storageKey = `a3s-testkit-repairs:${options.pageId}:${location.pathname}`;
    this.#load();
  }

  submit(submission: RepairSubmission): SubmittedRepair[] {
    const drafts = submission.findings.filter(validDraft);
    if (drafts.length === 0) return [];
    const batchId = submission.batchId?.trim() || id("batch");
    const now = new Date().toISOString();
    const created: SubmittedRepair[] = [];
    for (const draft of drafts) {
      const existing = this.#repairs.get(draft.id);
      if (existing) {
        created.push(existing);
        continue;
      }
      const captured = this.#options.contextFor(draft);
      const repair: SubmittedRepair = {
        ...structuredClone(draft),
        batchId,
        pageId: this.#options.pageId,
        url: this.#options.pageUrl(),
        contextRevision: captured.revision,
        context: captured.context,
        status: "queued",
        submittedAt: now,
      };
      this.#repairs.set(repair.id, repair);
      this.#sequences.set(repair.id, 0);
      created.push(structuredClone(repair));
    }
    this.#persist();
    if (created.length > 0) this.#options.emit({ type: "repair.submitted", repairs: created });
    if (created.length > 0) void this.#forward(created);
    return created;
  }

  list(): SubmittedRepair[] {
    return Array.from(this.#repairs.values())
      .sort((left, right) => left.submittedAt.localeCompare(right.submittedAt))
      .map((repair) => structuredClone(repair));
  }

  batches(): RepairBatch[] {
    const grouped = new Map<string, SubmittedRepair[]>();
    for (const repair of this.list()) {
      const records = grouped.get(repair.batchId) ?? [];
      records.push(repair);
      grouped.set(repair.batchId, records);
    }
    return Array.from(grouped, ([id, repairs]) => ({
      id,
      findingIds: repairs.map((repair) => repair.id),
      status: batchStatus(repairs.map((repair) => repair.status)),
      results: repairs.map((repair) => ({ findingId: repair.id, status: repair.status })),
    }));
  }

  queued(limit = 50): SubmittedRepair[] {
    const bounded = Math.max(1, Math.min(100, Math.trunc(limit)));
    const selected = this.peek(bounded)
      .filter((repair) => repair.status === "queued" && !this.#claimed.has(repair.id))
      .slice(0, bounded);
    for (const repair of selected) this.#claimed.add(repair.id);
    return selected;
  }

  peek(limit = 50): SubmittedRepair[] {
    const bounded = Math.max(1, Math.min(100, Math.trunc(limit)));
    return this.list()
      .filter((repair) => repair.status === "queued")
      .slice(0, bounded);
  }

  apply(event: RepairEvent): SubmittedRepair | null {
    if (this.#requests.has(event.requestId)) return this.#clone(event.findingId);
    const repair = this.#repairs.get(event.findingId);
    if (!repair) return null;
    const currentSequence = this.#sequences.get(event.findingId) ?? 0;
    if (!Number.isSafeInteger(event.sequence) || event.sequence <= currentSequence) return null;
    if (!VALID_TRANSITIONS[repair.status].has(event.status) && event.status !== repair.status) return null;
    this.#requests.add(event.requestId);
    const acknowledgedAction = this.#actions.findIndex((action) => action.requestId === event.requestId);
    if (acknowledgedAction >= 0) {
      this.#actions.splice(acknowledgedAction, 1);
      this.#claimedActions.delete(event.requestId);
    }
    this.#sequences.set(event.findingId, event.sequence);
    repair.status = event.status;
    if (event.status === "queued") this.#claimed.delete(event.findingId);
    if (event.message?.trim()) {
      const replies = this.#replies.get(event.findingId) ?? [];
      if (!replies.some((candidate) => candidate.requestId === event.requestId)) {
        replies.push({
          requestId: event.requestId,
          findingId: event.findingId,
          actor: event.actor,
          timestamp: event.timestamp,
          message: event.message,
        });
        this.#replies.set(event.findingId, replies.slice(-100));
      }
    }
    this.#persist();
    const result = structuredClone(repair);
    this.#options.emit({ type: "repair.updated", repair: result, event: structuredClone(event) });
    return result;
  }

  addReply(reply: RepairThreadMessage): boolean {
    if (!this.#repairs.has(reply.findingId) || !reply.requestId.trim() || !reply.message.trim() || reply.message.length > 8_192) return false;
    const replies = this.#replies.get(reply.findingId) ?? [];
    if (replies.some((candidate) => candidate.requestId === reply.requestId)) return true;
    replies.push(structuredClone(reply));
    this.#replies.set(reply.findingId, replies.slice(-100));
    this.#persist();
    return true;
  }

  replies(findingId: string): RepairThreadMessage[] {
    return structuredClone(this.#replies.get(findingId) ?? []);
  }

  submitAction(input: RepairHumanActionInput): RepairHumanAction | null {
    const repair = this.#repairs.get(input.findingId);
    const message = input.message?.trim();
    if (!repair || !validHumanAction(repair.status, input.action, message)) return null;
    const action: RepairHumanAction = {
      requestId: id("human"),
      findingId: input.findingId,
      action: input.action,
      timestamp: new Date().toISOString(),
      ...(message ? { message } : {}),
    };
    this.#actions.push(action);
    if (input.action === "reply" && message) {
      this.addReply({
        requestId: action.requestId,
        findingId: action.findingId,
        actor: "human",
        timestamp: action.timestamp,
        message,
      });
    }
    if (this.#actions.length > 100) this.#actions.splice(0, this.#actions.length - 100);
    this.#persist();
    this.#options.emit({ type: "repair.action_submitted", action: structuredClone(action) });
    void this.#forwardActions([action]);
    return structuredClone(action);
  }

  takeActions(limit = 50): RepairHumanAction[] {
    const bounded = Math.max(1, Math.min(100, Math.trunc(limit)));
    const selected = this.#actions
      .filter((action) => !this.#claimedActions.has(action.requestId))
      .slice(0, bounded);
    for (const action of selected) this.#claimedActions.add(action.requestId);
    return structuredClone(selected);
  }

  #clone(findingId: string): SubmittedRepair | null {
    const repair = this.#repairs.get(findingId);
    return repair ? structuredClone(repair) : null;
  }

  #load(): void {
    if (!this.#storage) return;
    try {
      const encoded = this.#storage.getItem(this.#storageKey);
      if (!encoded) return;
      const parsed = JSON.parse(encoded) as { repairs?: SubmittedRepair[]; sequences?: Record<string, number>; replies?: Record<string, RepairThreadMessage[]>; actions?: RepairHumanAction[] };
      for (const repair of parsed.repairs ?? []) {
        if (repair.pageId === this.#options.pageId && typeof repair.id === "string") this.#repairs.set(repair.id, repair);
      }
      for (const [findingId, sequence] of Object.entries(parsed.sequences ?? {})) {
        if (Number.isSafeInteger(sequence) && sequence >= 0) this.#sequences.set(findingId, sequence);
      }
      for (const [findingId, replies] of Object.entries(parsed.replies ?? {})) {
        if (Array.isArray(replies)) this.#replies.set(findingId, replies.slice(-100));
      }
      for (const action of parsed.actions ?? []) {
        if (validStoredHumanAction(action, this.#repairs)) this.#actions.push(action);
      }
    } catch {
      // Corrupt page-local state is ignored; submitted server state is authoritative.
    }
  }

  #persist(): void {
    if (!this.#storage) return;
    try {
      this.#storage.setItem(
        this.#storageKey,
        JSON.stringify({ repairs: Array.from(this.#repairs.values()), sequences: Object.fromEntries(this.#sequences), replies: Object.fromEntries(this.#replies), actions: this.#actions }),
      );
    } catch {
      // Storage may be disabled or full. The live bridge remains usable.
    }
  }

  async #forward(repairs: SubmittedRepair[]): Promise<void> {
    const endpoint = this.#options.repairEndpoint;
    if (!endpoint) return;
    try {
      const response = await fetch(endpoint, {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ protocol: "a3s.test.repair/1", repairs }),
      });
      if (!response.ok) throw new Error(`repair endpoint returned ${response.status}`);
    } catch {
      // The page-local queue remains available for browser-owned pickup and retry.
    }
  }

  async #forwardActions(actions: RepairHumanAction[]): Promise<void> {
    const endpoint = this.#options.repairEndpoint;
    if (!endpoint) return;
    try {
      const response = await fetch(endpoint, {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ protocol: "a3s.test.repair/1", actions }),
      });
      if (!response.ok) throw new Error(`repair endpoint returned ${response.status}`);
    } catch {
      // Browser-owned pickup can retry the durable page-local action queue.
    }
  }
}

function validHumanAction(status: RepairStatus, action: RepairHumanAction["action"], message?: string): boolean {
  if (action === "reply") return status === "needs_input" && Boolean(message) && message!.length <= 8_192;
  if (action === "accept" || action === "dismiss") return status === "review_ready";
  return ["review_ready", "resolved", "dismissed", "cancelled", "failed", "verification_failed"].includes(status);
}

function batchStatus(statuses: RepairStatus[]): RepairBatch["status"] {
  if (statuses.every((status) => status === "resolved")) return "resolved";
  const terminal = new Set<RepairStatus>(["resolved", "dismissed", "cancelled", "failed", "verification_failed", "review_ready"]);
  if (statuses.some((status) => status === "failed" || status === "verification_failed") && statuses.every((status) => terminal.has(status))) {
    return "completed_with_failures";
  }
  if (statuses.some((status) => status === "needs_input")) return "needs_input";
  if (statuses.every((status) => status === "review_ready")) return "review_ready";
  if (statuses.some((status) => status !== "queued")) return "in_progress";
  return "queued";
}

function validStoredHumanAction(action: RepairHumanAction, repairs: Map<string, SubmittedRepair>): boolean {
  return Boolean(
    action &&
      typeof action.requestId === "string" &&
      action.requestId.length > 0 &&
      typeof action.findingId === "string" &&
      repairs.has(action.findingId) &&
      ["reply", "accept", "dismiss", "reopen"].includes(action.action) &&
      typeof action.timestamp === "string" &&
      (!action.message || action.message.length <= 8_192),
  );
}
