use std::collections::HashSet;
use std::fmt::Write as _;
use std::time::Duration;

use a3s_acl::{Block, Value};
use a3s_test_agent::{AgentGoal, AgentOptions, HttpProviderEndpoint, LlmIdentity};
use a3s_test_core::{Action, TestSuite};
use anyhow::{Context, Result};
use url::Url;

pub(super) const AUTHORIZATION_ENV_PREFIX: &str = "A3S_TEST_PROVIDER_AUTHORIZATION_";

#[derive(Clone, Debug)]
pub(super) struct AgentRunConfig {
    pub(super) id: String,
    pub(super) initial_url: Url,
    pub(super) allowed_origins: Vec<Url>,
    pub(super) allowed_domains: Vec<String>,
    pub(super) allowed_actions: Vec<a3s_test_agent::ActionKind>,
    pub(super) goal: AgentGoal,
    pub(super) options: AgentOptions,
    pub(super) provider: LlmIdentity,
    pub(super) endpoint: HttpProviderEndpoint,
    pub(super) authorization_env: Option<String>,
    pub(super) verification: TestSuite,
}

pub(super) fn parse_config(source: &str) -> Result<AgentRunConfig> {
    let document = a3s_acl::parse(source).context("invalid agent run ACL")?;
    if document.blocks.len() != 1 || document.blocks[0].name != "agent_run" {
        anyhow::bail!("agent run config must contain exactly one agent_run block");
    }
    let root = &document.blocks[0];
    let id = one_label(root, "agent_run")?.to_string();
    validate_identifier(&id, "agent_run")?;
    ensure_attributes(
        root,
        &[
            "url",
            "goal",
            "success_criteria",
            "allow_origins",
            "allow_domains",
            "allow_actions",
            "max_turns",
            "max_total_tokens",
            "max_cost_microusd",
            "max_context_bytes",
            "timeout_ms",
        ],
        "agent_run",
    )?;
    let provider_block = exactly_one_block(root, "provider", "agent_run")?;
    let verification_block = exactly_one_block(root, "verification", "agent_run")?;
    if let Some(block) = root
        .blocks
        .iter()
        .find(|block| !matches!(block.name.as_str(), "provider" | "verification"))
    {
        anyhow::bail!("unsupported agent_run block '{}'", block.name);
    }

    let initial_url = parse_web_url(required_string(root, "url", "agent_run")?, "agent_run.url")?;
    let mut allowed_origins = vec![initial_url.clone()];
    for (index, value) in optional_string_list(root, "allow_origins", "agent_run")?
        .into_iter()
        .enumerate()
    {
        allowed_origins.push(parse_web_url(
            &value,
            &format!("agent_run.allow_origins[{index}]"),
        )?);
    }
    allowed_origins.sort_by_key(|url| url.origin().ascii_serialization());
    allowed_origins.dedup_by(|left, right| left.origin() == right.origin());

    let mut allowed_domains = optional_string_list(root, "allow_domains", "agent_run")?;
    allowed_domains.sort();
    allowed_domains.dedup();

    let success_criteria = required_string_list(root, "success_criteria", "agent_run")?;
    let goal = AgentGoal {
        instruction: required_string(root, "goal", "agent_run")?.to_string(),
        success_criteria,
    };
    let timeout_ms = optional_u64(root, "timeout_ms", 120_000, "agent_run")?;
    let options = AgentOptions {
        max_turns: optional_u32(root, "max_turns", 12, "agent_run")?,
        max_total_tokens: optional_u64(root, "max_total_tokens", 64_000, "agent_run")?,
        max_cost_microusd: required_u64(root, "max_cost_microusd", "agent_run")?,
        max_context_bytes: optional_usize(root, "max_context_bytes", 512 * 1_024, "agent_run")?,
        timeout: Duration::from_millis(timeout_ms),
        provenance_redactor: Default::default(),
    };
    goal.validate().map_err(anyhow::Error::new)?;
    options.validate().map_err(anyhow::Error::new)?;
    let allowed_actions = parse_allowed_actions(root)?;
    let (provider, endpoint, authorization_env) = parse_provider(provider_block)?;
    let verification = parse_verification(verification_block, &id, timeout_ms)?;
    for step in &verification.scenarios[0].steps {
        if let Action::Assert {
            expectation: a3s_test_core::Expectation::Url(url),
        }
        | Action::Wait {
            condition: a3s_test_core::WaitCondition::Url(url),
        } = &step.action
        {
            let expected = parse_web_url(url, "agent_run.verification.url")?;
            if allowed_origins
                .iter()
                .all(|allowed| allowed.origin() != expected.origin())
            {
                anyhow::bail!(
                    "agent_run.verification URL is outside the allowed origin set: '{url}'"
                );
            }
        }
    }

    Ok(AgentRunConfig {
        id,
        initial_url,
        allowed_origins,
        allowed_domains,
        allowed_actions,
        goal,
        options,
        provider,
        endpoint,
        authorization_env,
        verification,
    })
}

