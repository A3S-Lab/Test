use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use a3s_acl::{Block, Value};
use a3s_test_worker::DistributedQuarantine;
use anyhow::{Context, Result};

pub(super) const AUTHORIZATION_ENV_PREFIX: &str = "A3S_TEST_WORKER_AUTHORIZATION_";
const DEFAULT_HISTORY_MAX_AGE_MS: u64 = 90 * 24 * 60 * 60 * 1_000;
const MAX_DISTRIBUTED_CONFIG_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) struct DistributedConfig {
    pub id: String,
    pub config_directory: PathBuf,
    pub input_root: PathBuf,
    pub manifest: PathBuf,
    pub additional_inputs: Vec<PathBuf>,
    pub history_root: PathBuf,
    pub history_window: usize,
    pub history_max_runs: usize,
    pub history_max_age_ms: u64,
    pub job_timeout_ms: u64,
    pub lease_ms: u64,
    pub poll_interval_ms: u64,
    pub http_timeout_ms: u64,
    pub workers: Vec<WorkerConfig>,
    pub quarantines: Vec<DistributedQuarantine>,
}

#[derive(Clone)]
pub(super) struct WorkerConfig {
    pub instance_id: String,
    pub endpoint: String,
    pub image_digest: String,
    pub inventory_digest: Option<String>,
    pub authorization_env: String,
    pub max_parallel_scenarios: u16,
}

pub(super) async fn load_config(path: &Path) -> Result<DistributedConfig> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect distributed config {}", path.display()))?;
    if !metadata.is_file()
        || is_link_like(&metadata)
        || metadata.len() == 0
        || metadata.len() > MAX_DISTRIBUTED_CONFIG_BYTES
    {
        anyhow::bail!("distributed config must be a bounded regular non-link file");
    }
    let canonical = tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("failed to resolve distributed config {}", path.display()))?;
    let source = tokio::fs::read_to_string(&canonical)
        .await
        .with_context(|| format!("failed to read distributed config {}", canonical.display()))?;
    parse_config(&source, &canonical)
}

impl std::fmt::Debug for WorkerConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerConfig")
            .field("instance_id", &self.instance_id)
            .field("endpoint", &self.endpoint)
            .field("image_digest", &self.image_digest)
            .field("inventory_digest", &self.inventory_digest)
            .field("authorization_env", &self.authorization_env)
            .field("max_parallel_scenarios", &self.max_parallel_scenarios)
            .finish()
    }
}

