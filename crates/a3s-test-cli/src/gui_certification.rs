use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{Surface, SurfaceObservation};
use a3s_test_driver_gui::{GuiCertificationMatrix, GuiExecutionProfile};
use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use super::{gui_driver, validate_timeout, GuiProfileArg, GuiRunArgs};

#[derive(Debug, Args)]
pub(super) struct GuiCertificationArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct GuiCertifyArgs {
    #[command(flatten)]
    gui: GuiRunArgs,
    /// Per-command CUA deadline.
    #[arg(long, default_value_t = 30_000)]
    command_timeout_ms: u64,
    /// Bounded cleanup deadline for the certification session.
    #[arg(long, default_value_t = 10_000)]
    cleanup_timeout_ms: u64,
    /// Artifact root for the certification observation.
    #[arg(long, default_value = ".a3s-test/gui-certification")]
    artifacts_root: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

pub(super) fn print_matrix(args: GuiCertificationArgs) -> Result<ExitCode> {
    let matrix = GuiCertificationMatrix::locked().map_err(anyhow::Error::new)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&matrix)?);
    } else {
        println!(
            "CUA {} @ {} (MCP {})",
            matrix.cua_driver_version(),
            matrix.cua_revision(),
            matrix.mcp_protocol()
        );
        for profile in matrix.profiles() {
            println!(
                "  {}: {} [semantic={}, window_vision={}, lifecycle={}]",
                profile.id(),
                profile.status().as_str(),
                profile.semantic(),
                profile.window_vision(),
                profile.lifecycle()
            );
            if let Some(reason) = profile.reason() {
                println!("    {reason}");
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

pub(super) async fn certify(args: GuiCertifyArgs) -> Result<ExitCode> {
    validate_timeout(args.command_timeout_ms, "command timeout")?;
    validate_timeout(args.cleanup_timeout_ms, "cleanup timeout")?;
    let selected_profile = args.gui.gui_profile;
    let driver = gui_driver(&args.gui, Duration::from_millis(args.command_timeout_ms)).await?;
    let execution_profile = driver.execution_profile().map_err(anyhow::Error::new)?;
    let artifacts_root = if args.artifacts_root.is_absolute() {
        args.artifacts_root
    } else {
        std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(args.artifacts_root)
    };
    let manager = a3s_test_session::AgentSessionManager::new(
        vec![Arc::new(driver)],
        a3s_test_session::SessionManagerOptions {
            artifacts_root: artifacts_root.clone(),
            cleanup_timeout: Duration::from_millis(args.cleanup_timeout_ms),
            max_sessions: 1,
        },
    )
    .map_err(anyhow::Error::new)?;
    let session_id = "gui-certification";
    let started = manager
        .start(a3s_test_session::StartSessionRequest {
            session: session_id.to_string(),
            surface: Surface::Gui,
            goal: "Certify the locked GUI execution profile".to_string(),
            success_criteria: vec![
                "A semantic window observation is returned".to_string(),
                "The exact owned session is closed within the cleanup deadline".to_string(),
            ],
            auto_resolve_repairs: false,
        })
        .await;
    if let Err(error) = started {
        return emit(
            GuiCertificationRun::failed(
                execution_profile,
                artifacts_root,
                CertificationFailure::from_session_error(&error),
                None,
            ),
            args.json,
        );
    }

    let observed = manager.observe(session_id).await;
    let observation = match observed {
        Ok(observation) => observation,
        Err(error) => {
            let cleanup = manager.abort(session_id).await.ok();
            return emit(
                GuiCertificationRun::failed(
                    execution_profile,
                    artifacts_root,
                    CertificationFailure::from_session_error(&error),
                    cleanup,
                ),
                args.json,
            );
        }
    };
    let certification = certify_observation(&observation.observation, selected_profile);
    let finish_status = if certification.is_ok() {
        a3s_test_session::SessionFinishStatus::Passed
    } else {
        a3s_test_session::SessionFinishStatus::Failed
    };
    let cleanup = manager
        .finish(a3s_test_session::FinishSessionRequest {
            session: session_id.to_string(),
            status: finish_status,
            summary: "GUI profile certification completed".to_string(),
        })
        .await;
    let cleanup = match cleanup {
        Ok(cleanup) => cleanup,
        Err(error) => {
            return emit(
                GuiCertificationRun::failed(
                    execution_profile,
                    artifacts_root,
                    CertificationFailure::from_session_error(&error),
                    None,
                ),
                args.json,
            );
        }
    };
    let certification = match certification {
        Ok(certification) => certification,
        Err(failure) => {
            return emit(
                GuiCertificationRun::failed(
                    execution_profile,
                    artifacts_root,
                    failure,
                    Some(cleanup),
                ),
                args.json,
            );
        }
    };
    if let Some(error) = &cleanup.cleanup_error {
        let failure = CertificationFailure {
            code: error.code.clone(),
            message: error.message.clone(),
            retryable: error.retryable,
        };
        return emit(
            GuiCertificationRun::failed(execution_profile, artifacts_root, failure, Some(cleanup)),
            args.json,
        );
    }

    emit(
        GuiCertificationRun::passed(execution_profile, artifacts_root, certification, cleanup),
        args.json,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum GuiCertificationRunStatus {
    Passed,
    Failed,
}

#[derive(Debug, Serialize)]
struct CertifiedObservation {
    summary: String,
    semantic_element_count: usize,
    visual_evidence_count: usize,
}

#[derive(Debug, Serialize)]
struct CertificationFailure {
    code: String,
    message: String,
    retryable: bool,
}

impl CertificationFailure {
    fn from_session_error(error: &a3s_test_session::SessionError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.message().to_string(),
            retryable: error.retryable(),
        }
    }

    fn observation(message: impl Into<String>) -> Self {
        Self {
            code: "test.driver.gui.certification_observation_invalid".to_string(),
            message: message.into(),
            retryable: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct GuiCertificationRun {
    status: GuiCertificationRunStatus,
    execution_profile: GuiExecutionProfile,
    artifacts_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation: Option<CertifiedObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cleanup: Option<a3s_test_session::SessionFinished>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<CertificationFailure>,
}

impl GuiCertificationRun {
    fn passed(
        execution_profile: GuiExecutionProfile,
        artifacts_root: PathBuf,
        observation: CertifiedObservation,
        cleanup: a3s_test_session::SessionFinished,
    ) -> Self {
        Self {
            status: GuiCertificationRunStatus::Passed,
            execution_profile,
            artifacts_root: artifacts_root.to_string_lossy().into_owned(),
            observation: Some(observation),
            cleanup: Some(cleanup),
            failure: None,
        }
    }

    fn failed(
        execution_profile: GuiExecutionProfile,
        artifacts_root: PathBuf,
        failure: CertificationFailure,
        cleanup: Option<a3s_test_session::SessionFinished>,
    ) -> Self {
        Self {
            status: GuiCertificationRunStatus::Failed,
            execution_profile,
            artifacts_root: artifacts_root.to_string_lossy().into_owned(),
            observation: None,
            cleanup,
            failure: Some(failure),
        }
    }
}

fn certify_observation(
    observation: &SurfaceObservation,
    profile: GuiProfileArg,
) -> Result<CertifiedObservation, CertificationFailure> {
    let elements = observation
        .data
        .get("elements")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CertificationFailure::observation(
                "GUI observation did not contain a structured elements array",
            )
        })?;
    if elements.is_empty() {
        return Err(CertificationFailure::observation(
            "GUI observation contained no semantic elements",
        ));
    }
    let visual_evidence_count = observation
        .evidence
        .iter()
        .filter(|evidence| evidence.media_type == "image/png")
        .count();
    if profile == GuiProfileArg::WindowVision {
        let visual = observation
            .data
            .get("visual")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                CertificationFailure::observation(
                    "window-vision observation did not contain visual grounding metadata",
                )
            })?;
        for field in ["ref", "sha256"] {
            if visual
                .get(field)
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(CertificationFailure::observation(format!(
                    "window-vision metadata did not contain {field}"
                )));
            }
        }
        if visual_evidence_count == 0 {
            return Err(CertificationFailure::observation(
                "window-vision observation did not return PNG evidence",
            ));
        }
    }
    Ok(CertifiedObservation {
        summary: observation.summary.clone(),
        semantic_element_count: elements.len(),
        visual_evidence_count,
    })
}

fn emit(run: GuiCertificationRun, json: bool) -> Result<ExitCode> {
    if json {
        println!("{}", serde_json::to_string_pretty(&run)?);
    } else {
        println!(
            "{}: {}",
            match run.status {
                GuiCertificationRunStatus::Passed => "PASS",
                GuiCertificationRunStatus::Failed => "FAIL",
            },
            run.execution_profile.id()
        );
        if let Some(observation) = &run.observation {
            println!(
                "  {} semantic element(s), {} visual evidence item(s)",
                observation.semantic_element_count, observation.visual_evidence_count
            );
        }
        if let Some(failure) = &run.failure {
            println!("  {}: {}", failure.code, failure.message);
        }
        println!("  artifacts: {}", run.artifacts_root);
    }
    Ok(match run.status {
        GuiCertificationRunStatus::Passed => ExitCode::SUCCESS,
        GuiCertificationRunStatus::Failed => ExitCode::from(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_visual_certification_without_image_evidence() {
        let observation = SurfaceObservation::new("GUI").with_data(json!({
            "elements": [{ "ref": "@g1.1" }],
            "visual": { "ref": "@v1", "sha256": "abc" }
        }));
        let error = certify_observation(&observation, GuiProfileArg::WindowVision)
            .expect_err("missing PNG evidence");
        assert_eq!(
            error.code,
            "test.driver.gui.certification_observation_invalid"
        );
    }
}
