use std::collections::HashSet;

mod conditions;
mod targets;

use a3s_acl::{Block, Value};
use url::Url;

use self::conditions::{
    condition_count_error, parse_assertion_stability, parse_expectation, parse_wait,
};
use self::targets::{optional_target, parse_target, required_target, target_argument};

use crate::{
    Action, AssertionMode, CaptureOperation, DialogOperation, FrameTarget, ModifierKey,
    NetworkRoute, SpecError, Surface, TabOperation, TestScenario, TestStep, TestSuite,
    VideoOperation, WaitMode,
};

const DEFAULT_SCENARIO_TIMEOUT_MS: u64 = 60_000;

impl TestSuite {
    pub fn from_acl(source: &str) -> Result<Self, SpecError> {
        let document = a3s_acl::parse(source).map_err(|error| {
            SpecError::new(
                "test.spec.syntax",
                "document",
                format!("invalid ACL document: {error}"),
            )
        })?;

        if document.blocks.len() != 1 || document.blocks[0].name != "suite" {
            return Err(SpecError::new(
                "test.spec.suite_required",
                "document",
                "the document must contain exactly one suite block",
            ));
        }

        parse_suite(&document.blocks[0])
    }

    /// Parses the bounded, read-only Web subset admitted for repair proof.
    ///
    /// A repair proof owns one fresh browser session and therefore must contain
    /// exactly one Web scenario. Every navigation remains on the exact origin
    /// of the finding, and steps that can mutate application or network state
    /// are rejected before the proof browser is opened.
    pub fn from_repair_acl(source: &str, finding_url: &str) -> Result<Self, SpecError> {
        let suite = Self::from_acl(source)?;
        admit_repair_suite(&suite, finding_url)?;
        Ok(suite)
    }
}

fn admit_repair_suite(suite: &TestSuite, finding_url: &str) -> Result<(), SpecError> {
    if suite.scenarios.len() != 1 {
        return Err(SpecError::new(
            "test.spec.repair_scenario_count",
            "suite.scenarios",
            "a repair ACL must contain exactly one scenario",
        ));
    }
    let finding_url = Url::parse(finding_url).map_err(|error| {
        SpecError::new(
            "test.spec.repair_origin_invalid",
            "finding.url",
            format!("the repair finding URL is invalid: {error}"),
        )
    })?;
    if !matches!(finding_url.scheme(), "http" | "https") {
        return Err(SpecError::new(
            "test.spec.repair_origin_invalid",
            "finding.url",
            "repair ACL proof requires an HTTP or HTTPS finding URL",
        ));
    }
    let scenario = &suite.scenarios[0];
    if scenario.surface != Surface::Web {
        return Err(SpecError::new(
            "test.spec.repair_surface",
            format!("suite.{}.scenario.{}.surface", suite.name, scenario.id),
            "a repair ACL scenario must use the Web surface",
        ));
    }

    let finding_origin = finding_url.origin();
    let mut navigation_count = 0_usize;
    for step in &scenario.steps {
        let path = format!(
            "suite.{}.scenario.{}.step.{}",
            suite.name, scenario.id, step.id
        );
        match &step.action {
            Action::Navigate { url } => {
                navigation_count += 1;
                let url = Url::parse(url).map_err(|error| {
                    SpecError::new(
                        "test.spec.repair_origin_invalid",
                        &path,
                        format!("repair navigation URL is invalid: {error}"),
                    )
                })?;
                if url.origin() != finding_origin {
                    return Err(SpecError::new(
                        "test.spec.repair_origin_denied",
                        path,
                        "repair navigation must remain on the finding's exact origin",
                    ));
                }
            }
            Action::Snapshot { .. }
            | Action::Wait { .. }
            | Action::Assert { .. }
            | Action::Screenshot { .. }
            | Action::Viewport { .. }
            | Action::Accessibility { .. }
            | Action::Console { .. }
            | Action::PageErrors { .. } => {}
            _ => {
                return Err(SpecError::new(
                    "test.spec.repair_action_denied",
                    path,
                    "repair ACL proof only admits navigation, observation, assertion, viewport, and evidence steps",
                ));
            }
        }
    }
    if navigation_count != 1 {
        return Err(SpecError::new(
            "test.spec.repair_navigation_count",
            format!("suite.{}.scenario.{}.steps", suite.name, scenario.id),
            "a repair ACL must contain exactly one navigation step",
        ));
    }
    Ok(())
}

