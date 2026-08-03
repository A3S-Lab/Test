use std::collections::BTreeMap;
use std::ffi::OsString;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use a3s_test_core::{DriverError, LoadState, Target, WaitCondition};
use serde_json::Value;

use crate::{AgentBrowserConfig, CommandInvocation};

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
    ]);
    if config.headed {
        args.push(OsString::from("--headed"));
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
    if let Some((policy_name, policy_value)) = config
        .command
        .allowed_domains_environment(&config.network_policy)
    {
        env.insert(policy_name, policy_value);
    }

    CommandInvocation {
        program: config.command.program().to_path_buf(),
        args,
        env,
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
        WaitCondition::Url(url) => vec!["wait".into(), "--url".into(), url.into()],
        WaitCondition::Visible(target) => {
            vec!["wait".into(), OsString::from(direct_selector(target)?)]
        }
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
        .or_else(|| value.get("data").and_then(Value::as_bool))
        .or_else(|| value.pointer("/data/value").and_then(Value::as_bool))
}

pub(crate) fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use a3s_test_core::{LoadState, WaitCondition};

    use super::wait_args;

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
}