pub(super) fn parse_config(source: &str, config_path: &Path) -> Result<DistributedConfig> {
    let document = a3s_acl::parse(source).context("invalid distributed run ACL")?;
    if document.blocks.len() != 1 || document.blocks[0].name != "distributed_run" {
        anyhow::bail!("distributed config must contain exactly one distributed_run block");
    }
    let root = &document.blocks[0];
    let id = one_label(root, "distributed_run")?.to_string();
    validate_identifier(&id, "distributed_run label")?;
    ensure_attributes(
        root,
        &[
            "input_root",
            "manifest",
            "additional_inputs",
            "history_root",
            "history_window",
            "history_max_runs",
            "history_max_age_ms",
            "job_timeout_ms",
            "lease_ms",
            "poll_interval_ms",
            "http_timeout_ms",
        ],
        "distributed_run",
    )?;
    let config_directory = config_path
        .parent()
        .context("distributed config path does not have a parent directory")?
        .to_path_buf();
    let input_root = parse_relative_path(
        optional_string(root, "input_root", ".", "distributed_run")?,
        "distributed_run.input_root",
        true,
    )?;
    let manifest = parse_relative_path(
        required_string(root, "manifest", "distributed_run")?,
        "distributed_run.manifest",
        false,
    )?;
    let additional_inputs = optional_string_list(root, "additional_inputs", "distributed_run")?
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            parse_relative_path(
                &value,
                &format!("distributed_run.additional_inputs[{index}]"),
                false,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let history_default = format!(".a3s-test/distributed/{id}");
    let history_root = parse_relative_path(
        optional_string(root, "history_root", &history_default, "distributed_run")?,
        "distributed_run.history_root",
        false,
    )?;

    let history_window = optional_usize(root, "history_window", 20, "distributed_run")?;
    let history_max_runs = optional_usize(root, "history_max_runs", 100, "distributed_run")?;
    let history_max_age_ms = optional_u64(
        root,
        "history_max_age_ms",
        DEFAULT_HISTORY_MAX_AGE_MS,
        "distributed_run",
    )?;
    let job_timeout_ms = optional_u64(root, "job_timeout_ms", 10 * 60 * 1_000, "distributed_run")?;
    let lease_ms = optional_u64(root, "lease_ms", 60_000, "distributed_run")?;
    let poll_interval_ms = optional_u64(root, "poll_interval_ms", 250, "distributed_run")?;
    let http_timeout_ms = optional_u64(root, "http_timeout_ms", 30_000, "distributed_run")?;
    if !(1..=100).contains(&history_window)
        || !(1..=200).contains(&history_max_runs)
        || history_max_age_ms < 1_000
        || !(1_000..=24 * 60 * 60 * 1_000).contains(&job_timeout_ms)
        || !(1_000..=60 * 60 * 1_000).contains(&lease_ms)
        || lease_ms > job_timeout_ms
        || !(10..=60_000).contains(&poll_interval_ms)
        || !(1..=5 * 60 * 1_000).contains(&http_timeout_ms)
    {
        anyhow::bail!("distributed run limits are outside their reviewed bounds");
    }

    let mut workers = Vec::new();
    let mut quarantines = Vec::new();
    let mut worker_ids = BTreeSet::new();
    let mut quarantine_ids = BTreeSet::new();
    for child in &root.blocks {
        match child.name.as_str() {
            "worker" => {
                let worker = parse_worker(child)?;
                if !worker_ids.insert(worker.instance_id.clone()) {
                    anyhow::bail!("duplicate distributed worker '{}'", worker.instance_id);
                }
                workers.push(worker);
            }
            "quarantine" => {
                let quarantine = parse_quarantine(child)?;
                if !quarantine_ids.insert(quarantine.scenario_id.clone()) {
                    anyhow::bail!(
                        "duplicate quarantine for scenario '{}'",
                        quarantine.scenario_id
                    );
                }
                quarantines.push(quarantine);
            }
            name => anyhow::bail!("unsupported distributed_run block '{name}'"),
        }
    }
    if workers.is_empty() || workers.len() > 64 {
        anyhow::bail!("distributed_run requires between 1 and 64 worker blocks");
    }
    workers.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
    quarantines.sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));

    Ok(DistributedConfig {
        id,
        config_directory,
        input_root,
        manifest,
        additional_inputs,
        history_root,
        history_window,
        history_max_runs,
        history_max_age_ms,
        job_timeout_ms,
        lease_ms,
        poll_interval_ms,
        http_timeout_ms,
        workers,
        quarantines,
    })
}

fn parse_worker(block: &Block) -> Result<WorkerConfig> {
    let instance_id = one_label(block, "distributed_run.worker")?.to_string();
    validate_identifier(&instance_id, "worker instance ID")?;
    if !block.blocks.is_empty() {
        anyhow::bail!("distributed worker blocks cannot contain nested blocks");
    }
    ensure_attributes(
        block,
        &[
            "endpoint",
            "image_digest",
            "inventory_digest",
            "authorization_env",
            "max_parallel_scenarios",
        ],
        "distributed_run.worker",
    )?;
    let image_digest = required_string(block, "image_digest", "distributed_run.worker")?;
    validate_digest(image_digest, "worker image digest")?;
    let inventory_digest =
        optional_string_value(block, "inventory_digest", "distributed_run.worker")?;
    if let Some(digest) = &inventory_digest {
        validate_digest(digest, "worker inventory digest")?;
    }
    let authorization_env =
        required_string(block, "authorization_env", "distributed_run.worker")?.to_string();
    validate_authorization_env(&authorization_env)?;
    let max_parallel_scenarios =
        optional_u16(block, "max_parallel_scenarios", 1, "distributed_run.worker")?;
    if !(1..=64).contains(&max_parallel_scenarios) {
        anyhow::bail!("worker max_parallel_scenarios must be between 1 and 64");
    }
    Ok(WorkerConfig {
        instance_id,
        endpoint: required_string(block, "endpoint", "distributed_run.worker")?.to_string(),
        image_digest: image_digest.to_string(),
        inventory_digest,
        authorization_env,
        max_parallel_scenarios,
    })
}