fn parse_suite(block: &Block) -> Result<TestSuite, SpecError> {
    let name = one_label(block, "suite")?.to_string();
    let path = format!("suite.{name}");
    ensure_attributes(block, &["version"], &path)?;
    let version = optional_integer(block, "version", 1, &path)?;

    let mut scenarios = Vec::new();
    let mut ids = HashSet::new();
    for child in &block.blocks {
        if child.name != "scenario" {
            return Err(SpecError::new(
                "test.spec.block_unknown",
                format!("{path}.{}", child.name),
                "only scenario blocks are allowed inside a suite",
            ));
        }
        let scenario = parse_scenario(child, &path)?;
        if !ids.insert(scenario.id.clone()) {
            return Err(SpecError::new(
                "test.spec.scenario_duplicate",
                format!("{path}.scenario.{}", scenario.id),
                "scenario identifiers must be unique",
            ));
        }
        scenarios.push(scenario);
    }

    if scenarios.is_empty() {
        return Err(SpecError::new(
            "test.spec.scenario_required",
            path,
            "a suite must contain at least one scenario",
        ));
    }

    Ok(TestSuite {
        name,
        version: u32::try_from(version).map_err(|_| {
            SpecError::new(
                "test.spec.number_range",
                "suite.version",
                "version is outside the supported range",
            )
        })?,
        scenarios,
    })
}

fn parse_scenario(block: &Block, suite_path: &str) -> Result<TestScenario, SpecError> {
    let id = one_label(block, "scenario")?.to_string();
    let path = format!("{suite_path}.scenario.{id}");
    validate_identifier(&id, &path)?;
    ensure_attributes(block, &["name", "surface", "timeout_ms"], &path)?;

    let name = optional_string(block, "name", &id, &path)?;
    let surface = match required_string(block, "surface", &path)? {
        "web" => Surface::Web,
        "gui" => Surface::Gui,
        "tui" => Surface::Tui,
        _ => {
            return Err(SpecError::new(
                "test.spec.surface_unknown",
                format!("{path}.surface"),
                "surface must be web, gui, or tui",
            ));
        }
    };
    let timeout_ms = optional_integer(block, "timeout_ms", DEFAULT_SCENARIO_TIMEOUT_MS, &path)?;

    let mut steps = Vec::new();
    let mut ids = HashSet::new();
    for action in &block.blocks {
        let step = parse_step(action, &path)?;
        if !ids.insert(step.id.clone()) {
            return Err(SpecError::new(
                "test.spec.step_duplicate",
                format!("{path}.{}", step.id),
                "step identifiers must be unique inside a scenario",
            ));
        }
        steps.push(step);
    }

    if steps.is_empty() {
        return Err(SpecError::new(
            "test.spec.step_required",
            path,
            "a scenario must contain at least one step",
        ));
    }

    Ok(TestScenario {
        id,
        name,
        surface,
        timeout_ms,
        steps,
    })
}

