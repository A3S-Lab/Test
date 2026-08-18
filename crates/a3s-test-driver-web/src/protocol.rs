use std::collections::BTreeMap;
use std::ffi::OsString;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use a3s_test_core::{
    DriverError, ElementState, LoadState, Target, WaitCondition, MAX_RENDERED_TEXT_ITEMS,
};
use serde_json::Value;

use crate::{AgentBrowserConfig, CommandInvocation};

mod interactability;
mod layout;

pub(crate) use interactability::{
    interactability_probe_args, InteractabilityProbe, POINTER_SAMPLE_COUNT,
};
pub(crate) use layout::layout_probe_args;

pub(crate) fn invocation(
    config: &AgentBrowserConfig,
    namespace: &str,
    session: &str,
    runtime_dir: &Path,
    action_args: Vec<OsString>,
) -> CommandInvocation {
    let mut args = config.command.prefix();
    args.extend([
        OsString::from("--session"),
        OsString::from(session),
        OsString::from("--json"),
        OsString::from("--headed"),
        OsString::from(config.headed.to_string()),
    ]);
    let closes_session = action_args.first().is_some_and(|action| action == "close");
    if !closes_session {
        args.extend(config.command.domain_policy_args(&config.network_policy));
    }
    args.extend(action_args);

    let mut env = BTreeMap::new();
    let (namespace_name, namespace_value) = config.command.namespace_environment(namespace);
    env.insert(namespace_name, namespace_value);
    // agent-browser 0.26.x resets its idle timer when a command starts, not
    // when it completes. Keep the daemon alive for every command that A3S Test
    // still considers valid, even when the requested between-turn deadline is
    // shorter than the per-command deadline.
    let daemon_idle_timeout = config.idle_timeout.max(config.command_timeout);
    let (idle_name, idle_value) = config.command.idle_environment(daemon_idle_timeout);
    env.insert(idle_name, idle_value);
    let (runtime_name, runtime_value) = config.command.runtime_environment(runtime_dir);
    env.insert(runtime_name, runtime_value);
    if let Some((arguments_name, arguments_value)) = config
        .command
        .launch_arguments_environment(config.headed, config.microphone)
    {
        env.insert(arguments_name, arguments_value);
    }
    for (policy_name, policy_value) in config
        .command
        .network_policy_environment(&config.network_policy)
    {
        env.insert(policy_name, policy_value);
    }

    CommandInvocation {
        program: config.command.program().to_path_buf(),
        args,
        env,
        env_remove: Default::default(),
        timeout: config.command_timeout,
    }
}

pub(crate) fn target_action(
    target: &Target,
    action: &str,
    value: Option<&str>,
) -> Result<Vec<OsString>, DriverError> {
    let mut args = match target {
        Target::Ref { value: selector } | Target::Css { selector } => {
            vec![OsString::from(action), OsString::from(selector)]
        }
        Target::Role { role, name } => {
            let mut args = vec![
                OsString::from("find"),
                OsString::from("role"),
                OsString::from(role),
                OsString::from(action),
            ];
            if let Some(value) = value {
                args.push(OsString::from(value));
            }
            args.extend([OsString::from("--name"), OsString::from(name)]);
            return Ok(args);
        }
        Target::Text { value: text, exact } => {
            let mut args = vec![
                OsString::from("find"),
                OsString::from("text"),
                OsString::from(text),
                OsString::from(action),
            ];
            if let Some(value) = value {
                args.push(OsString::from(value));
            }
            if *exact {
                args.push(OsString::from("--exact"));
            }
            return Ok(args);
        }
        Target::TestId { value: id } => {
            vec![
                OsString::from("find"),
                OsString::from("testid"),
                OsString::from(id),
                OsString::from(action),
            ]
        }
        Target::Label { value: label } => {
            vec![
                OsString::from("find"),
                OsString::from("label"),
                OsString::from(label),
                OsString::from(action),
            ]
        }
        Target::Placeholder { value: placeholder } => {
            vec![
                OsString::from("find"),
                OsString::from("placeholder"),
                OsString::from(placeholder),
                OsString::from(action),
            ]
        }
        Target::AutomationId { .. } | Target::VisualPoint { .. } => {
            return Err(DriverError::new(
                "test.driver.web.target_unsupported",
                "automation_id and visual_point targets are only available on GUI surfaces",
            ));
        }
    };
    if let Some(value) = value {
        args.push(OsString::from(value));
    }
    Ok(args)
}

