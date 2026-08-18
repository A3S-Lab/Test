use std::ffi::OsString;

use a3s_test_core::{DriverError, Target, MAX_LAYOUT_COORDINATE_ABS};

pub(crate) fn layout_probe_args(
    target: &Target,
    relative_to: &Target,
) -> Result<Vec<OsString>, DriverError> {
    validate_target(target, "target")?;
    validate_target(relative_to, "relative_to")?;
    let target = serde_json::to_string(target).map_err(|error| {
        DriverError::new(
            "test.driver.web.target_invalid",
            format!("failed to encode layout target: {error}"),
        )
    })?;
    let relative_to = serde_json::to_string(relative_to).map_err(|error| {
        DriverError::new(
            "test.driver.web.target_invalid",
            format!("failed to encode relative layout target: {error}"),
        )
    })?;
    let coordinate_limit = MAX_LAYOUT_COORDINATE_ABS.to_string();
    let expression = [
        "(() => {\n  const target = ",
        &target,
        ";\n  const relativeTo = ",
        &relative_to,
        ";\n  const A3S_MAX_LAYOUT_COORDINATE_ABS = ",
        &coordinate_limit,
        ";\n",
        LAYOUT_PROBE,
        "\n})()",
    ]
    .concat();
    Ok(vec!["eval".into(), expression.into()])
}

fn validate_target(target: &Target, subject: &str) -> Result<(), DriverError> {
    match target {
        Target::Css { .. }
        | Target::Role { .. }
        | Target::Text { .. }
        | Target::TestId { .. }
        | Target::Label { .. }
        | Target::Placeholder { .. } => Ok(()),
        Target::Ref { .. } | Target::VisualPoint { .. } => Err(DriverError::new(
            "test.driver.web.target_unsupported",
            format!(
                "layout {subject} requires a stable semantic or CSS locator, not an observation-bound ref or visual point"
            ),
        )),
        Target::AutomationId { .. } => Err(DriverError::new(
            "test.driver.web.target_unsupported",
            format!("layout {subject} automation IDs are available only on GUI surfaces"),
        )),
    }
}

const LAYOUT_PROBE: &str = r#"
  const A3S_LAYOUT_PROBE = "a3s.test.layout/1";
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
  const resolve = (candidate, subject) => {
    let matches;
    if (candidate.type === "css") {
      try {
        matches = Array.from(document.querySelectorAll(candidate.selector))
          .filter((element) => renderedVisible(element, false));
      } catch (error) {
        return { error: { status: "invalid_target", subject, message: String(error) } };
      }
    } else {
      matches = allElements.filter((element) => semanticMatch(element, candidate) && renderedVisible(element, true));
    }
    if (matches.length === 0) return { error: { status: "not_found", subject, count: 0 } };
    if (matches.length > 1) return { error: { status: "ambiguous", subject, count: matches.length } };
    return { element: matches[0] };
  };
  const subject = resolve(target, "target");
  if (subject.error) return subject.error;
  const reference = resolve(relativeTo, "relative_to");
  if (reference.error) return reference.error;

  const toRect = (element) => {
    const rect = element.getBoundingClientRect();
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
  };
  const validRect = (rect) => {
    const right = rect.x + rect.width;
    const bottom = rect.y + rect.height;
    return [rect.x, rect.y, rect.width, rect.height, right, bottom].every(Number.isFinite)
      && rect.width > 0
      && rect.height > 0
      && rect.width <= A3S_MAX_LAYOUT_COORDINATE_ABS
      && rect.height <= A3S_MAX_LAYOUT_COORDINATE_ABS
      && Math.abs(rect.x) <= A3S_MAX_LAYOUT_COORDINATE_ABS
      && Math.abs(rect.y) <= A3S_MAX_LAYOUT_COORDINATE_ABS
      && Math.abs(right) <= A3S_MAX_LAYOUT_COORDINATE_ABS
      && Math.abs(bottom) <= A3S_MAX_LAYOUT_COORDINATE_ABS;
  };
  const targetRect = toRect(subject.element);
  if (!validRect(targetRect)) return { status: "invalid_geometry", subject: "target" };
  const relativeRect = toRect(reference.element);
  if (!validRect(relativeRect)) return { status: "invalid_geometry", subject: "relative_to" };
  return { status: "ok", target_rect: targetRect, relative_rect: relativeRect };
"#;