fn parse_step(block: &Block, scenario_path: &str) -> Result<TestStep, SpecError> {
    let action_path = format!("{scenario_path}.{}", block.name);
    let id = one_label(block, &action_path)?.to_string();
    let path = format!("{action_path}.{id}");
    validate_identifier(&id, &path)?;
    if !block.blocks.is_empty() {
        return Err(SpecError::new(
            "test.spec.action_nested",
            path,
            "action blocks cannot contain nested blocks",
        ));
    }

    let mut stability = None;
    let mut assertion_mode = AssertionMode::Positive;
    let mut wait_mode = WaitMode::Positive;
    let action = match block.name.as_str() {
        "navigate" => {
            ensure_attributes(block, &["url"], &path)?;
            Action::Navigate {
                url: required_string(block, "url", &path)?.to_string(),
            }
        }
        "snapshot" => {
            ensure_attributes(block, &["interactive"], &path)?;
            Action::Snapshot {
                interactive: optional_bool(block, "interactive", true, &path)?,
            }
        }
        "click" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::Click {
                target: required_target(block, "target", &path)?,
            }
        }
        "hover" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::Hover {
                target: required_target(block, "target", &path)?,
            }
        }
        "focus" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::Focus {
                target: required_target(block, "target", &path)?,
            }
        }
        "double_click" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::DoubleClick {
                target: required_target(block, "target", &path)?,
            }
        }
        "context_click" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::ContextClick {
                target: required_target(block, "target", &path)?,
            }
        }
        "fill" => {
            ensure_attributes(block, &["target", "value"], &path)?;
            Action::Fill {
                target: required_target(block, "target", &path)?,
                value: required_string(block, "value", &path)?.to_string(),
            }
        }
        "type" => {
            ensure_attributes(block, &["target", "value"], &path)?;
            Action::Type {
                target: required_target(block, "target", &path)?,
                value: required_string(block, "value", &path)?.to_string(),
            }
        }
        "insert_text" => {
            ensure_attributes(block, &["value"], &path)?;
            Action::InsertText {
                value: required_string(block, "value", &path)?.to_string(),
            }
        }
        "check" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::Check {
                target: required_target(block, "target", &path)?,
            }
        }
        "uncheck" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::Uncheck {
                target: required_target(block, "target", &path)?,
            }
        }
        "select" => {
            ensure_attributes(block, &["target", "values"], &path)?;
            Action::Select {
                target: required_target(block, "target", &path)?,
                values: required_string_list(block, "values", &path)?,
            }
        }
        "drag" => {
            ensure_attributes(block, &["source", "target"], &path)?;
            Action::Drag {
                source: required_target(block, "source", &path)?,
                target: required_target(block, "target", &path)?,
            }
        }
        "press" => {
            ensure_attributes(block, &["key"], &path)?;
            Action::Press {
                key: required_string(block, "key", &path)?.to_string(),
            }
        }
        "terminal_paste" => {
            ensure_attributes(block, &["text"], &path)?;
            Action::TerminalPaste {
                text: required_string(block, "text", &path)?.to_string(),
            }
        }
        "terminal_resize" => {
            ensure_attributes(block, &["columns", "rows"], &path)?;
            Action::TerminalResize {
                columns: required_u16(block, "columns", &path)?,
                rows: required_u16(block, "rows", &path)?,
            }
        }
        "terminal_recording" => {
            ensure_attributes(block, &["path"], &path)?;
            Action::TerminalRecording {
                path: required_string(block, "path", &path)?.to_string(),
            }
        }
        "wheel" => {
            ensure_attributes(block, &["target", "delta_x", "delta_y", "modifiers"], &path)?;
            let delta_x = optional_signed_integer(block, "delta_x", 0, &path)?;
            let delta_y = required_signed_integer(block, "delta_y", &path)?;
            if delta_x == 0 && delta_y == 0 {
                return Err(SpecError::new(
                    "test.spec.wheel_delta_required",
                    path,
                    "wheel requires a non-zero delta_x or delta_y",
                ));
            }
            Action::Wheel {
                target: optional_target(block, "target", &path)?,
                delta_x,
                delta_y,
                modifiers: optional_modifiers(block, "modifiers", &path)?,
            }
        }
        "viewport" => {
            ensure_attributes(block, &["width", "height", "scale"], &path)?;
            Action::Viewport {
                width: required_u32(block, "width", &path)?,
                height: required_u32(block, "height", &path)?,
                scale: optional_u32(block, "scale", &path)?,
            }
        }
        "wait" => {
            ensure_attributes(
                block,
                &["load", "text", "regex", "url", "visible", "hidden"],
                &path,
            )?;
            let (condition, parsed_mode) = parse_wait(block, &path)?;
            wait_mode = parsed_mode;
            Action::Wait { condition }
        }
        "expect" => {
            ensure_attributes(
                block,
                &[
                    "text",
                    "url",
                    "visible",
                    "hidden",
                    "in_viewport",
                    "pointer_reachable",
                    "target",
                    "rendered_text",
                    "rendered_texts",
                    "visible_count",
                    "value",
                    "enabled",
                    "disabled",
                    "checked",
                    "unchecked",
                    "selected",
                    "unselected",
                    "focused",
                    "unfocused",
                    "focus_within",
                    "focus_outside",
                    "expanded",
                    "collapsed",
                    "pressed",
                    "unpressed",
                    "readonly",
                    "writable",
                    "required",
                    "optional",
                    "invalid",
                    "valid",
                    "selected_values",
                    "layout",
                    "relative_to",
                    "tolerance_px",
                    "stable_for_ms",
                    "sample_interval_ms",
                ],
                &path,
            )?;
            stability = parse_assertion_stability(block, &path)?;
            let (expectation, parsed_mode) = parse_expectation(block, &path)?;
            assertion_mode = parsed_mode;
            Action::Assert { expectation }
        }
        "screenshot" => {
            ensure_attributes(block, &["path"], &path)?;
            Action::Screenshot {
                path: required_string(block, "path", &path)?.to_string(),
            }
        }
        "tab" => {
            ensure_attributes(block, &["operation", "url", "label", "tab"], &path)?;
            Action::Tab {
                operation: parse_tab_operation(block, &path)?,
            }
        }
        "frame" => {
            ensure_attributes(block, &["target"], &path)?;
            Action::Frame {
                target: required_frame_target(block, "target", &path)?,
            }
        }
        "dialog" => {
            ensure_attributes(block, &["operation", "text"], &path)?;
            Action::Dialog {
                operation: parse_dialog_operation(block, &path)?,
            }
        }
        "upload" => {
            ensure_attributes(block, &["target", "paths"], &path)?;
            Action::Upload {
                target: required_target(block, "target", &path)?,
                paths: required_string_list(block, "paths", &path)?,
            }
        }
        "download" => {
            ensure_attributes(block, &["target", "path"], &path)?;
            Action::Download {
                target: required_target(block, "target", &path)?,
                path: required_string(block, "path", &path)?.to_string(),
            }
        }
        "network_route" => {
            ensure_attributes(block, &["pattern", "abort", "body"], &path)?;
            Action::NetworkRoute {
                pattern: required_string(block, "pattern", &path)?.to_string(),
                route: parse_network_route(block, &path)?,
            }
        }
        "network_unroute" => {
            ensure_attributes(block, &["pattern"], &path)?;
            Action::NetworkUnroute {
                pattern: optional_string_attribute(block, "pattern", &path)?,
            }
        }
        "har" => {
            ensure_attributes(block, &["operation", "path"], &path)?;
            Action::Har {
                operation: parse_capture_operation(block, &path)?,
            }
        }
        "trace" => {
            ensure_attributes(block, &["operation", "path"], &path)?;
            Action::Trace {
                operation: parse_capture_operation(block, &path)?,
            }
        }
        "video" => {
            ensure_attributes(block, &["operation", "path", "url"], &path)?;
            Action::Video {
                operation: parse_video_operation(block, &path)?,
            }
        }
        "accessibility" => {
            ensure_attributes(block, &["path", "interactive"], &path)?;
            Action::Accessibility {
                path: required_string(block, "path", &path)?.to_string(),
                interactive: optional_bool(block, "interactive", false, &path)?,
            }
        }
        "console" => {
            ensure_attributes(block, &["path", "clear"], &path)?;
            Action::Console {
                path: required_string(block, "path", &path)?.to_string(),
                clear: optional_bool(block, "clear", false, &path)?,
            }
        }
        "page_errors" => {
            ensure_attributes(block, &["path", "clear"], &path)?;
            Action::PageErrors {
                path: required_string(block, "path", &path)?.to_string(),
                clear: optional_bool(block, "clear", false, &path)?,
            }
        }
        "verify_contract" => {
            ensure_attributes(block, &["contract", "variant", "state"], &path)?;
            Action::VerifyContract {
                contract: required_string(block, "contract", &path)?.to_string(),
                variant: required_string(block, "variant", &path)?.to_string(),
                state: required_string(block, "state", &path)?.to_string(),
            }
        }
        _ => {
            return Err(SpecError::new(
                "test.spec.action_unknown",
                action_path,
                "unsupported action block",
            ));
        }
    };

    Ok(TestStep {
        id,
        action,
        stability,
        assertion_mode,
        wait_mode,
    })
}