fn parse_provider(block: &Block) -> Result<(LlmIdentity, HttpProviderEndpoint, Option<String>)> {
    no_labels_or_blocks(block, "agent_run.provider")?;
    ensure_attributes(
        block,
        &["name", "model", "endpoint", "authorization_env"],
        "agent_run.provider",
    )?;
    let authorization_env = optional_string(block, "authorization_env", "agent_run.provider")?;
    if let Some(name) = &authorization_env {
        validate_authorization_env(name)?;
    }
    Ok((
        LlmIdentity {
            provider: required_string(block, "name", "agent_run.provider")?.to_string(),
            model: required_string(block, "model", "agent_run.provider")?.to_string(),
        },
        required_string(block, "endpoint", "agent_run.provider")?
            .parse()
            .map_err(anyhow::Error::new)?,
        authorization_env,
    ))
}

fn parse_verification(block: &Block, run_id: &str, timeout_ms: u64) -> Result<TestSuite> {
    if !block.labels.is_empty() || !block.attributes.is_empty() {
        anyhow::bail!("agent_run.verification accepts only action blocks");
    }
    if block.blocks.is_empty() {
        anyhow::bail!("agent_run.verification requires at least one deterministic action");
    }
    let actions = block
        .blocks
        .iter()
        .map(generate_action_block)
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    let acl = format!(
        "suite \"agent-host-verification\" {{\n  scenario \"{run_id}\" {{\n    surface = \"web\"\n    timeout_ms = {timeout_ms}\n{actions}  }}\n}}\n"
    );
    let suite = TestSuite::from_acl(&acl).context("invalid agent run verification actions")?;
    for step in &suite.scenarios[0].steps {
        if !matches!(
            step.action,
            Action::Snapshot { .. }
                | Action::Wait { .. }
                | Action::Assert { .. }
                | Action::Screenshot { .. }
                | Action::Accessibility { .. }
                | Action::Console { .. }
                | Action::PageErrors { .. }
        ) {
            anyhow::bail!(
                "agent run verification action '{}' is effectful; only observation, wait, assertion, and evidence actions are allowed",
                step.id
            );
        }
    }
    if suite.scenarios[0]
        .steps
        .iter()
        .all(|step| !matches!(step.action, Action::Assert { .. }))
    {
        anyhow::bail!("agent_run.verification requires at least one deterministic expect action");
    }
    Ok(suite)
}

fn generate_action_block(block: &Block) -> Result<String> {
    let name = block.name.as_str();
    if !matches!(
        name,
        "snapshot" | "wait" | "expect" | "screenshot" | "accessibility" | "console" | "page_errors"
    ) {
        anyhow::bail!("unsupported agent_run.verification action '{name}'");
    }
    if !block.blocks.is_empty() {
        anyhow::bail!("agent_run.verification action '{name}' cannot contain nested blocks");
    }
    let label = one_label(block, &format!("agent_run.verification.{name}"))?;
    validate_identifier(label, &format!("agent_run.verification.{name}"))?;
    let mut attributes = block.attributes.iter().collect::<Vec<_>>();
    attributes.sort_by_key(|(name, _)| *name);
    let mut rendered_attributes = String::new();
    for (key, value) in attributes {
        writeln!(rendered_attributes, "      {key} = {}", render_value(value))?;
    }
    Ok(format!(
        "    {name} \"{label}\" {{\n{rendered_attributes}    }}\n"
    ))
}

fn render_value(value: &Value) -> String {
    match value {
        Value::String(value) => format!("\"{}\"", escape_acl_string(value)),
        Value::Number(value) => {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                value.to_string()
            }
        }
        Value::Bool(value) => value.to_string(),
        Value::List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Call(name, arguments) => format!(
            "{name}({})",
            arguments
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(_) | Value::Null => value.to_string(),
    }
}