fn parse_quarantine(block: &Block) -> Result<DistributedQuarantine> {
    let scenario_id = one_label(block, "distributed_run.quarantine")?.to_string();
    validate_identifier(&scenario_id, "quarantine scenario ID")?;
    if !block.blocks.is_empty() {
        anyhow::bail!("quarantine blocks cannot contain nested blocks");
    }
    ensure_attributes(
        block,
        &["reason", "owner", "issue", "expires_at_ms"],
        "distributed_run.quarantine",
    )?;
    Ok(DistributedQuarantine {
        scenario_id,
        reason: bounded_string(block, "reason", 1_024, "distributed_run.quarantine")?,
        owner: bounded_string(block, "owner", 256, "distributed_run.quarantine")?,
        issue: bounded_string(block, "issue", 1_024, "distributed_run.quarantine")?,
        expires_at_ms: required_u64(block, "expires_at_ms", "distributed_run.quarantine")?,
    })
}

fn validate_authorization_env(value: &str) -> Result<()> {
    if !value.starts_with(AUTHORIZATION_ENV_PREFIX)
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        anyhow::bail!(
            "worker authorization_env must start with {AUTHORIZATION_ENV_PREFIX} and use uppercase ASCII letters, digits, or underscores"
        );
    }
    Ok(())
}

fn validate_digest(value: &str, label: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        anyhow::bail!("{label} must be a canonical lowercase SHA-256 digest");
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        anyhow::bail!("{label} must be a bounded portable identifier");
    }
    Ok(())
}

fn parse_relative_path(value: &str, path: &str, allow_current: bool) -> Result<PathBuf> {
    let parsed = Path::new(value);
    if parsed.as_os_str().is_empty()
        || parsed.is_absolute()
        || parsed.components().any(|component| match component {
            Component::Normal(_) => false,
            Component::CurDir => !allow_current,
            _ => true,
        })
    {
        anyhow::bail!("{path} must be a contained relative path");
    }
    Ok(parsed.to_path_buf())
}

fn one_label<'a>(block: &'a Block, path: &str) -> Result<&'a str> {
    if block.labels.len() != 1 || block.labels[0].is_empty() {
        anyhow::bail!("{path} requires exactly one non-empty label");
    }
    Ok(&block.labels[0])
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
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{path}.{name} must be a non-empty string"))
}

fn optional_string<'a>(
    block: &'a Block,
    name: &str,
    default: &'a str,
    path: &str,
) -> Result<&'a str> {
    match block.attributes.get(name) {
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty())
            .with_context(|| format!("{path}.{name} must be a non-empty string")),
        None => Ok(default),
    }
}

fn optional_string_value(block: &Block, name: &str, path: &str) -> Result<Option<String>> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .with_context(|| format!("{path}.{name} must be a non-empty string"))
        })
        .transpose()
}

fn optional_string_list(block: &Block, name: &str, path: &str) -> Result<Vec<String>> {
    let Some(value) = block.attributes.get(name) else {
        return Ok(Vec::new());
    };
    let Value::List(values) = value else {
        anyhow::bail!("{path}.{name} must be a list of strings");
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .with_context(|| format!("{path}.{name} must contain non-empty strings"))
        })
        .collect()
}