fn parse_tab_operation(block: &Block, path: &str) -> Result<TabOperation, SpecError> {
    match required_string(block, "operation", path)? {
        "list" => {
            ensure_absent(block, &["url", "label", "tab"], path)?;
            Ok(TabOperation::List)
        }
        "new" => {
            ensure_absent(block, &["tab"], path)?;
            Ok(TabOperation::New {
                url: optional_string_attribute(block, "url", path)?,
                label: optional_string_attribute(block, "label", path)?,
            })
        }
        "switch" => {
            ensure_absent(block, &["url", "label"], path)?;
            Ok(TabOperation::Switch {
                tab: required_string(block, "tab", path)?.to_string(),
            })
        }
        "close" => {
            ensure_absent(block, &["url", "label"], path)?;
            Ok(TabOperation::Close {
                tab: optional_string_attribute(block, "tab", path)?,
            })
        }
        _ => Err(SpecError::new(
            "test.spec.operation_unknown",
            format!("{path}.operation"),
            "tab operation must be list, new, switch, or close",
        )),
    }
}

fn required_frame_target(block: &Block, name: &str, path: &str) -> Result<FrameTarget, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required frame target is missing",
        )
    })?;
    let target_path = format!("{path}.{name}");
    let Value::Call(function, arguments) = value else {
        return Err(type_error(
            target_path,
            "frame target must use main(), ref(), or css()",
        ));
    };
    match (function.as_str(), arguments.as_slice()) {
        ("main", []) => Ok(FrameTarget::Main),
        ("ref" | "css", [value]) => {
            Ok(FrameTarget::Selector(target_argument(value, &target_path)?))
        }
        _ => Err(SpecError::new(
            "test.spec.frame_target_invalid",
            target_path,
            "frame target must use main(), ref(), or css() with the expected argument count",
        )),
    }
}

