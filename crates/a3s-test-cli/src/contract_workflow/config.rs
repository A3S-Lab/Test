use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use a3s_acl::{Block, Value};
use a3s_test_agent::{
    ContractGenerationOptions, ContractGenerationProviderIdentity, ContractSource,
    ContractSourceKind, HttpProviderEndpoint,
};
use a3s_test_core::{ContractContext, ContractMode};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use super::storage::{canonical_regular_file, read_bounded};

pub(super) const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
const AUTHORIZATION_ENV_PREFIX: &str = "A3S_TEST_PROVIDER_AUTHORIZATION_";

#[derive(Debug)]
pub(super) struct GenerationConfig {
    pub(super) contract_name: String,
    pub(super) context: ContractContext,
    pub(super) sources: Vec<ContractSource>,
    pub(super) provider: ContractGenerationProviderIdentity,
    pub(super) endpoint: HttpProviderEndpoint,
    pub(super) authorization_env: Option<String>,
    pub(super) max_cost_microusd: u64,
    pub(super) options: ContractGenerationOptions,
}

pub(super) async fn parse_generation_config(
    source: &str,
    config_root: &Path,
) -> Result<GenerationConfig> {
    let document = a3s_acl::parse(source).context("invalid contract workflow ACL")?;
    if document.blocks.len() != 1 || document.blocks[0].name != "contract_generation" {
        anyhow::bail!("contract workflow must contain exactly one contract_generation block");
    }
    let root = &document.blocks[0];
    let contract_name = one_label(root, "contract_generation")?.to_string();
    ensure_attributes(
        root,
        &[
            "max_cost_microusd",
            "timeout_ms",
            "max_sources",
            "max_source_bytes",
            "max_candidates",
            "max_elements",
            "max_string_bytes",
        ],
        "contract_generation",
    )?;
    let context_block = exactly_one_block(root, "context", "contract_generation")?;
    let provider_block = exactly_one_block(root, "provider", "contract_generation")?;
    let source_blocks = root
        .blocks
        .iter()
        .filter(|block| block.name == "source")
        .collect::<Vec<_>>();
    if source_blocks.is_empty() {
        anyhow::bail!("contract_generation requires at least one source block");
    }
    if let Some(block) = root
        .blocks
        .iter()
        .find(|block| !matches!(block.name.as_str(), "context" | "provider" | "source"))
    {
        anyhow::bail!("unsupported contract_generation block '{}'", block.name);
    }

    let context = parse_context(context_block)?;
    let (provider, endpoint, authorization_env) = parse_provider(provider_block)?;
    let mut sources = Vec::with_capacity(source_blocks.len());
    let mut source_ids = HashSet::new();
    for block in source_blocks {
        let source = parse_source(block, config_root).await?;
        if !source_ids.insert(source.id.clone()) {
            anyhow::bail!("source identifier '{}' is duplicated", source.id);
        }
        sources.push(source);
    }
    let timeout_ms = optional_u64(root, "timeout_ms", 30_000, "contract_generation")?;
    let options = ContractGenerationOptions {
        timeout: Duration::from_millis(timeout_ms),
        max_sources: optional_usize(root, "max_sources", 8, "contract_generation")?,
        max_source_bytes: optional_usize(
            root,
            "max_source_bytes",
            8 * 1024 * 1024,
            "contract_generation",
        )?,
        max_candidates: optional_usize(root, "max_candidates", 64, "contract_generation")?,
        max_elements: optional_usize(root, "max_elements", 1024, "contract_generation")?,
        max_string_bytes: optional_usize(
            root,
            "max_string_bytes",
            16 * 1024,
            "contract_generation",
        )?,
    };
    Ok(GenerationConfig {
        contract_name,
        context,
        sources,
        provider,
        endpoint,
        authorization_env,
        max_cost_microusd: required_u64(root, "max_cost_microusd", "contract_generation")?,
        options,
    })
}

fn parse_context(block: &Block) -> Result<ContractContext> {
    no_labels_or_blocks(block, "contract_generation.context")?;
    ensure_attributes(
        block,
        &["mode", "audience", "primary_outcome"],
        "contract_generation.context",
    )?;
    let mode = match required_string(block, "mode", "contract_generation.context")? {
        "persuade" => ContractMode::Persuade,
        "operate" => ContractMode::Operate,
        "read" => ContractMode::Read,
        "experience" => ContractMode::Experience,
        value => anyhow::bail!("unsupported contract context mode '{value}'"),
    };
    Ok(ContractContext {
        mode,
        audience: required_string_list(block, "audience", "contract_generation.context")?,
        primary_outcome: required_string(block, "primary_outcome", "contract_generation.context")?
            .to_string(),
    })
}

