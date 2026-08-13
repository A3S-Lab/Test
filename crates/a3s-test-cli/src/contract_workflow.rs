use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use a3s_test_agent::{
    ContractGenerationService, ContractWorkflowAdmission, ContractWorkflowArtifact,
    HttpContractGenerationProvider, HttpProviderConfig,
};
use a3s_test_core::SurfaceContractDraft;
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use tokio_util::sync::CancellationToken;

use self::config::{parse_generation_config, MAX_CONFIG_BYTES};
use self::review::parse_review;
use self::storage::{
    canonical_regular_file, publish_review_outputs, read_bounded, write_atomic, MAX_REVIEW_BYTES,
    MAX_WORKFLOW_BYTES,
};

mod config;
mod review;
mod storage;

#[derive(Debug, Args)]
pub(crate) struct ContractArgs {
    #[command(subcommand)]
    command: ContractCommand,
}

#[derive(Debug, Subcommand)]
enum ContractCommand {
    /// Call a deployment-owned provider and save a non-authoritative draft.
    Generate(GenerateArgs),
    /// Apply an explicit human review and emit the canonical ACL contract.
    Review(ReviewArgs),
}

#[derive(Debug, Args)]
struct GenerateArgs {
    /// ACL source-to-contract workflow configuration.
    #[arg(long)]
    config: PathBuf,
    /// Destination for the generated workflow JSON artifact.
    #[arg(long)]
    output: PathBuf,
    /// Replace an existing output file atomically.
    #[arg(long)]
    force: bool,
    /// Emit a machine-readable command result.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ReviewArgs {
    /// Generated workflow JSON artifact.
    #[arg(long)]
    draft: PathBuf,
    /// ACL review decisions authored by a human reviewer.
    #[arg(long)]
    review: PathBuf,
    /// Destination for the canonical reviewed Surface Contract ACL.
    #[arg(long)]
    output: PathBuf,
    /// Destination for the complete reviewed workflow audit artifact.
    #[arg(long)]
    audit: PathBuf,
    /// Replace existing ACL and audit files atomically.
    #[arg(long)]
    force: bool,
    /// Emit a machine-readable command result.
    #[arg(long)]
    json: bool,
}

pub(crate) async fn execute(args: ContractArgs) -> Result<ExitCode> {
    match args.command {
        ContractCommand::Generate(args) => generate(args).await,
        ContractCommand::Review(args) => review(args).await,
    }
}

async fn generate(args: GenerateArgs) -> Result<ExitCode> {
    storage::ensure_output_target(&args.output, args.force, "contract workflow draft")?;
    let config_path = canonical_regular_file(&args.config, "contract workflow config").await?;
    let config_root = config_path
        .parent()
        .context("contract workflow config does not have a parent directory")?;
    let source = read_bounded(&config_path, MAX_CONFIG_BYTES, "contract workflow config").await?;
    let source = std::str::from_utf8(&source).context("contract workflow config must be UTF-8")?;
    let config = parse_generation_config(source, config_root).await?;

    let mut transport = HttpProviderConfig::new(config.endpoint.clone())
        .with_timeout(config.options.timeout)
        .map_err(anyhow::Error::new)?;
    if let Some(name) = &config.authorization_env {
        let authorization = std::env::var(name).with_context(|| {
            format!("provider authorization environment variable '{name}' is unavailable")
        })?;
        transport = transport
            .with_authorization(authorization)
            .map_err(anyhow::Error::new)?;
    }
    let provider = Arc::new(
        HttpContractGenerationProvider::new(config.provider.clone(), transport)
            .map_err(anyhow::Error::new)?,
    );
    let admission = ContractWorkflowAdmission::new(
        config.sources.clone(),
        config.max_cost_microusd,
        &config.options,
    )
    .map_err(anyhow::Error::new)?;
    let service = ContractGenerationService::new(provider, config.options.clone())
        .map_err(anyhow::Error::new)?;
    let cancellation = CancellationToken::new();
    let signal = install_interrupt_handler(cancellation.clone());
    let generated = service
        .generate(
            config.contract_name,
            config.context,
            config.sources,
            config.max_cost_microusd,
            cancellation,
        )
        .await
        .map_err(anyhow::Error::new);
    signal.abort();
    let _ = signal.await;
    let generated = generated?;
    let workflow =
        ContractWorkflowArtifact::generated(generated, admission).map_err(anyhow::Error::new)?;
    let bytes = serde_json::to_vec_pretty(&workflow).context("failed to encode contract draft")?;
    write_atomic(&args.output, &bytes, args.force, "contract workflow draft").await?;

    print_result(
        args.json,
        serde_json::json!({
            "stage": "generated",
            "output": args.output,
            "contract": workflow.generated.name,
            "candidates": candidate_count(&workflow),
            "conflicts": workflow.generated.conflicts.len(),
            "unresolved_decisions": workflow.generated.unresolved_decisions.len(),
        }),
        format!(
            "Generated review-gated contract draft: {} ({} candidates, {} conflicts)",
            args.output.display(),
            candidate_count(&workflow),
            workflow.generated.conflicts.len()
        ),
    )?;
    Ok(ExitCode::SUCCESS)
}