fn parse_dialog_operation(block: &Block, path: &str) -> Result<DialogOperation, SpecError> {
    match required_string(block, "operation", path)? {
        "status" => {
            ensure_absent(block, &["text"], path)?;
            Ok(DialogOperation::Status)
        }
        "accept" => Ok(DialogOperation::Accept {
            text: optional_string_attribute(block, "text", path)?,
        }),
        "dismiss" => {
            ensure_absent(block, &["text"], path)?;
            Ok(DialogOperation::Dismiss)
        }
        _ => Err(SpecError::new(
            "test.spec.operation_unknown",
            format!("{path}.operation"),
            "dialog operation must be status, accept, or dismiss",
        )),
    }
}

fn parse_network_route(block: &Block, path: &str) -> Result<NetworkRoute, SpecError> {
    let count = ["abort", "body"]
        .iter()
        .filter(|name| block.attributes.contains_key(**name))
        .count();
    if count != 1 {
        return Err(condition_count_error(path, count));
    }

    if block.attributes.contains_key("abort") {
        if !optional_bool(block, "abort", false, path)? {
            return Err(SpecError::new(
                "test.spec.route_invalid",
                format!("{path}.abort"),
                "abort must be true when configured",
            ));
        }
        return Ok(NetworkRoute::Abort);
    }
    Ok(NetworkRoute::Body(
        required_string(block, "body", path)?.to_string(),
    ))
}

fn parse_capture_operation(block: &Block, path: &str) -> Result<CaptureOperation, SpecError> {
    match required_string(block, "operation", path)? {
        "start" => {
            ensure_absent(block, &["path"], path)?;
            Ok(CaptureOperation::Start)
        }
        "stop" => Ok(CaptureOperation::Stop {
            path: required_string(block, "path", path)?.to_string(),
        }),
        _ => Err(SpecError::new(
            "test.spec.operation_unknown",
            format!("{path}.operation"),
            "capture operation must be start or stop",
        )),
    }
}

fn parse_video_operation(block: &Block, path: &str) -> Result<VideoOperation, SpecError> {
    match required_string(block, "operation", path)? {
        "start" => Ok(VideoOperation::Start {
            path: required_string(block, "path", path)?.to_string(),
            url: optional_string_attribute(block, "url", path)?,
        }),
        "stop" => {
            ensure_absent(block, &["path", "url"], path)?;
            Ok(VideoOperation::Stop)
        }
        _ => Err(SpecError::new(
            "test.spec.operation_unknown",
            format!("{path}.operation"),
            "video operation must be start or stop",
        )),
    }
}

