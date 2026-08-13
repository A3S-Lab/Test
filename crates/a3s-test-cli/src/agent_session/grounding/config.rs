use std::path::Path;
use std::time::Duration;

use a3s_acl::{Block, Value};
use a3s_test_agent::{
    GroundingOptions, GroundingProviderIdentity, HttpProviderEndpoint, ProvenanceRedactor,
};
use anyhow::{Context, Result};

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const AUTHORIZATION_ENV_PREFIX: &str = "A3S_TEST_PROVIDER_AUTHORIZATION_";

#[derive(Debug)]
pub(super) struct GroundingConfig {
    pub(super) identity: GroundingProviderIdentity,
    pub(super) endpoint: HttpProviderEndpoint,
    authorization_env: Option<String>,
    pub(super) max_cost_microusd: u64,
    pub(super) options: GroundingOptions,
}

impl GroundingConfig {
    pub(super) fn read_authorization(&self) -> Result<Option<String>> {
        self.authorization_env
            .as_ref()
            .map(|name| {
                std::env::var(name).with_context(|| {
                    format!("provider authorization environment variable {name} is not set")
                })
            })
            .transpose()
    }

    pub(super) fn redactor(&self, authorization: Option<&str>) -> Result<ProvenanceRedactor> {
        authorization
            .map(|value| ProvenanceRedactor::from_exact_secrets(authorization_secrets(value)))
            .transpose()
            .map(Option::unwrap_or_default)
            .map_err(Into::into)
    }
}

pub(super) async fn read(path: &Path) -> Result<GroundingConfig> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect grounding config {}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        anyhow::bail!("grounding config must be a regular non-symbolic-link file");
    }
    if metadata.len() == 0 || metadata.len() > MAX_CONFIG_BYTES {
        anyhow::bail!("grounding config must contain 1 to {MAX_CONFIG_BYTES} bytes");
    }
    let source = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read grounding config {}", path.display()))?;
    parse(&source)
}

fn parse(source: &str) -> Result<GroundingConfig> {
    let document = a3s_acl::parse(source).context("invalid visual grounding ACL")?;
    if document.blocks.len() != 1 || document.blocks[0].name != "visual_grounding" {
        anyhow::bail!("grounding config must contain exactly one visual_grounding block");
    }
    let root = &document.blocks[0];
    if !root.labels.is_empty() {
        anyhow::bail!("visual_grounding does not accept labels");
    }
    ensure_attributes(
        root,
        &[
            "max_cost_microusd",
            "timeout_ms",
            "max_candidates",
            "max_query_bytes",
            "max_label_bytes",
        ],
        "visual_grounding",
    )?;
    if root.blocks.len() != 1 || root.blocks[0].name != "provider" {
        anyhow::bail!("visual_grounding requires exactly one provider block");
    }
    let provider = &root.blocks[0];
    if !provider.labels.is_empty() || !provider.blocks.is_empty() {
        anyhow::bail!("visual_grounding.provider does not accept labels or nested blocks");
    }
    ensure_attributes(
        provider,
        &["name", "model", "endpoint", "authorization_env"],
        "visual_grounding.provider",
    )?;
    let authorization_env =
        optional_string(provider, "authorization_env", "visual_grounding.provider")?;
    if let Some(name) = &authorization_env {
        validate_authorization_env(name)?;
    }
    let timeout_ms = optional_u64(root, "timeout_ms", 15_000, "visual_grounding")?;
    Ok(GroundingConfig {
        identity: GroundingProviderIdentity {
            provider: required_string(provider, "name", "visual_grounding.provider")?.to_string(),
            model: required_string(provider, "model", "visual_grounding.provider")?.to_string(),
        },
        endpoint: required_string(provider, "endpoint", "visual_grounding.provider")?
            .parse()
            .map_err(anyhow::Error::new)?,
        authorization_env,
        max_cost_microusd: required_u64(root, "max_cost_microusd", "visual_grounding")?,
        options: GroundingOptions {
            timeout: Duration::from_millis(timeout_ms),
            max_candidates: optional_usize(root, "max_candidates", 32, "visual_grounding")?,
            max_query_bytes: optional_usize(root, "max_query_bytes", 4 * 1024, "visual_grounding")?,
            max_label_bytes: optional_usize(root, "max_label_bytes", 1024, "visual_grounding")?,
        },
    })
}

fn authorization_secrets(value: &str) -> Vec<&str> {
    let mut secrets = vec![value];
    if let Some((_, credential)) = value.split_once(' ') {
        if credential.len() >= 8 {
            secrets.push(credential);
        }
    }
    secrets
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
    fn parses_bounded_visual_grounding_acl() {
        let config = parse(
            r#"
visual_grounding {
  max_cost_microusd = 5000
  timeout_ms = 10000
  max_candidates = 16

  provider {
    name = "deployment"
    model = "ui-grounder"
    endpoint = "https://models.example.test/v1/locate"
    authorization_env = "A3S_TEST_PROVIDER_AUTHORIZATION_GROUNDING"
  }
}
"#,
        )
        .expect("grounding config");

        assert_eq!(config.identity.provider, "deployment");
        assert_eq!(config.options.max_candidates, 16);
        assert_eq!(config.max_cost_microusd, 5000);
    }

    #[test]
    fn rejects_unknown_grounding_blocks_and_unsafe_authorization_names() {
        let error = parse(
            r#"
visual_grounding {
  max_cost_microusd = 1
  provider {
    name = "deployment"
    model = "ui-grounder"
    endpoint = "https://models.example.test/v1/locate"
    authorization_env = "TOKEN"
  }
  fallback {}
}
"#,
        )
        .expect_err("invalid grounding config");

        assert!(error.to_string().contains("exactly one provider block"));
    }
}
