use std::ffi::OsString;

use a3s_test_core::{DriverError, Target, MAX_LAYOUT_COORDINATE_ABS};

pub(crate) const POINTER_SAMPLE_COUNT: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InteractabilityProbe {
    InViewport,
    PointerReachable,
}

pub(crate) fn interactability_probe_args(
    target: &Target,
    probe: InteractabilityProbe,
) -> Result<Vec<OsString>, DriverError> {
    validate_target(target)?;
    let target = serde_json::to_string(target).map_err(|error| {
        DriverError::new(
            "test.driver.web.target_invalid",
            format!("failed to encode interactability target: {error}"),
        )
    })?;
    let probe = match probe {
        InteractabilityProbe::InViewport => "in_viewport",
        InteractabilityProbe::PointerReachable => "pointer_reachable",
    };
    let expression = [
        "(() => {\n  const target = ",
        &target,
        ";\n  const probe = ",
        &serde_json::to_string(probe).expect("static probe name is serializable"),
        ";\n  const coordinateLimit = ",
        &MAX_LAYOUT_COORDINATE_ABS.to_string(),
        ";\n",
        INTERACTABILITY_PROBE,
        "\n})()",
    ]
    .concat();
    Ok(vec!["eval".into(), expression.into()])
}

fn validate_target(target: &Target) -> Result<(), DriverError> {
    match target {
        Target::Css { .. }
        | Target::Role { .. }
        | Target::Text { .. }
        | Target::TestId { .. }
        | Target::Label { .. }
        | Target::Placeholder { .. } => Ok(()),
        Target::Ref { .. } | Target::VisualPoint { .. } => Err(DriverError::new(
            "test.driver.web.target_unsupported",
            "interactability assertions require a stable semantic or CSS locator, not an observation-bound ref or visual point",
        )),
        Target::AutomationId { .. } => Err(DriverError::new(
            "test.driver.web.target_unsupported",
            "interactability automation IDs are available only on GUI surfaces",
        )),
    }
}

