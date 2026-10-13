//! Verified-session ACL candidate.
//!
//! A later passing assertion confirms earlier passed actions. Prose success
//! with no assertion emits nothing. Observation refs, visual points, and
//! secret literals are left out.

use crate::{
    Action, Expectation, ModifierKey, SecretPurpose, Target, TestSuite,
    DEFAULT_PIXEL_THRESHOLD_MILLIS,
};

#[derive(Clone, Debug)]
pub struct RecordedStep {
    pub index: u32,
    pub action: Action,
    pub passed: bool,
}

/// Render a candidate, or `None` when no later assertion passed or nothing
/// stable remains.
#[must_use]
pub fn verified_session_candidate(
    steps: &[RecordedStep],
    secret_values: &[&str],
) -> Option<String> {
    let last_verified = steps
        .iter()
        .rev()
        .find(|step| step.passed && matches!(step.action, Action::Assert { .. }))?;
    let mut rendered = Vec::new();
    let mut saw_expect = false;
    for step in steps {
        if !step.passed || step.index > last_verified.index {
            continue;
        }
        if unstable(&step.action) || contains_secret(&step.action, secret_values) {
            continue;
        }
        if let Some(block) = render_action(step.index, &step.action) {
            if text_contains_observation_ref(&block) {
                continue;
            }
            saw_expect |=
                block.contains("\n        expect ") || block.starts_with("        expect ");
            rendered.push(block);
        }
    }
    if rendered.is_empty() || !saw_expect {
        return None;
    }
    let body = rendered.join("\n\n");
    let acl = format!(
        "suite \"verified-session\" {{\n    version = 1\n\n    scenario \"verified\" {{\n        name = \"Verified session\"\n        surface = \"web\"\n        timeout_ms = 60000\n\n{body}\n    }}\n}}\n"
    );
    let suite = TestSuite::from_acl(&acl).ok()?;
    match serde_json::to_value(&suite) {
        Ok(value) if !value_contains_secret(&value, secret_values) => Some(acl),
        _ => None,
    }
}

fn unstable(action: &Action) -> bool {
    action_targets(action)
        .iter()
        .any(|target| matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }))
}

/// `serde_json` text escapes `"`, so a value such as `p"ass` is not a
/// substring of the encoded action. Compare the decoded strings. An action
/// that cannot be inspected is treated as secret-bearing.
fn contains_secret(action: &Action, secret_values: &[&str]) -> bool {
    let Ok(value) = serde_json::to_value(action) else {
        return true;
    };
    value_contains_secret(&value, secret_values)
}

pub(crate) fn value_contains_secret(value: &serde_json::Value, secret_values: &[&str]) -> bool {
    match value {
        serde_json::Value::String(text) => secret_values
            .iter()
            .any(|secret| !secret.is_empty() && text.contains(secret)),
        serde_json::Value::Array(items) => items
            .iter()
            .any(|item| value_contains_secret(item, secret_values)),
        serde_json::Value::Object(fields) => fields
            .values()
            .any(|item| value_contains_secret(item, secret_values)),
        _ => false,
    }
}

pub(crate) fn text_contains_observation_ref(text: &str) -> bool {
    for prefix in ["@e", "@c", "@u"] {
        let mut rest = text;
        while let Some(index) = rest.find(prefix) {
            let suffix = &rest[index + prefix.len()..];
            if suffix.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return true;
            }
            rest = &rest[index + prefix.len()..];
        }
    }
    false
}