fn escape_acl_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn parse_allowed_actions(block: &Block) -> Result<Vec<a3s_test_agent::ActionKind>> {
    let values = required_string_list(block, "allow_actions", "agent_run")?;
    let mut actions = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let action = match value.as_str() {
            "navigate" => a3s_test_agent::ActionKind::Navigate,
            "snapshot" => a3s_test_agent::ActionKind::Snapshot,
            "click" => a3s_test_agent::ActionKind::Click,
            "hover" => a3s_test_agent::ActionKind::Hover,
            "focus" => a3s_test_agent::ActionKind::Focus,
            "double_click" => a3s_test_agent::ActionKind::DoubleClick,
            "context_click" => a3s_test_agent::ActionKind::ContextClick,
            "fill" => a3s_test_agent::ActionKind::Fill,
            "type" => a3s_test_agent::ActionKind::Type,
            "check" => a3s_test_agent::ActionKind::Check,
            "uncheck" => a3s_test_agent::ActionKind::Uncheck,
            "select" => a3s_test_agent::ActionKind::Select,
            "drag" => a3s_test_agent::ActionKind::Drag,
            "press" => a3s_test_agent::ActionKind::Press,
            "wheel" => a3s_test_agent::ActionKind::Wheel,
            "viewport" => a3s_test_agent::ActionKind::Viewport,
            "wait" => a3s_test_agent::ActionKind::Wait,
            "assert" => a3s_test_agent::ActionKind::Assert,
            "screenshot" => a3s_test_agent::ActionKind::Screenshot,
            "tab" => a3s_test_agent::ActionKind::Tab,
            "frame" => a3s_test_agent::ActionKind::Frame,
            "dialog" => a3s_test_agent::ActionKind::Dialog,
            "upload" => a3s_test_agent::ActionKind::Upload,
            "download" => a3s_test_agent::ActionKind::Download,
            "network_route" => a3s_test_agent::ActionKind::NetworkRoute,
            "network_unroute" => a3s_test_agent::ActionKind::NetworkUnroute,
            "har" => a3s_test_agent::ActionKind::Har,
            "trace" => a3s_test_agent::ActionKind::Trace,
            "video" => a3s_test_agent::ActionKind::Video,
            "accessibility" => a3s_test_agent::ActionKind::Accessibility,
            "console" => a3s_test_agent::ActionKind::Console,
            "page_errors" => a3s_test_agent::ActionKind::PageErrors,
            _ => anyhow::bail!("unsupported agent_run.allow_actions value '{value}'"),
        };
        if !seen.insert(action) {
            anyhow::bail!("agent_run.allow_actions contains duplicate '{value}'");
        }
        actions.push(action);
    }
    Ok(actions)
}

fn parse_web_url(value: &str, path: &str) -> Result<Url> {
    let parsed = Url::parse(value).with_context(|| format!("{path} is not a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        anyhow::bail!("{path} must be an HTTP or HTTPS URL with a hostname");
    }
    Ok(parsed)
}

fn validate_authorization_env(name: &str) -> Result<()> {
    let suffix = name
        .strip_prefix(AUTHORIZATION_ENV_PREFIX)
        .context("authorization_env must use the A3S_TEST_PROVIDER_AUTHORIZATION_ prefix")?;
    if suffix.is_empty()
        || !suffix.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        anyhow::bail!("authorization_env must be an uppercase A3S Test provider variable name");
    }
    Ok(())
}

fn validate_identifier(value: &str, path: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("{path} identifier must contain 1-64 ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}

fn one_label<'a>(block: &'a Block, path: &str) -> Result<&'a str> {
    match block.labels.as_slice() {
        [label] if !label.trim().is_empty() => Ok(label),
        _ => anyhow::bail!("{path} requires exactly one non-empty label"),
    }
}

fn exactly_one_block<'a>(parent: &'a Block, name: &str, path: &str) -> Result<&'a Block> {
    let matches = parent
        .blocks
        .iter()
        .filter(|block| block.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [block] => Ok(*block),
        _ => anyhow::bail!("{path} requires exactly one {name} block"),
    }
}

fn no_labels_or_blocks(block: &Block, path: &str) -> Result<()> {
    if !block.labels.is_empty() || !block.blocks.is_empty() {
        anyhow::bail!("{path} does not accept labels or nested blocks");
    }
    Ok(())
}

fn ensure_attributes(block: &Block, allowed: &[&str], path: &str) -> Result<()> {
    if let Some(name) = block
        .attributes
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        anyhow::bail!("unsupported {path} attribute '{name}'");
    }
    Ok(())
}

fn required_string<'a>(block: &'a Block, name: &str, path: &str) -> Result<&'a str> {
    block
        .attributes
        .get(name)
        .with_context(|| format!("{path}.{name} is required"))?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("{path}.{name} must be a non-empty string"))
}

fn optional_string(block: &Block, name: &str, path: &str) -> Result<Option<String>> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .with_context(|| format!("{path}.{name} must be a non-empty string"))
        })
        .transpose()
}

fn required_string_list(block: &Block, name: &str, path: &str) -> Result<Vec<String>> {
    let values = optional_string_list(block, name, path)?;
    if values.is_empty() {
        anyhow::bail!("{path}.{name} requires at least one string");
    }
    Ok(values)
}