fn parse_provider(
    block: &Block,
) -> Result<(
    ContractGenerationProviderIdentity,
    HttpProviderEndpoint,
    Option<String>,
)> {
    no_labels_or_blocks(block, "contract_generation.provider")?;
    ensure_attributes(
        block,
        &["name", "model", "endpoint", "authorization_env"],
        "contract_generation.provider",
    )?;
    let authorization_env =
        optional_string(block, "authorization_env", "contract_generation.provider")?;
    if let Some(name) = &authorization_env {
        validate_authorization_env(name)?;
    }
    Ok((
        ContractGenerationProviderIdentity {
            provider: required_string(block, "name", "contract_generation.provider")?.to_string(),
            model: required_string(block, "model", "contract_generation.provider")?.to_string(),
        },
        required_string(block, "endpoint", "contract_generation.provider")?
            .parse()
            .map_err(anyhow::Error::new)?,
        authorization_env,
    ))
}

async fn parse_source(block: &Block, config_root: &Path) -> Result<ContractSource> {
    let id = one_label(block, "contract_generation.source")?.to_string();
    no_nested_blocks(block, "contract_generation.source")?;
    ensure_attributes(
        block,
        &["kind", "path", "uri", "media_type", "width", "height"],
        "contract_generation.source",
    )?;
    let kind = match required_string(block, "kind", "contract_generation.source")? {
        "prd" => ContractSourceKind::Prd,
        "design" => ContractSourceKind::Design,
        value => anyhow::bail!("unsupported contract source kind '{value}'"),
    };
    let relative_path = required_string(block, "path", "contract_generation.source")?;
    let path = resolve_contained_source(config_root, relative_path).await?;
    let bytes = read_bounded(&path, MAX_SOURCE_BYTES, "contract source").await?;
    let uri = optional_string(block, "uri", "contract_generation.source")?
        .unwrap_or_else(|| relative_path.to_string());
    if !valid_relative_uri(&uri) {
        anyhow::bail!("contract source uri must be a contained relative path");
    }
    Ok(ContractSource {
        id,
        kind,
        uri,
        path: path.to_string_lossy().into_owned(),
        sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
        media_type: optional_string(block, "media_type", "contract_generation.source")?,
        width: optional_u32(block, "width", "contract_generation.source")?,
        height: optional_u32(block, "height", "contract_generation.source")?,
    })
}

fn valid_relative_uri(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

async fn resolve_contained_source(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    if relative.trim().is_empty()
        || relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        anyhow::bail!("contract source path must stay inside the config directory");
    }
    let requested = root.join(relative_path);
    let canonical = canonical_regular_file(&requested, "contract source").await?;
    if !canonical.starts_with(root) {
        anyhow::bail!("contract source path must stay inside the config directory");
    }
    Ok(canonical)
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

pub(super) fn one_label<'a>(block: &'a Block, path: &str) -> Result<&'a str> {
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

pub(super) fn no_labels_or_blocks(block: &Block, path: &str) -> Result<()> {
    if !block.labels.is_empty() {
        anyhow::bail!("{path} does not accept labels");
    }
    no_nested_blocks(block, path)
}

pub(super) fn no_nested_blocks(block: &Block, path: &str) -> Result<()> {
    if !block.blocks.is_empty() {
        anyhow::bail!("{path} does not accept nested blocks");
    }
    Ok(())
}

pub(super) fn ensure_attributes(block: &Block, allowed: &[&str], path: &str) -> Result<()> {
    if let Some(name) = block
        .attributes
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        anyhow::bail!("unsupported {path} attribute '{name}'");
    }
    Ok(())
}

pub(super) fn required_string<'a>(block: &'a Block, name: &str, path: &str) -> Result<&'a str> {
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
    let value = block
        .attributes
        .get(name)
        .with_context(|| format!("{path}.{name} is required"))?;
    let Value::List(values) = value else {
        anyhow::bail!("{path}.{name} must be a string list");
    };
    if values.is_empty() {
        anyhow::bail!("{path}.{name} must not be empty");
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .with_context(|| format!("{path}.{name} must contain non-empty strings"))
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

fn optional_usize(block: &Block, name: &str, default: usize, path: &str) -> Result<usize> {
    let value = optional_u64(block, name, default as u64, path)?;
    usize::try_from(value).with_context(|| format!("{path}.{name} exceeds the platform range"))
}

fn optional_u32(block: &Block, name: &str, path: &str) -> Result<Option<u32>> {
    block
        .attributes
        .get(name)
        .map(|value| {
            u64_value(value, name, path).and_then(|value| {
                u32::try_from(value).with_context(|| format!("{path}.{name} exceeds u32"))
            })
        })
        .transpose()
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