fn action_targets(action: &Action) -> Vec<&Target> {
    match action {
        Action::Click { target, .. }
        | Action::Hover { target }
        | Action::Focus { target }
        | Action::DoubleClick { target, .. }
        | Action::ContextClick { target }
        | Action::Fill { target, .. }
        | Action::Type { target, .. }
        | Action::FillSecret { target, .. }
        | Action::TypeSecret { target, .. }
        | Action::Check { target }
        | Action::Uncheck { target }
        | Action::LongPress { target, .. } => vec![target],
        Action::ScrollUntil {
            target, container, ..
        } => {
            let mut targets = vec![target];
            if let Some(container) = container {
                targets.push(container);
            }
            targets
        }
        Action::Assert { expectation } => match expectation {
            Expectation::Visible(target)
            | Expectation::InViewport(target)
            | Expectation::PointerReachable(target)
            | Expectation::RenderedText { target, .. }
            | Expectation::Value { target, .. } => vec![target],
            Expectation::Layout {
                target,
                relative_to,
                ..
            } => vec![target, relative_to],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn render_action(index: u32, action: &Action) -> Option<String> {
    let id = format!("s{index}");
    let block = match action {
        Action::Navigate { url } => format!("        navigate \"{id}\" {{\n            url = {}\n        }}", quote(url)),
        Action::Back {} => format!("        back \"{id}\" {{}}"),
        Action::Reload {} => format!("        reload \"{id}\" {{}}"),
        Action::Click { target, modifiers , ..} => {
            format!(
                "        click \"{id}\" {{\n            target = {}{}\n        }}",
                render_target(target)?,
                render_modifiers(modifiers)
            )
        }
        Action::Fill { target, value } => format!(
            "        fill \"{id}\" {{\n            target = {}\n            value = {}\n        }}",
            render_target(target)?,
            quote(value)
        ),
        Action::Type { target, value } => format!(
            "        type \"{id}\" {{\n            target = {}\n            value = {}\n        }}",
            render_target(target)?,
            quote(value)
        ),
        Action::FillSecret {
            target,
            name,
            purpose,
        } => format!(
            "        fill_secret \"{id}\" {{\n            target = {}\n            name = {}\n            purpose = \"{}\"\n        }}",
            render_target(target)?,
            quote(name),
            purpose_name(*purpose)
        ),
        Action::TypeSecret {
            target,
            name,
            purpose,
        } => format!(
            "        type_secret \"{id}\" {{\n            target = {}\n            name = {}\n            purpose = \"{}\"\n        }}",
            render_target(target)?,
            quote(name),
            purpose_name(*purpose)
        ),
        Action::ScrollUntil {
            target,
            container,
            direction,
            timeout_ms,
            ..
        } => {
            let container = match container {
                Some(container) => format!("\n            container = {}", render_target(container)?),
                None => String::new(),
            };
            format!(
                "        scroll_until \"{id}\" {{\n            target = {}{container}\n            direction = \"{}\"\n            timeout_ms = {timeout_ms}\n        }}",
                render_target(target)?,
                direction.as_str()
            )
        }
        Action::DoubleClick { target, modifiers , ..} => format!(
            "        double_click \"{id}\" {{\n            target = {}{}\n        }}",
            render_target(target)?,
            render_modifiers(modifiers)
        ),
        Action::LongPress {
            target,
            duration_ms,
        } => format!(
            "        long_press \"{id}\" {{\n            target = {}\n            duration_ms = {duration_ms}\n        }}",
            render_target(target)?
        ),
        Action::NetworkFulfill { pattern, body } => format!(
            "        network_fulfill \"{id}\" {{\n            pattern = {}\n            body = {}\n        }}",
            quote(pattern),
            quote(body)
        ),
        Action::NetworkContinue { pattern } => format!(
            "        network_continue \"{id}\" {{\n            pattern = {}\n        }}",
            quote(pattern)
        ),
        Action::NetworkAbort { pattern } => format!(
            "        network_abort \"{id}\" {{\n            pattern = {}\n        }}",
            quote(pattern)
        ),
        Action::WaitForResponse {
            pattern,
            timeout_ms,
        } => format!(
            "        wait_response \"{id}\" {{\n            pattern = {}\n            timeout_ms = {timeout_ms}\n        }}",
            quote(pattern)
        ),
        Action::Viewport {
            width,
            height,
            scale,
        } => {
            let scale = match scale {
                Some(scale) => format!("\n            scale = {scale}"),
                None => String::new(),
            };
            format!("        viewport \"{id}\" {{\n            width = {width}\n            height = {height}{scale}\n        }}")
        }
        Action::Assert { expectation } => render_expect(&id, expectation)?,
        _ => return None,
    };
    Some(block)
}

fn render_expect(id: &str, expectation: &Expectation) -> Option<String> {
    let body = match expectation {
        Expectation::TextVisible(text) => format!("text = {}", quote(text)),
        Expectation::TextContains(text) => format!("text_contains = {}", quote(text)),
        Expectation::TextMatches(pattern) => format!("text_matches = {}", quote(pattern)),
        Expectation::Url(url) => format!("url = {}", quote(url)),
        Expectation::Visible(target) => format!("visible = {}", render_target(target)?),
        Expectation::RenderedText { target, value } => format!(
            "target = {}\n            rendered_text = {}",
            render_target(target)?,
            quote(value)
        ),
        Expectation::VisibleCount { target, count } => format!(
            "target = {}\n            visible_count = {count}",
            render_target(target)?
        ),
        Expectation::InViewport(target) => format!("in_viewport = {}", render_target(target)?),
        Expectation::PointerReachable(target) => {
            format!("pointer_reachable = {}", render_target(target)?)
        }
        Expectation::Value { target, value } => format!(
            "target = {}\n            value = {}",
            render_target(target)?,
            quote(value)
        ),
        Expectation::Pixels(pixels) => {
            let mut lines = vec![format!("pixels = {}", quote(&pixels.baseline))];
            if pixels.threshold_millis != DEFAULT_PIXEL_THRESHOLD_MILLIS {
                lines.push(format!("threshold_millis = {}", pixels.threshold_millis));
            }
            if let Some(max) = pixels.max_diff_pixels {
                lines.push(format!("max_diff_pixels = {max}"));
            }
            if let Some(ratio) = pixels.max_diff_pixel_ratio_micros {
                lines.push(format!("max_diff_pixel_ratio_micros = {ratio}"));
            }
            if pixels.update {
                lines.push("update = true".to_string());
            }
            if !pixels.masks.is_empty() {
                let masks = pixels
                    .masks
                    .iter()
                    .map(|mask| {
                        quote(&format!(
                            "{},{},{},{}",
                            mask.x, mask.y, mask.width, mask.height
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("masks = [{masks}]"));
            }
            lines.join("\n            ")
        }
        _ => return None,
    };
    Some(format!(
        "        expect \"{id}\" {{\n            {body}\n        }}"
    ))
}

fn render_target(target: &Target) -> Option<String> {
    match target {
        Target::Css { selector } => Some(format!("css({})", quote(selector))),
        Target::Role { role, name } => Some(format!("role({}, {})", quote(role), quote(name))),
        Target::Text { value, exact } => {
            if *exact {
                Some(format!("text({}, true)", quote(value)))
            } else {
                Some(format!("text({})", quote(value)))
            }
        }
        Target::TestId { value } => Some(format!("testid({})", quote(value))),
        Target::Label { value } => Some(format!("label({})", quote(value))),
        Target::Placeholder { value } => Some(format!("placeholder({})", quote(value))),
        Target::AutomationId { value } => Some(format!("automation_id({})", quote(value))),
        Target::Ref { .. } | Target::VisualPoint { .. } => None,
    }
}

fn purpose_name(purpose: SecretPurpose) -> &'static str {
    match purpose {
        SecretPurpose::Text => "text",
        SecretPurpose::Password => "password",
    }
}

fn render_modifiers(modifiers: &[ModifierKey]) -> String {
    if modifiers.is_empty() {
        return String::new();
    }
    let items = modifiers
        .iter()
        .map(|modifier| match modifier {
            ModifierKey::Alt => "\"alt\"",
            ModifierKey::Control => "\"control\"",
            ModifierKey::Meta => "\"meta\"",
            ModifierKey::Shift => "\"shift\"",
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("\n            modifiers = [{items}]")
}

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}