pub(crate) fn semantic_target_action_args(
    target: &Target,
    action: &str,
    value: Option<&str>,
) -> Result<Option<Vec<OsString>>, DriverError> {
    if matches!(
        target,
        Target::Ref { .. }
            | Target::Css { .. }
            | Target::AutomationId { .. }
            | Target::VisualPoint { .. }
    ) {
        return Ok(None);
    }

    let target = serde_json::to_string(target).map_err(|error| {
        DriverError::new(
            "test.driver.web.target_invalid",
            format!("failed to encode semantic Shadow DOM target: {error}"),
        )
    })?;
    let operation = match action {
        "click" => r#"
  if ("disabled" in element && element.disabled) {
    throw new Error("A3S Test click target is disabled");
  }
  element.scrollIntoView({ behavior: "instant", block: "center", inline: "center" });
  const rect = element.getBoundingClientRect();
  return {
    handled: false,
    matched: true,
    pointer: {
      x: Math.round(rect.left + rect.width / 2),
      y: Math.round(rect.top + rect.height / 2),
    },
  };
"#
        .to_string(),
        "fill" => {
            let value = serde_json::to_string(value.unwrap_or_default()).map_err(|error| {
                DriverError::new(
                    "test.driver.web.target_invalid",
                    format!("failed to encode semantic Shadow DOM fill value: {error}"),
                )
            })?;
            format!(
                r#"
  if (!(element.getRootNode() instanceof ShadowRoot)) {{
    return {{ handled: false, matched: true }};
  }}
  if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement)) {{
    throw new Error("A3S Test fill target is not a text control");
  }}
  const prototype = element instanceof HTMLTextAreaElement
    ? HTMLTextAreaElement.prototype
    : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
  if (!setter) throw new Error("A3S Test fill target has no native value setter");
  element.focus({{ preventScroll: true }});
  setter.call(element, {value});
  element.dispatchEvent(new Event("input", {{ bubbles: true, composed: true }}));
  element.dispatchEvent(new Event("change", {{ bubbles: true, composed: true }}));
  return {{ handled: true }};
"#
            )
        }
        "check" => r#"
  if (!(element.getRootNode() instanceof ShadowRoot)) {
    return { handled: false, matched: true };
  }
  if (!(element instanceof HTMLInputElement) || !["checkbox", "radio"].includes(element.type)) {
    throw new Error("A3S Test check target is not a checkbox or radio control");
  }
  if (!element.checked) element.click();
  return { handled: true };
"#
        .to_string(),
        _ => return Ok(None),
    };

    let script = [
        "(() => {\n  const target = ",
        &target,
        ";\n",
        SEMANTIC_SHADOW_TARGET_QUERY,
        "  if (!element) return { handled: false, matched: false };\n",
        &operation,
        "})()",
    ]
    .concat();
    Ok(Some(vec!["eval".into(), script.into()]))
}

const SEMANTIC_SHADOW_TARGET_QUERY: &str = r#"
  const elements = [];
  const visit = (root) => {
    for (const element of root.children ?? []) {
      elements.push(element);
      visit(element);
      if (element.shadowRoot) visit(element.shadowRoot);
    }
  };
  visit(document);
  const text = (element) => (element.innerText || element.textContent || "").replace(/\s+/g, " ").trim();
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
  const matches = (element) => {
    if (target.type === "test_id") return element.getAttribute("data-testid") === target.value || element.getAttribute("data-test-id") === target.value;
    if (target.type === "placeholder") return element.getAttribute("placeholder") === target.value;
    if (target.type === "label") return Array.from(element.labels ?? []).map(text).some((value) => value === target.value);
    if (target.type === "role") return role(element) === target.role && labelledText(element) === target.name;
    if (target.type === "text") return target.exact ? text(element) === target.value : text(element).includes(target.value);
    return false;
  };
  const visible = (element) => {
    if (!(element instanceof HTMLElement || element instanceof SVGElement)) return false;
    let current = element;
    while (current) {
      const style = getComputedStyle(current);
      if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse" || Number(style.opacity) === 0) return false;
      if (current.hasAttribute("hidden") || current.getAttribute("aria-hidden") === "true") return false;
      const root = current.getRootNode();
      current = current.assignedSlot || current.parentElement || (root instanceof ShadowRoot ? root.host : null);
    }
    const rect = element.getBoundingClientRect();
    return element.getClientRects().length > 0 && rect.width > 0 && rect.height > 0;
  };
  const matchedElements = elements.filter((candidate) => matches(candidate) && visible(candidate));
  const element = matchedElements[0];