fn bounded_string(block: &Block, name: &str, max: usize, path: &str) -> Result<String> {
    let value = required_string(block, name, path)?;
    if value.trim().is_empty() || value.len() > max {
        anyhow::bail!("{path}.{name} must be bounded and non-empty");
    }
    Ok(value.to_string())
}

fn required_u64(block: &Block, name: &str, path: &str) -> Result<u64> {
    let value = block
        .attributes
        .get(name)
        .with_context(|| format!("{path}.{name} is required"))?;
    positive_integer(value, &format!("{path}.{name}"))
}

fn optional_u64(block: &Block, name: &str, default: u64, path: &str) -> Result<u64> {
    block.attributes.get(name).map_or(Ok(default), |value| {
        positive_integer(value, &format!("{path}.{name}"))
    })
}

fn optional_u16(block: &Block, name: &str, default: u16, path: &str) -> Result<u16> {
    let value = optional_u64(block, name, u64::from(default), path)?;
    u16::try_from(value).with_context(|| format!("{path}.{name} is outside the supported range"))
}

fn optional_usize(block: &Block, name: &str, default: usize, path: &str) -> Result<usize> {
    let default = u64::try_from(default).context("default usize does not fit u64")?;
    let value = optional_u64(block, name, default, path)?;
    usize::try_from(value).with_context(|| format!("{path}.{name} is outside the supported range"))
}

fn positive_integer(value: &Value, path: &str) -> Result<u64> {
    let number = value
        .as_number()
        .with_context(|| format!("{path} must be a positive integer"))?;
    if !number.is_finite() || number < 1.0 || number.fract() != 0.0 || number > u64::MAX as f64 {
        anyhow::bail!("{path} must be a positive integer within range");
    }
    Ok(number as u64)
}

#[cfg(windows)]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::parse_config;
    use std::path::Path;

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn parses_workers_and_accountable_quarantines_without_credentials() {
        let source = format!(
            r#"distributed_run "ci" {{
  manifest = "tests/suite.acl"
  history_window = 12
  worker "runner-west" {{
    endpoint = "https://worker.example.test"
    image_digest = "{DIGEST}"
    authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_WEST"
    max_parallel_scenarios = 4
  }}
  quarantine "checkout" {{
    reason = "Known race"
    owner = "checkout-team"
    issue = "https://issues.example.test/1"
    expires_at_ms = 1800000100000
  }}
}}
"#
        );
        let config = parse_config(&source, Path::new("/workspace/distributed.acl"))
            .expect("distributed config");
        assert_eq!(config.id, "ci");
        assert_eq!(config.history_window, 12);
        assert_eq!(config.workers[0].instance_id, "runner-west");
        assert_eq!(config.workers[0].max_parallel_scenarios, 4);
        assert_eq!(config.quarantines[0].scenario_id, "checkout");
    }

    #[test]
    fn rejects_inline_credentials_and_unaccountable_quarantine() {
        let credential = format!(
            r#"distributed_run "ci" {{
  manifest = "suite.acl"
  worker "runner" {{
    endpoint = "https://user:secret@worker.example.test"
    image_digest = "{DIGEST}"
    authorization_env = "secret"
  }}
}}
"#
        );
        let error = parse_config(&credential, Path::new("/workspace/distributed.acl"))
            .expect_err("invalid authorization environment");
        assert!(error.to_string().contains("authorization_env"));

        let quarantine = format!(
            r#"distributed_run "ci" {{
  manifest = "suite.acl"
  worker "runner" {{
    endpoint = "https://worker.example.test"
    image_digest = "{DIGEST}"
    authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_RUNNER"
  }}
  quarantine "checkout" {{
    reason = "Known race"
    owner = "checkout-team"
    expires_at_ms = 1800000100000
  }}
}}
"#
        );
        let error = parse_config(&quarantine, Path::new("/workspace/distributed.acl"))
            .expect_err("missing issue");
        assert!(error.to_string().contains("issue"));
    }
}