fn optional_string_list(block: &Block, name: &str, path: &str) -> Result<Vec<String>> {
    let Some(value) = block.attributes.get(name) else {
        return Ok(Vec::new());
    };
    let Value::List(values) = value else {
        anyhow::bail!("{path}.{name} must be a string list");
    };
    if values.len() > 64 {
        anyhow::bail!("{path}.{name} cannot contain more than 64 entries");
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty() && value.len() <= 64 * 1_024)
                .map(str::to_string)
                .with_context(|| format!("{path}.{name} must contain bounded non-empty strings"))
        })
        .collect()
}

fn required_u64(block: &Block, name: &str, path: &str) -> Result<u64> {
    let value = block
        .attributes
        .get(name)
        .with_context(|| format!("{path}.{name} is required"))?;
    u64_value(value, name, path)
}

fn optional_u64(block: &Block, name: &str, default: u64, path: &str) -> Result<u64> {
    block
        .attributes
        .get(name)
        .map(|value| u64_value(value, name, path))
        .unwrap_or(Ok(default))
}

fn optional_u32(block: &Block, name: &str, default: u32, path: &str) -> Result<u32> {
    let value = optional_u64(block, name, u64::from(default), path)?;
    u32::try_from(value).with_context(|| format!("{path}.{name} exceeds u32"))
}

fn optional_usize(block: &Block, name: &str, default: usize, path: &str) -> Result<usize> {
    let value = optional_u64(block, name, default as u64, path)?;
    usize::try_from(value).with_context(|| format!("{path}.{name} exceeds the platform range"))
}

fn u64_value(value: &Value, name: &str, path: &str) -> Result<u64> {
    let number = value
        .as_number()
        .with_context(|| format!("{path}.{name} must be an integer"))?;
    if !number.is_finite() || number.fract() != 0.0 || number < 0.0 || number > u64::MAX as f64 {
        anyhow::bail!("{path}.{name} is outside the supported integer range");
    }
    Ok(number as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_bounded_agent_host_config_and_read_only_verification() {
        let config = parse_config(
            r##"
agent_run "checkout" {
  url = "https://shop.example.test/checkout"
  goal = "Complete checkout"
  success_criteria = ["Confirmation is visible"]
  allow_origins = ["https://auth.example.test"]
  allow_domains = ["cdn.example.test"]
  allow_actions = ["click", "fill", "wait", "assert"]
  max_turns = 8
  max_total_tokens = 20000
  max_cost_microusd = 50000

  provider {
    name = "deployment"
    model = "planner"
    endpoint = "https://models.example.test/v1/plan"
    authorization_env = "A3S_TEST_PROVIDER_AUTHORIZATION_DEPLOYMENT"
  }

  verification {
    expect "confirmation" { text = "Order confirmed" }
    screenshot "final" { path = "final.png" }
  }
}
"##,
        )
        .expect("agent run config");

        assert_eq!(config.id, "checkout");
        assert_eq!(config.allowed_origins.len(), 2);
        assert_eq!(config.allowed_domains, ["cdn.example.test"]);
        assert_eq!(config.allowed_actions.len(), 4);
        assert_eq!(config.verification.scenarios[0].steps.len(), 2);
    }

    #[test]
    fn rejects_effectful_verification_actions() {
        let error = parse_config(
            r##"
agent_run "unsafe" {
  url = "https://example.test"
  goal = "Reach the final state"
  success_criteria = ["Done"]
  allow_actions = ["click"]
  max_cost_microusd = 1
  provider {
    name = "deployment"
    model = "planner"
    endpoint = "https://models.example.test/v1/plan"
  }
  verification { click "mutate" { target = css("#danger") } }
}
"##,
        )
        .expect_err("effectful verification");

        assert!(error
            .to_string()
            .contains("unsupported agent_run.verification action"));
    }

    #[test]
    fn rejects_cross_origin_verification_urls() {
        let error = parse_config(
            r#"
agent_run "unsafe-url" {
  url = "https://example.test"
  goal = "Reach the final state"
  success_criteria = ["Done"]
  allow_actions = ["click"]
  max_cost_microusd = 1
  provider {
    name = "deployment"
    model = "planner"
    endpoint = "https://models.example.test/v1/plan"
  }
  verification { expect "url" { url = "https://outside.test/done" } }
}
"#,
        )
        .expect_err("cross-origin verification URL");

        assert!(error.to_string().contains("outside the allowed origin set"));
    }

    #[test]
    fn rejects_agent_limits_during_acl_admission() {
        let error = parse_config(
            r#"
agent_run "unbounded" {
  url = "https://example.test"
  goal = "Reach the final state"
  success_criteria = ["Done"]
  allow_actions = ["click"]
  max_turns = 257
  max_cost_microusd = 1
  provider {
    name = "deployment"
    model = "planner"
    endpoint = "https://models.example.test/v1/plan"
  }
  verification { expect "done" { text = "Done" } }
}
"#,
        )
        .expect_err("unbounded turns");

        assert!(error.to_string().contains("maximum turns must be between"));
    }
}