async fn review(args: ReviewArgs) -> Result<ExitCode> {
    storage::ensure_distinct_outputs(&args.output, &args.audit)?;
    storage::ensure_output_target(&args.output, args.force, "reviewed contract")?;
    storage::ensure_output_target(&args.audit, args.force, "reviewed contract audit")?;
    let draft_path = canonical_regular_file(&args.draft, "contract workflow draft").await?;
    let draft_bytes =
        read_bounded(&draft_path, MAX_WORKFLOW_BYTES, "contract workflow draft").await?;
    let workflow: ContractWorkflowArtifact = serde_json::from_slice(&draft_bytes)
        .with_context(|| format!("invalid contract workflow draft {}", draft_path.display()))?;
    workflow
        .validate_evidence()
        .await
        .map_err(anyhow::Error::new)?;
    let review_path = canonical_regular_file(&args.review, "contract review").await?;
    let review_bytes = read_bounded(&review_path, MAX_REVIEW_BYTES, "contract review").await?;
    let review_source =
        std::str::from_utf8(&review_bytes).context("contract review must be UTF-8")?;
    let review = parse_review(review_source)?;
    let audit = workflow.apply_review(review).map_err(anyhow::Error::new)?;
    let contract_acl = audit
        .contract_acl
        .as_deref()
        .context("reviewed workflow did not contain canonical ACL")?
        .to_string();
    SurfaceContractDraft::from_acl(&contract_acl)
        .context("reviewed contract did not round-trip through ACL")?
        .admit()
        .context("reviewed contract did not pass local admission")?;
    audit
        .validate_evidence()
        .await
        .map_err(anyhow::Error::new)?;
    let audit_bytes = serde_json::to_vec_pretty(&audit)
        .context("failed to encode reviewed contract workflow audit")?;

    publish_review_outputs(
        &args.output,
        contract_acl.as_bytes(),
        &args.audit,
        &audit_bytes,
        args.force,
    )
    .await?;
    print_result(
        args.json,
        serde_json::json!({
            "stage": "reviewed",
            "output": args.output,
            "audit": args.audit,
            "contract": audit.generated.name,
        }),
        format!(
            "Published reviewed Surface Contract: {} (audit: {})",
            args.output.display(),
            args.audit.display()
        ),
    )?;
    Ok(ExitCode::SUCCESS)
}

fn install_interrupt_handler(cancellation: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancellation.cancel();
        }
    })
}

fn candidate_count(workflow: &ContractWorkflowArtifact) -> usize {
    workflow
        .generated
        .candidates
        .iter()
        .map(|candidate| {
            candidate
                .variants
                .iter()
                .map(|variant| variant.elements.len())
                .sum::<usize>()
        })
        .sum()
}

fn print_result(json: bool, value: serde_json::Value, human: String) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{human}");
    }
    Ok(())
}
