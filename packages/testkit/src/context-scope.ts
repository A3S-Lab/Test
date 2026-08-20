import { boundaryElements } from "./boundary";
import { overlaps, walkElements } from "./dom";
import type { BoundaryRegistration, ContextScope } from "./types";

export function elementsForContextScope(
  scope: ContextScope,
  resolveNode: (nodeId: string) => Element | null,
  boundaries: ReadonlyMap<string, BoundaryRegistration>,
): Element[] {
  let elements: Element[];
  if (scope.kind === "node") {
    const root = resolveNode(scope.nodeId);
    elements = root ? walkElements(root) : [];
  } else if (scope.kind === "component") {
    const registration = boundaries.get(scope.componentId);
    const roots = registration ? boundaryElements(registration) : [];
    elements = roots.flatMap((root) => walkElements(root));
  } else {
    elements = walkElements(document);
  }
  const uniqueElements = Array.from(new Set(elements));
  if (scope.kind !== "region") return uniqueElements;
  return uniqueElements.filter((element) => {
    const rect = element.getBoundingClientRect();
    const candidate =
      scope.space === "document"
        ? {
            x: rect.x + scrollX,
            y: rect.y + scrollY,
            width: rect.width,
            height: rect.height,
          }
        : { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    return overlaps(candidate, scope);
  });
}