fn one_label<'a>(block: &'a Block, path: &str) -> Result<&'a str, SpecError> {
    if block.labels.len() != 1 || block.labels[0].is_empty() {
        return Err(SpecError::new(
            "test.spec.label_required",
            path,
            "block requires exactly one non-empty label",
        ));
    }
    Ok(&block.labels[0])
}

fn validate_identifier(value: &str, path: &str) -> Result<(), SpecError> {
    let valid = value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if !valid {
        return Err(SpecError::new(
            "test.spec.identifier_invalid",
            path,
            "identifier must contain only ASCII letters, digits, '-' or '_' and be at most 64 bytes",
        ));
    }
    Ok(())
}

fn ensure_attributes(block: &Block, allowed: &[&str], path: &str) -> Result<(), SpecError> {
    for name in block.attributes.keys() {
        if !allowed.contains(&name.as_str()) {
            return Err(SpecError::new(
                "test.spec.attribute_unknown",
                format!("{path}.{name}"),
                "unsupported attribute",
            ));
        }
    }
    Ok(())
}

fn ensure_absent(block: &Block, names: &[&str], path: &str) -> Result<(), SpecError> {
    if let Some(name) = names
        .iter()
        .find(|name| block.attributes.contains_key(**name))
    {
        return Err(SpecError::new(
            "test.spec.attribute_unexpected",
            format!("{path}.{name}"),
            "attribute is not valid for this operation",
        ));
    }
    Ok(())
}

fn required_string<'a>(block: &'a Block, name: &str, path: &str) -> Result<&'a str, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required attribute is missing",
        )
    })?;
    value
        .as_str()
        .ok_or_else(|| type_error(format!("{path}.{name}"), "attribute must be a string"))
}

fn optional_string_attribute(
    block: &Block,
    name: &str,
    path: &str,
) -> Result<Option<String>, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| type_error(format!("{path}.{name}"), "attribute must be a string"))
        })
        .transpose()
}

fn required_string_list(block: &Block, name: &str, path: &str) -> Result<Vec<String>, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required string list is missing",
        )
    })?;
    let Value::List(values) = value else {
        return Err(type_error(
            format!("{path}.{name}"),
            "attribute must be a non-empty list of strings",
        ));
    };
    if values.is_empty() {
        return Err(SpecError::new(
            "test.spec.list_empty",
            format!("{path}.{name}"),
            "list must contain at least one item",
        ));
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                type_error(
                    format!("{path}.{name}[{index}]"),
                    "list items must be strings",
                )
            })
        })
        .collect()
}

fn optional_modifiers(
    block: &Block,
    name: &str,
    path: &str,
) -> Result<Vec<ModifierKey>, SpecError> {
    let Some(value) = block.attributes.get(name) else {
        return Ok(Vec::new());
    };
    let Value::List(values) = value else {
        return Err(type_error(
            format!("{path}.{name}"),
            "modifiers must be a list of alt, control, meta, or shift",
        ));
    };

    let mut seen = HashSet::new();
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let item_path = format!("{path}.{name}[{index}]");
            let modifier = match value.as_str() {
                Some("alt") => ModifierKey::Alt,
                Some("control") => ModifierKey::Control,
                Some("meta") => ModifierKey::Meta,
                Some("shift") => ModifierKey::Shift,
                Some(_) => {
                    return Err(SpecError::new(
                        "test.spec.modifier_unknown",
                        item_path,
                        "modifier must be alt, control, meta, or shift",
                    ));
                }
                None => {
                    return Err(type_error(item_path, "modifier list items must be strings"));
                }
            };
            if !seen.insert(modifier) {
                return Err(SpecError::new(
                    "test.spec.modifier_duplicate",
                    item_path,
                    "modifiers cannot contain duplicates",
                ));
            }
            Ok(modifier)
        })
        .collect()
}