const INTERACTABILITY_PROBE: &str = r#"
  const A3S_INTERACTABILITY_PROBE = "a3s.test.interactability/1";
  const allElements = [];
  const visit = (root) => {
    for (const element of root.children ?? []) {
      allElements.push(element);
      visit(element);
      if (element.shadowRoot) visit(element.shadowRoot);
    }
  };
  visit(document);

  const text = (element) => String(element.innerText || element.textContent || "")
    .replace(/\s+/g, " ")
    .trim();
  const labelledText = (element) => {
    const aria = element.getAttribute("aria-label")?.trim();
    if (aria) return aria;
    const labelledBy = element.getAttribute("aria-labelledby")?.trim();
    if (labelledBy) {
      const labels = labelledBy.split(/\s+/).map((id) => {
        const root = element.getRootNode();
        return (typeof root.getElementById === "function" ? root.getElementById(id) : null)
          || element.ownerDocument.getElementById(id);
      }).filter(Boolean).map(text).filter(Boolean);
      if (labels.length) return labels.join(" ");
    }
    const labels = Array.from(element.labels ?? []).map(text).filter(Boolean);
    if (labels.length) return labels.join(" ");
    if (element instanceof HTMLImageElement && element.alt.trim()) return element.alt.trim();
    return text(element) || element.getAttribute("placeholder")?.trim() || "";
  };
  const role = (element) => {
    const explicit = element.getAttribute("role")?.trim().split(/\s+/)[0];
    if (explicit) return explicit;
    const tag = element.tagName.toLowerCase();
    if (tag === "input") {
      if (["button", "submit", "reset"].includes(element.type)) return "button";
      if (element.type === "checkbox") return "checkbox";
      if (element.type === "radio") return "radio";
      if (element.type === "range") return "slider";
      if (element.type === "search") return "searchbox";
      return "textbox";
    }
    if (tag === "a") return element.hasAttribute("href") ? "link" : "";
    if (tag === "select") return element.multiple || element.size > 1 ? "listbox" : "combobox";
    return ({ button:"button", h1:"heading", h2:"heading", h3:"heading", h4:"heading", h5:"heading", h6:"heading", img:"img", nav:"navigation", main:"main", form:"form", option:"option", table:"table", textarea:"textbox" })[tag] || "";
  };
  const semanticMatch = (element, candidate) => {
    if (candidate.type === "test_id") return element.getAttribute("data-testid") === candidate.value || element.getAttribute("data-test-id") === candidate.value;
    if (candidate.type === "placeholder") return element.getAttribute("placeholder") === candidate.value;
    if (candidate.type === "label") return Array.from(element.labels ?? []).map(text).some((value) => value === candidate.value);
    if (candidate.type === "role") return role(element) === candidate.role && labelledText(element) === candidate.name;
    if (candidate.type === "text") return candidate.exact ? text(element) === candidate.value : text(element).includes(candidate.value);
    return false;
  };
  const renderedVisible = (element, semantic) => {
    if (!(element instanceof HTMLElement || element instanceof SVGElement)) return false;
    let current = element;
    while (current) {
      const style = getComputedStyle(current);
      if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse" || Number(style.opacity) === 0) return false;
      if (current.hasAttribute("hidden")) return false;
      if (semantic && current.getAttribute("aria-hidden") === "true") return false;
      const root = current.getRootNode();
      current = current.parentElement || (root instanceof ShadowRoot ? root.host : null);
    }
    const rect = element.getBoundingClientRect();
    return element.getClientRects().length > 0 && rect.width > 0 && rect.height > 0;
  };
  const resolve = () => {
    let matches;
    if (target.type === "css") {
      try {
        matches = Array.from(document.querySelectorAll(target.selector))
          .filter((element) => renderedVisible(element, false));
      } catch (error) {
        return { error: { status: "invalid_target", message: String(error) } };
      }
    } else {
      matches = allElements.filter((element) => semanticMatch(element, target) && renderedVisible(element, true));
    }
    if (matches.length === 0) return { error: { status: "not_found", count: 0 } };
    if (matches.length > 1) return { error: { status: "ambiguous", count: matches.length } };
    return { element: matches[0] };
  };
  const resolved = resolve();
  if (resolved.error) return resolved.error;

  const toRect = (rect) => ({ x: rect.x, y: rect.y, width: rect.width, height: rect.height });
  const validRect = (rect) => {
    const right = rect.x + rect.width;
    const bottom = rect.y + rect.height;
    return [rect.x, rect.y, rect.width, rect.height, right, bottom].every(Number.isFinite)
      && rect.width > 0
      && rect.height > 0
      && rect.width <= coordinateLimit
      && rect.height <= coordinateLimit
      && Math.abs(rect.x) <= coordinateLimit
      && Math.abs(rect.y) <= coordinateLimit
      && Math.abs(right) <= coordinateLimit
      && Math.abs(bottom) <= coordinateLimit;
  };
  const targetRect = toRect(resolved.element.getBoundingClientRect());
  if (!validRect(targetRect)) return { status: "invalid_geometry", subject: "target" };

  const visual = window.visualViewport;
  const viewportRect = {
    x: visual && Number.isFinite(visual.offsetLeft) ? visual.offsetLeft : 0,
    y: visual && Number.isFinite(visual.offsetTop) ? visual.offsetTop : 0,
    width: visual && Number.isFinite(visual.width) && visual.width > 0 ? visual.width : window.innerWidth,
    height: visual && Number.isFinite(visual.height) && visual.height > 0 ? visual.height : window.innerHeight,
  };
  if (!validRect(viewportRect)) return { status: "invalid_geometry", subject: "viewport" };

  const left = Math.max(targetRect.x, viewportRect.x);
  const top = Math.max(targetRect.y, viewportRect.y);
  const right = Math.min(targetRect.x + targetRect.width, viewportRect.x + viewportRect.width);
  const bottom = Math.min(targetRect.y + targetRect.height, viewportRect.y + viewportRect.height);
  const intersectionWidth = Math.max(0, right - left);
  const intersectionHeight = Math.max(0, bottom - top);
  const base = { status: "ok", target_rect: targetRect, viewport_rect: viewportRect };
  if (probe === "in_viewport") return base;
  if (probe !== "pointer_reachable") return { status: "unsupported" };
  if (intersectionWidth === 0 || intersectionHeight === 0) return { ...base, samples: [] };
  if (typeof document.elementFromPoint !== "function") return { status: "unsupported" };

  const deepElementFromPoint = (x, y) => {
    let hit = document.elementFromPoint(x, y);
    while (hit?.shadowRoot && typeof hit.shadowRoot.elementFromPoint === "function") {
      const deeper = hit.shadowRoot.elementFromPoint(x, y);
      if (!deeper || deeper === hit) break;
      hit = deeper;
    }
    return hit;
  };
  const composedContains = (ancestor, node) => {
    let current = node;
    while (current) {
      if (current === ancestor) return true;
      const root = current.getRootNode?.();
      current = current.parentElement || (root instanceof ShadowRoot ? root.host : null);
    }
    return false;
  };
  const fractions = [1 / 6, 1 / 2, 5 / 6];
  const samples = [];
  for (const yFraction of fractions) {
    for (const xFraction of fractions) {
      const x = left + intersectionWidth * xFraction;
      const y = top + intersectionHeight * yFraction;
      samples.push({
        x,
        y,
        reachable: composedContains(resolved.element, deepElementFromPoint(x, y)),
      });
    }
  }
  return { ...base, samples };
"#;