"#;

#[derive(Clone, Copy, Debug)]
pub(crate) enum AssertionProbe {
    State(ElementState),
    RenderedText,
    RenderedTexts,
    VisibleCount,
    Value,
    SelectedValues,
}

pub(crate) fn assertion_probe_args(
    target: &Target,
    probe: AssertionProbe,
) -> Result<Vec<OsString>, DriverError> {
    let query = match target {
        Target::Css { .. }
            if matches!(
                probe,
                AssertionProbe::RenderedText
                    | AssertionProbe::RenderedTexts
                    | AssertionProbe::VisibleCount
            ) =>
        {
            r#"
  const renderedVisible = (element) => {
    if (!(element instanceof HTMLElement || element instanceof SVGElement)) return false;
    let current = element;
    while (current) {
      const style = getComputedStyle(current);
      if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse" || Number(style.opacity) === 0) return false;
      if (current.hasAttribute("hidden")) return false;
      const root = current.getRootNode();
      current = current.parentElement || (root instanceof ShadowRoot ? root.host : null);
    }
    const rect = element.getBoundingClientRect();
    return element.getClientRects().length > 0 && rect.width > 0 && rect.height > 0;
  };
  let matchedElements;
  try {
    matchedElements = Array.from(document.querySelectorAll(target.selector)).filter(renderedVisible);
  } catch (error) {
    return { status: "invalid_target", message: String(error) };
  }
  const element = matchedElements[0];
"#
        }
        Target::Css { .. } => {
            r#"
  let matchedElements;
  try {
    matchedElements = Array.from(document.querySelectorAll(target.selector));
  } catch (error) {
    return { status: "invalid_target", message: String(error) };
  }
  const element = matchedElements[0];
"#
        }
        Target::Role { .. }
        | Target::Text { .. }
        | Target::TestId { .. }
        | Target::Label { .. }
        | Target::Placeholder { .. } => SEMANTIC_SHADOW_TARGET_QUERY,
        Target::Ref { .. } => {
            return Err(DriverError::new(
                "test.driver.web.target_unsupported",
                "current browser refs require native state queries",
            ));
        }
        Target::AutomationId { .. } | Target::VisualPoint { .. } => {
            return Err(DriverError::new(
                "test.driver.web.target_unsupported",
                "automation_id and visual_point targets are only available on GUI surfaces",
            ));
        }
    };
    let target = serde_json::to_string(target).map_err(|error| {
        DriverError::new(
            "test.driver.web.target_invalid",
            format!("failed to encode assertion target: {error}"),
        )
    })?;
    let probe = match probe {
        AssertionProbe::State(ElementState::Enabled) => "enabled",
        AssertionProbe::State(ElementState::Checked) => "checked",
        AssertionProbe::State(ElementState::Selected) => "selected",
        AssertionProbe::State(ElementState::Focused) => "focused",
        AssertionProbe::State(ElementState::FocusWithin) => "focus_within",
        AssertionProbe::State(ElementState::Expanded) => "expanded",
        AssertionProbe::State(ElementState::Pressed) => "pressed",
        AssertionProbe::State(ElementState::ReadOnly) => "readonly",
        AssertionProbe::State(ElementState::Required) => "required",
        AssertionProbe::State(ElementState::Invalid) => "invalid",
        AssertionProbe::RenderedText => "rendered_text",
        AssertionProbe::RenderedTexts => "rendered_texts",
        AssertionProbe::VisibleCount => "visible_count",
        AssertionProbe::Value => "value",
        AssertionProbe::SelectedValues => "selected_values",
    };
    let expression = format!(
        r#"(() => {{
  const target = {target};
  const A3S_ASSERTION_PROBE = "{probe}";
  const A3S_MAX_RENDERED_TEXT_ITEMS = {MAX_RENDERED_TEXT_ITEMS};
{query}
  if (A3S_ASSERTION_PROBE === "rendered_texts") {{
    if (matchedElements.length > A3S_MAX_RENDERED_TEXT_ITEMS) {{
      return {{ status: "collection_limit", count: matchedElements.length }};
    }}
    const actual = matchedElements.map((element) => {{
      const rendered = element instanceof HTMLElement ? element.innerText : element.textContent;
      return String(rendered ?? "").replace(/\s+/g, " ").trim();
    }});
    return {{ status: "ok", count: matchedElements.length, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "visible_count") {{
    return {{ status: "ok", count: matchedElements.length, actual: matchedElements.length }};
  }}
  if (matchedElements.length === 0) return {{ status: "not_found", count: 0 }};
  if (matchedElements.length > 1) return {{ status: "ambiguous", count: matchedElements.length }};

  if (A3S_ASSERTION_PROBE === "focused" || A3S_ASSERTION_PROBE === "focus_within") {{
    const deepestActiveElement = () => {{
      let active = document.activeElement;
      const seen = new Set();
      while (active && !seen.has(active)) {{
        seen.add(active);
        const shadowActive = active.shadowRoot?.activeElement;
        if (!shadowActive) break;
        active = shadowActive;
      }}
      return active;
    }};
    const composedContains = (ancestor, candidate) => {{
      let current = candidate;
      const seen = new Set();
      while (current && !seen.has(current)) {{
        if (current === ancestor) return true;
        seen.add(current);
        const root = current.getRootNode?.();
        current = current.assignedSlot || current.parentElement || (root instanceof ShadowRoot ? root.host : null);
      }}
      return false;
    }};
    const active = deepestActiveElement();
    const actual = A3S_ASSERTION_PROBE === "focused"
      ? active === element
      : composedContains(element, active);
    return {{ status: "ok", count: 1, actual }};
  }}

  const ariaBoolean = (name) => {{
    const value = element.getAttribute(name);
    if (value === "true") return true;
    if (value === "false") return false;
    return null;
  }};
  if (A3S_ASSERTION_PROBE === "rendered_text") {{
    const rendered = element instanceof HTMLElement ? element.innerText : element.textContent;
    const actual = String(rendered ?? "").replace(/\s+/g, " ").trim();
    return {{ status: "ok", count: 1, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "enabled") {{
    let current = element;
    let ariaDisabled = false;
    while (current) {{
      if (current.getAttribute?.("aria-disabled") === "true") {{
        ariaDisabled = true;
        break;
      }}
      const root = current.getRootNode?.();
      current = current.parentElement || (root instanceof ShadowRoot ? root.host : null);
    }}
    const nativeDisabled = element.matches?.(":disabled") === true
      || ("disabled" in element && element.disabled === true);
    return {{ status: "ok", count: 1, actual: !(nativeDisabled || ariaDisabled) }};
  }}
  if (A3S_ASSERTION_PROBE === "checked") {{
    if (element instanceof HTMLInputElement && ["checkbox", "radio"].includes(element.type)) {{
      return {{ status: "ok", count: 1, actual: element.checked }};
    }}
    const actual = ariaBoolean("aria-checked");
    return actual === null
      ? {{ status: "unsupported", count: 1 }}
      : {{ status: "ok", count: 1, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "selected") {{
    if (element instanceof HTMLOptionElement) {{
      return {{ status: "ok", count: 1, actual: element.selected }};
    }}
    const actual = ariaBoolean("aria-selected");
    return actual === null
      ? {{ status: "unsupported", count: 1 }}
      : {{ status: "ok", count: 1, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "expanded") {{
    if (element instanceof HTMLDetailsElement) {{
      return {{ status: "ok", count: 1, actual: element.open }};
    }}
    const actual = ariaBoolean("aria-expanded");
    return actual === null
      ? {{ status: "unsupported", count: 1 }}
      : {{ status: "ok", count: 1, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "pressed") {{
    const actual = ariaBoolean("aria-pressed");
    return actual === null
      ? {{ status: "unsupported", count: 1 }}
      : {{ status: "ok", count: 1, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "readonly") {{
    const readonlyInputTypes = new Set([
      "date", "datetime-local", "email", "month", "number", "password",
      "search", "tel", "text", "time", "url", "week",
    ]);
    if (element instanceof HTMLTextAreaElement
        || (element instanceof HTMLInputElement && readonlyInputTypes.has(element.type))) {{
      return {{ status: "ok", count: 1, actual: element.readOnly }};
    }}
    const actual = ariaBoolean("aria-readonly");
    return actual === null
      ? {{ status: "unsupported", count: 1 }}
      : {{ status: "ok", count: 1, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "required") {{
    const requiredInputTypes = new Set([
      "checkbox", "date", "datetime-local", "email", "file", "month", "number",
      "password", "radio", "search", "tel", "text", "time", "url", "week",
    ]);
    if (element instanceof HTMLSelectElement
        || element instanceof HTMLTextAreaElement
        || (element instanceof HTMLInputElement && requiredInputTypes.has(element.type))) {{
      return {{ status: "ok", count: 1, actual: element.required }};
    }}
    const actual = ariaBoolean("aria-required");
    return actual === null
      ? {{ status: "unsupported", count: 1 }}
      : {{ status: "ok", count: 1, actual }};
  }}
  if (A3S_ASSERTION_PROBE === "invalid") {{
    if ((element instanceof HTMLInputElement
          || element instanceof HTMLSelectElement
          || element instanceof HTMLTextAreaElement)
        && element.willValidate) {{
      return {{ status: "ok", count: 1, actual: !element.validity.valid }};
    }}
    const value = element.getAttribute("aria-invalid")?.trim();
    if (value === "false") return {{ status: "ok", count: 1, actual: false }};
    if (["true", "grammar", "spelling"].includes(value)) {{
      return {{ status: "ok", count: 1, actual: true }};
    }}
    return {{ status: "unsupported", count: 1 }};
  }}
  if (A3S_ASSERTION_PROBE === "value") {{
    return "value" in element
      ? {{ status: "ok", count: 1, actual: String(element.value ?? "") }}
      : {{ status: "unsupported", count: 1 }};
  }}
  if (A3S_ASSERTION_PROBE === "selected_values") {{
    return element instanceof HTMLSelectElement
      ? {{
          status: "ok",
          count: 1,
          actual: Array.from(element.selectedOptions, (option) => option.value),
        }}
      : {{ status: "unsupported", count: 1 }};
  }}
  return {{ status: "unsupported", count: 1 }};
}})()"#
    );
    Ok(vec!["eval".into(), expression.into()])
}

pub(crate) fn direct_selector(target: &Target) -> Result<&str, DriverError> {
    match target {
        Target::Ref { value } => Ok(value),
        Target::Css { selector } => Ok(selector),
        _ => Err(DriverError::new(
            "test.driver.web.target_unsupported",
            "this operation requires a ref() or css() target",
        )),
    }
}

pub(crate) fn visibility_args(target: &Target) -> Result<Vec<OsString>, DriverError> {
    match target {
        Target::Ref { value } => Ok(vec!["is".into(), "visible".into(), OsString::from(value)]),
        Target::Css { selector } => Ok(vec![
            "is".into(),
            "visible".into(),
            OsString::from(selector),
        ]),
        Target::Role { .. }
        | Target::Text { .. }
        | Target::TestId { .. }
        | Target::Label { .. }
        | Target::Placeholder { .. } => Ok(vec![
            "eval".into(),
            OsString::from(semantic_visibility_expression(target)?),
        ]),
        Target::AutomationId { .. } | Target::VisualPoint { .. } => Err(DriverError::new(
            "test.driver.web.target_unsupported",
            "automation_id and visual_point targets are only available on GUI surfaces",
        )),
    }
}

fn semantic_visibility_expression(target: &Target) -> Result<String, DriverError> {
    let target = serde_json::to_string(target).map_err(|error| {
        DriverError::new(
            "test.driver.web.target_invalid",
            format!("failed to encode semantic visibility target: {error}"),
        )
    })?;
    Ok(format!(
        r#"(() => {{
  const target = {target};
  const elements = [];
  const visit = (root) => {{
    for (const element of root.children ?? []) {{
      elements.push(element);
      visit(element);
      if (element.shadowRoot) visit(element.shadowRoot);
    }}
  }};
  visit(document);
  const text = (element) => (element.innerText || element.textContent || "").replace(/\s+/g, " ").trim();
  const labelledText = (element) => {{
    const aria = element.getAttribute("aria-label")?.trim();
    if (aria) return aria;
    const labelledBy = element.getAttribute("aria-labelledby")?.trim();
    if (labelledBy) {{
      const labels = labelledBy.split(/\s+/).map((id) => {{
        const root = element.getRootNode();
        return (typeof root.getElementById === "function" ? root.getElementById(id) : null)
          || element.ownerDocument.getElementById(id);
      }}).filter(Boolean).map(text).filter(Boolean);
      if (labels.length) return labels.join(" ");
    }}
    const labels = Array.from(element.labels ?? []).map(text).filter(Boolean);
    if (labels.length) return labels.join(" ");
    if (element instanceof HTMLImageElement && element.alt.trim()) return element.alt.trim();
    return text(element) || element.getAttribute("placeholder")?.trim() || "";
  }};
  const role = (element) => {{
    const explicit = element.getAttribute("role")?.trim().split(/\s+/)[0];
    if (explicit) return explicit;
    const tag = element.tagName.toLowerCase();
    if (tag === "input") {{
      if (["button", "submit", "reset"].includes(element.type)) return "button";
      if (element.type === "checkbox") return "checkbox";
      if (element.type === "radio") return "radio";
      if (element.type === "range") return "slider";
      if (element.type === "search") return "searchbox";
      return "textbox";
    }}
    if (tag === "a") return element.hasAttribute("href") ? "link" : "";
    if (tag === "select") return element.multiple || element.size > 1 ? "listbox" : "combobox";
    return ({{ button:"button", h1:"heading", h2:"heading", h3:"heading", h4:"heading", h5:"heading", h6:"heading", img:"img", nav:"navigation", main:"main", form:"form", option:"option", table:"table", textarea:"textbox" }})[tag] || "";
  }};
  const matches = (element) => {{
    if (target.type === "test_id") return element.getAttribute("data-testid") === target.value || element.getAttribute("data-test-id") === target.value;
    if (target.type === "placeholder") return element.getAttribute("placeholder") === target.value;
    if (target.type === "label") return Array.from(element.labels ?? []).map(text).some((value) => value === target.value);
    if (target.type === "role") return role(element) === target.role && labelledText(element) === target.name;
    if (target.type === "text") return target.exact ? text(element) === target.value : text(element).includes(target.value);
    return false;
  }};
  const visible = (element) => {{
    if (!(element instanceof HTMLElement || element instanceof SVGElement)) return false;
    const style = getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse" || Number(style.opacity) === 0) return false;
    if (element.closest("[hidden], [aria-hidden='true']")) return false;
    const rect = element.getBoundingClientRect();
    return element.getClientRects().length > 0 && rect.width > 0 && rect.height > 0;
  }};
  return elements.some((element) => matches(element) && visible(element));
}})()"#
    ))
}

pub(crate) fn wait_args(condition: &WaitCondition) -> Result<Vec<OsString>, DriverError> {
    Ok(match condition {
        WaitCondition::Load(LoadState::NetworkIdle) => {
            vec!["wait".into(), "--load".into(), "networkidle".into()]
        }
        // The admitted standalone 0.26.x runtime subscribes only to future
        // lifecycle events. A separate command issued after navigation can
        // therefore miss DOMContentLoaded and wait for its entire deadline.
        // readyState expresses the same condition as current page state.
        WaitCondition::Load(LoadState::DomContentLoaded) => vec![
            "wait".into(),
            "--fn".into(),
            "document.readyState !== 'loading'".into(),
        ],
        WaitCondition::Text(text) => vec!["wait".into(), "--text".into(), text.into()],
        WaitCondition::Regex(_) => {
            return Err(DriverError::new(
                "test.driver.web.wait_unsupported",
                "regular-expression waits are available only on terminal surfaces",
            ));
        }
        WaitCondition::Url(url) => vec!["wait".into(), "--url".into(), url.into()],
        WaitCondition::Visible(target) => match target {
            Target::Ref { value } => vec!["wait".into(), OsString::from(value)],
            Target::Css { selector } => vec!["wait".into(), OsString::from(selector)],
            Target::Role { .. }
            | Target::Text { .. }
            | Target::TestId { .. }
            | Target::Label { .. }
            | Target::Placeholder { .. } => {
                let mut args = visibility_args(target)?;
                args[0] = "wait".into();
                args.insert(1, "--fn".into());
                args
            }
            Target::AutomationId { .. } | Target::VisualPoint { .. } => {
                return Err(DriverError::new(
                    "test.driver.web.target_unsupported",
                    "automation_id and visual_point targets are only available on GUI surfaces",
                ));
            }
        },
    })
}

pub(crate) fn compact_component(value: &str, max_readable_bytes: usize) -> String {
    if value.len() <= max_readable_bytes {
        return value.to_string();
    }

    let prefix = value
        .chars()
        .take(max_readable_bytes.saturating_sub(17))
        .collect::<String>();
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{prefix}-{:016x}", hasher.finish())
}

pub(crate) fn validate_component(value: &str, field: &str) -> Result<(), DriverError> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(DriverError::new(
            "test.driver.web.session_name_invalid",
            format!("{field} must contain only ASCII letters, digits, '-' or '_'"),
        ));
    }
    Ok(())
}

pub(crate) fn scalar_string(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("data").and_then(Value::as_str))
        .or_else(|| value.pointer("/data/value").and_then(Value::as_str))
}

pub(crate) fn scalar_bool(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| value.get("visible").and_then(Value::as_bool))
        .or_else(|| value.get("enabled").and_then(Value::as_bool))
        .or_else(|| value.get("checked").and_then(Value::as_bool))
        .or_else(|| value.get("selected").and_then(Value::as_bool))
        .or_else(|| value.get("data").and_then(Value::as_bool))
        .or_else(|| value.pointer("/data/value").and_then(Value::as_bool))
        .or_else(|| value.pointer("/data/visible").and_then(Value::as_bool))
        .or_else(|| value.pointer("/data/enabled").and_then(Value::as_bool))
        .or_else(|| value.pointer("/data/checked").and_then(Value::as_bool))
        .or_else(|| value.pointer("/data/selected").and_then(Value::as_bool))
}

pub(crate) fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::Duration;

    use a3s_test_core::{LoadState, Target, WaitCondition};
    use serde_json::json;

    use super::{
        invocation, scalar_bool, semantic_target_action_args, semantic_visibility_expression,
        visibility_args, wait_args,
    };
    use crate::{AgentBrowserConfig, BrowserCommand, BrowserNetworkPolicy};

    #[test]
    fn invocation_always_overrides_browser_visibility_defaults() {
        for (headed, expected) in [(false, "false"), (true, "true")] {
            let config = AgentBrowserConfig {
                command: BrowserCommand::Standalone {
                    executable: PathBuf::from("agent-browser"),
                },
                namespace: "test".to_string(),
                headed,
                command_timeout: Duration::from_secs(5),
                idle_timeout: Duration::from_secs(30),
                microphone: Default::default(),
                network_policy: BrowserNetworkPolicy::default(),
            };
            let invocation = invocation(
                &config,
                "test",
                "visibility",
                std::path::Path::new("/tmp/a3s-test-browser"),
                vec![
                    OsString::from("open"),
                    OsString::from("https://example.test"),
                ],
            );

            assert!(
                invocation.args.windows(2).any(|arguments| {
                    arguments == [OsString::from("--headed"), OsString::from(expected)]
                }),
                "browser visibility was not explicit in {:?}",
                invocation.args
            );
            let headless_environment = invocation
                .env
                .get(&OsString::from("AGENT_BROWSER_ARGS"))
                .map(OsString::as_os_str);
            if headed {
                assert!(headless_environment.is_none());
            } else {
                assert!(headless_environment
                    .expect("headless Browser arguments")
                    .to_string_lossy()
                    .ends_with("--headless=new"));
            }
        }
    }

    #[test]
    fn dom_content_loaded_wait_checks_current_document_state() {
        assert_eq!(
            wait_args(&WaitCondition::Load(LoadState::DomContentLoaded))
                .expect("DOMContentLoaded wait"),
            [
                OsString::from("wait"),
                OsString::from("--fn"),
                OsString::from("document.readyState !== 'loading'"),
            ]
        );
    }

    #[test]
    fn network_idle_wait_uses_the_native_load_state() {
        assert_eq!(
            wait_args(&WaitCondition::Load(LoadState::NetworkIdle)).expect("network idle wait"),
            [
                OsString::from("wait"),
                OsString::from("--load"),
                OsString::from("networkidle"),
            ]
        );
    }

    #[test]
    fn scalar_bool_accepts_browser_visibility_envelopes() {
        assert_eq!(scalar_bool(&json!(true)), Some(true));
        assert_eq!(scalar_bool(&json!({ "visible": false })), Some(false));
        assert_eq!(scalar_bool(&json!({ "data": true })), Some(true));
        assert_eq!(
            scalar_bool(&json!({ "data": { "value": false } })),
            Some(false)
        );
        assert_eq!(
            scalar_bool(&json!({
                "success": true,
                "data": { "visible": true },
                "error": null
            })),
            Some(true)
        );
        assert_eq!(scalar_bool(&json!({ "data": {} })), None);
    }

    #[test]
    fn semantic_visibility_uses_a_read_only_shadow_dom_expression() {
        let target = Target::TestId {
            value: "repair-target".to_string(),
        };
        let args = visibility_args(&target).expect("semantic visibility arguments");
        assert_eq!(args[0], "eval");
        let expression = args[1].to_string_lossy();
        assert!(expression.contains(r#""type":"test_id""#));
        assert!(expression.contains("element.shadowRoot"));
        assert!(!expression.contains(".click("));
        assert!(!expression.contains(".focus("));

        let wait = wait_args(&WaitCondition::Visible(target)).expect("semantic visibility wait");
        assert_eq!(wait[0], "wait");
        assert_eq!(wait[1], "--fn");
        assert_eq!(wait[2], args[1]);
    }

    #[test]
    fn semantic_click_and_fill_prepare_bounded_shadow_dom_fallbacks() {
        let click = semantic_target_action_args(
            &Target::Role {
                role: "button".to_string(),
                name: "Send and auto-fix".to_string(),
            },
            "click",
            None,
        )
        .expect("Shadow DOM click fallback")
        .expect("semantic target fallback");
        assert_eq!(click[0], "eval");
        let click = click[1].to_string_lossy();
        assert!(click.contains(r#""type":"role""#));
        assert!(click.contains("element.shadowRoot"));
        assert!(click.contains("getBoundingClientRect"));
        assert!(click.contains(r#"behavior: "instant""#));
        assert!(click.contains("pointer:"));

        let fill = semantic_target_action_args(
            &Target::Placeholder {
                value: "Requested fix".to_string(),
            },
            "fill",
            Some("Use a \"quoted\" label"),
        )
        .expect("Shadow DOM fill fallback")
        .expect("semantic target fallback");
        let fill = fill[1].to_string_lossy();
        assert!(fill.contains("HTMLTextAreaElement.prototype"));
        assert!(fill.contains(r#"Use a \"quoted\" label"#));
        assert!(fill.contains(r#"new Event("input""#));
    }

    #[test]
    fn semantic_shadow_dom_targets_preserve_the_searchbox_role() {
        let target = Target::Role {
            role: "searchbox".to_string(),
            name: "Search component catalog".to_string(),
        };
        let fill = semantic_target_action_args(&target, "fill", Some("checkout"))
            .expect("Shadow DOM searchbox fallback")
            .expect("semantic target fallback");
        let fill = fill[1].to_string_lossy();
        assert!(fill.contains(r#"element.type === "search""#));
        assert!(fill.contains(r#"return "searchbox""#));

        let visibility = semantic_visibility_expression(&target)
            .expect("Shadow DOM searchbox visibility expression");
        assert!(visibility.contains(r#"element.type === "search""#));
        assert!(visibility.contains(r#"return "searchbox""#));
    }
}
