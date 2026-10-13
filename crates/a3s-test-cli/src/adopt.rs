//! Write a caller-named ACL suite from a finished agent session report.
//!
//! The report already carries the verified-session candidate. This command
//! re-validates that text and writes it only when the caller names the file.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use a3s_test_core::adopted_suite_acl;
use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;

#[derive(Debug, Args)]
pub(crate) struct AdoptArgs {
    /// Finished agent session report JSON.
    #[arg(long)]
    report: PathBuf,
    /// Caller-named ACL suite file to write.
    #[arg(long)]
    output: PathBuf,
    /// Suite identifier to write. Omit to keep the candidate's suite name.
    #[arg(long)]
    name: Option<String>,
    /// Replace an existing suite file.
    #[arg(long)]
    replace: bool,
    /// Environment variable whose value must not appear in the adopted suite.
    #[arg(long = "secret-env")]
    secret_env: Vec<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Deserialize)]
struct FinishedSessionReport {
    #[serde(default)]
    acl_candidate: Option<String>,
}

pub(crate) async fn execute(args: AdoptArgs) -> Result<ExitCode> {
    let report_bytes = tokio::fs::read(&args.report).await.with_context(|| {
        format!(
            "test.adopt.report_unreadable: failed to read {}",
            args.report.display()
        )
    })?;
    let report: FinishedSessionReport =
        serde_json::from_slice(&report_bytes).with_context(|| {
            format!(
                "test.adopt.report_invalid: {} is not a finished session report",
                args.report.display()
            )
        })?;
    let candidate = report.acl_candidate.unwrap_or_default();
    if candidate.trim().is_empty() {
        anyhow::bail!(
            "test.adopt.prose_only: a prose-only success has no verified-session candidate"
        );
    }
    let secret_values = secret_values(&args.secret_env)?;
    let secret_refs = secret_values.iter().map(String::as_str).collect::<Vec<_>>();
    let acl = adopted_suite_acl(&candidate, args.name.as_deref(), &secret_refs)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    write_suite(&args.output, &acl, args.replace).await?;
    let suite = a3s_test_core::TestSuite::from_acl(&acl)
        .map_err(|error| anyhow::anyhow!("test.adopt.candidate_invalid: {error}"))?;
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "suite": suite.name,
                "output": args.output,
                "scenarios": suite.scenarios.len(),
            })
        );
    } else {
        println!(
            "Adopted suite \"{}\" to {}",
            suite.name,
            args.output.display()
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn secret_values(names: &[String]) -> Result<Vec<String>> {
    let mut values = Vec::new();
    for name in names {
        if !admit_env_name(name) {
            anyhow::bail!("test.adopt.secret_env: {name} is not an environment variable name");
        }
        let Ok(value) = std::env::var(name) else {
            continue;
        };
        if value.is_empty() || values.iter().any(|existing| existing == &value) {
            continue;
        }
        values.push(value);
    }
    Ok(values)
}

fn admit_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && name.len() <= 128
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

async fn write_suite(path: &Path, acl: &str, replace: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "test.adopt.output_unwritable: failed to create {}",
                    parent.display()
                )
            })?;
        }
    }
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!(
                "test.adopt.output_link: refusing to write through {}",
                path.display()
            );
        }
        Ok(_) if !replace => {
            anyhow::bail!(
                "test.adopt.output_exists: {} already exists; pass --replace to overwrite it",
                path.display()
            );
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "test.adopt.output_unwritable: failed to inspect {}",
                    path.display()
                )
            });
        }
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("suite.acl");
    let temporary = path.with_file_name(format!(".{file_name}.partial"));
    if let Err(error) = tokio::fs::write(&temporary, acl).await {
        return Err(error).with_context(|| {
            format!(
                "test.adopt.output_unwritable: failed to write {}",
                temporary.display()
            )
        });
    }
    if replace && tokio::fs::symlink_metadata(path).await.is_ok() {
        if let Err(error) = tokio::fs::remove_file(path).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(error).with_context(|| {
                format!(
                    "test.adopt.output_unwritable: failed to replace {}",
                    path.display()
                )
            });
        }
    }
    if let Err(error) = tokio::fs::rename(&temporary, path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error).with_context(|| {
            format!(
                "test.adopt.output_unwritable: failed to publish {}",
                path.display()
            )
        });
    }
    Ok(())
}