fn optional_string(
    block: &Block,
    name: &str,
    default: &str,
    path: &str,
) -> Result<String, SpecError> {
    match block.attributes.get(name) {
        Some(value) => value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| type_error(format!("{path}.{name}"), "attribute must be a string")),
        None => Ok(default.to_string()),
    }
}

fn value_string(value: &Value, path: impl Into<String>) -> Result<String, SpecError> {
    let path = path.into();
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| type_error(path, "condition must be a string"))
}

fn optional_bool(block: &Block, name: &str, default: bool, path: &str) -> Result<bool, SpecError> {
    match block.attributes.get(name) {
        Some(value) => value
            .as_bool()
            .ok_or_else(|| type_error(format!("{path}.{name}"), "attribute must be a boolean")),
        None => Ok(default),
    }
}

fn optional_integer(block: &Block, name: &str, default: u64, path: &str) -> Result<u64, SpecError> {
    let Some(value) = block.attributes.get(name) else {
        return Ok(default);
    };
    positive_integer(value, &format!("{path}.{name}"))
}

fn required_u32(block: &Block, name: &str, path: &str) -> Result<u32, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required positive integer is missing",
        )
    })?;
    let value = positive_integer(value, &format!("{path}.{name}"))?;
    u32::try_from(value).map_err(|_| {
        SpecError::new(
            "test.spec.number_range",
            format!("{path}.{name}"),
            "integer is outside the supported range",
        )
    })
}

fn required_nonnegative_u32(block: &Block, name: &str, path: &str) -> Result<u32, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required non-negative integer is missing",
        )
    })?;
    let number = value
        .as_number()
        .ok_or_else(|| type_error(format!("{path}.{name}"), "attribute must be an integer"))?;
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > f64::from(u32::MAX)
    {
        return Err(SpecError::new(
            "test.spec.number_range",
            format!("{path}.{name}"),
            "integer must be non-negative and within the unsigned 32-bit range",
        ));
    }
    Ok(number as u32)
}

fn required_u16(block: &Block, name: &str, path: &str) -> Result<u16, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required positive integer is missing",
        )
    })?;
    let value = positive_integer(value, &format!("{path}.{name}"))?;
    u16::try_from(value).map_err(|_| {
        SpecError::new(
            "test.spec.number_range",
            format!("{path}.{name}"),
            "integer is outside the supported range",
        )
    })
}

fn optional_u32(block: &Block, name: &str, path: &str) -> Result<Option<u32>, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| {
            let value = positive_integer(value, &format!("{path}.{name}"))?;
            u32::try_from(value).map_err(|_| {
                SpecError::new(
                    "test.spec.number_range",
                    format!("{path}.{name}"),
                    "integer is outside the supported range",
                )
            })
        })
        .transpose()
}

fn positive_integer(value: &Value, path: &str) -> Result<u64, SpecError> {
    let Some(number) = value.as_number() else {
        return Err(type_error(path, "attribute must be an integer"));
    };
    if !number.is_finite() || number < 1.0 || number.fract() != 0.0 || number > u64::MAX as f64 {
        return Err(SpecError::new(
            "test.spec.number_range",
            path,
            "integer must be positive and within range",
        ));
    }
    Ok(number as u64)
}

fn required_signed_integer(block: &Block, name: &str, path: &str) -> Result<i32, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required integer is missing",
        )
    })?;
    signed_integer(value, &format!("{path}.{name}"))
}

fn optional_signed_integer(
    block: &Block,
    name: &str,
    default: i32,
    path: &str,
) -> Result<i32, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| signed_integer(value, &format!("{path}.{name}")))
        .unwrap_or(Ok(default))
}

fn signed_integer(value: &Value, path: &str) -> Result<i32, SpecError> {
    let Some(number) = value.as_number() else {
        return Err(type_error(path, "attribute must be an integer"));
    };
    if !number.is_finite()
        || number.fract() != 0.0
        || number < f64::from(i32::MIN)
        || number > f64::from(i32::MAX)
    {
        return Err(SpecError::new(
            "test.spec.number_range",
            path,
            "integer is outside the supported range",
        ));
    }
    Ok(number as i32)
}

fn type_error(path: impl Into<String>, message: impl Into<String>) -> SpecError {
    SpecError::new("test.spec.type", path, message)
}
