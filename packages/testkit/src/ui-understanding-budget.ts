import type { UIUnderstandingSnapshot } from "./types";

export function finalizeUIUnderstandingSnapshot(
  snapshot: UIUnderstandingSnapshot,
  encodedBytes: number,
): void {
  for (let attempt = 0; attempt < 8; attempt += 1) {
    fitEncodedBudget(snapshot, encodedBytes);
    snapshot.observationId = `ui-${snapshot.pageRevision}-${uiFingerprint(
      JSON.stringify({
        pageRevision: snapshot.pageRevision,
        viewport: snapshot.viewport,
        scope: snapshot.scope,
        style: snapshot.style,
        layout: snapshot.layout,
        components: snapshot.components,
        stateDiffs: snapshot.stateDiffs,
        motion: snapshot.motion,
      }),
    )}`;
    refreshEncodedBytes(snapshot);
    if (encodedLength(snapshot) <= encodedBytes) return;
  }
}

export function uiFingerprint(value: string): string {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(value)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, "0");
}

function fitEncodedBudget(
  snapshot: UIUnderstandingSnapshot,
  encodedBytes: number,
): void {
  let guard = 0;
  while (encodedLength(snapshot) > encodedBytes && guard < 40) {
    guard += 1;
    snapshot.budget.truncated = true;
    if (!snapshot.budget.reasons.includes("encoded_size_limit"))
      snapshot.budget.reasons.push("encoded_size_limit");
    const collections: unknown[][] = [
      snapshot.layout.nodes,
      snapshot.layout.edges,
      snapshot.components,
      snapshot.stateDiffs,
      snapshot.style.colors,
      snapshot.style.typography,
      snapshot.style.spacing,
      snapshot.style.radii,
      snapshot.style.shadows,
      snapshot.style.zIndices,
      snapshot.style.customProperties,
      snapshot.style.responsiveConditions,
      snapshot.motion.transitions,
      snapshot.motion.animations,
      snapshot.motion.keyframeNames,
      snapshot.motion.stickyNodeIds,
      snapshot.motion.scrollContainerNodeIds,
      snapshot.motion.canvasNodeIds,
      snapshot.motion.mediaNodeIds,
      snapshot.evidence.sampledNodeIds,
    ];
    const largest = collections.sort(
      (left, right) => right.length - left.length,
    )[0];
    if (!largest || largest.length === 0) break;
    largest.splice(Math.max(1, Math.ceil(largest.length / 2)));
    const retainedLayoutIds = new Set(
      snapshot.layout.nodes.map((node) => node.nodeId),
    );
    snapshot.layout.edges = snapshot.layout.edges.filter(
      (edge) =>
        retainedLayoutIds.has(edge.fromNodeId) &&
        retainedLayoutIds.has(edge.toNodeId),
    );
  }
}

function refreshEncodedBytes(snapshot: UIUnderstandingSnapshot): void {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const bytes = encodedLength(snapshot);
    if (snapshot.budget.used.encodedBytes === bytes) return;
    snapshot.budget.used.encodedBytes = bytes;
  }
}

function encodedLength(value: unknown): number {
  return new TextEncoder().encode(JSON.stringify(value)).byteLength;
}
