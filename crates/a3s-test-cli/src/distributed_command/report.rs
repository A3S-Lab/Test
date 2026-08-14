use std::collections::{BTreeMap, BTreeSet};

use a3s_test_core::Surface;
use a3s_test_runner::{RunResult, RunStatus, ScenarioResult};
use a3s_test_worker::{
    DistributedScenarioObservation, DistributedScenarioOutcome, RemoteArtifactCommand,
    RemoteArtifactDescriptor, RemoteArtifactFileDescriptor, RemoteArtifactKind,
    RemoteArtifactOutcome, RemoteArtifactRequest, RemoteArtifactSelector, RemoteJobSnapshot,
    RemoteJobState, RemoteScenarioCounts, WorkerSurface, REMOTE_ARTIFACT_PROTOCOL,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use sha2::{Digest, Sha256};

use super::http::RemoteHttpClient;
use super::request_id;

const RUN_REPORT_MEDIA_TYPE: &str = "application/vnd.a3s-test.run-result+json";

pub(super) async fn fetch_verified_report(
    client: &RemoteHttpClient,
    artifacts: &RemoteArtifactDescriptor,
    snapshot: &RemoteJobSnapshot,
    expected_suite: &str,
    expected_scenarios: &[String],
    scenario_surfaces: &BTreeMap<String, WorkerSurface>,
) -> Result<RunResult> {
    let summary = snapshot
        .result
        .as_ref()
        .context("terminal remote job did not retain a run summary")?;
    if snapshot.state != summary.status
        || !matches!(
            summary.status,
            RemoteJobState::Passed | RemoteJobState::Failed
        )
    {
        anyhow::bail!("remote job state and retained run summary status do not match");
    }
    if summary.suite != expected_suite || summary.report.media_type != RUN_REPORT_MEDIA_TYPE {
        anyhow::bail!("remote run summary is bound to an unexpected suite or media type");
    }
    if summary.report.bytes == 0 {
        anyhow::bail!("remote run report has invalid size metadata");
    }

    let expected_artifact = RemoteArtifactFileDescriptor {
        kind: RemoteArtifactKind::Report,
        path: None,
        sha256: summary.report.sha256.clone(),
        bytes: summary.report.bytes,
        media_type: summary.report.media_type.clone(),
    };
    let capacity = usize::try_from(summary.report.bytes)
        .context("remote run report size does not fit this platform")?;
    let mut report = Vec::with_capacity(capacity);
    let mut offset = 0_u64;
    while offset < summary.report.bytes {
        let remaining = summary.report.bytes - offset;
        let max_bytes = u32::try_from(remaining.min(u64::from(artifacts.limits.max_chunk_bytes)))
            .context("remote report chunk size does not fit the protocol")?;
        let request = RemoteArtifactRequest {
            protocol: REMOTE_ARTIFACT_PROTOCOL.to_string(),
            request_id: request_id("report"),
            command: RemoteArtifactCommand::Read {
                job_id: snapshot.job_id.clone(),
                dispatch_id: snapshot.dispatch_id.clone(),
                expected_request_digest: snapshot.request_digest.clone(),
                artifact: RemoteArtifactSelector::Report {
                    sha256: summary.report.sha256.clone(),
                },
                offset,
                max_bytes,
            },
        };
        let response = client.artifacts(&request).await?;
        let chunk = match response.outcome {
            RemoteArtifactOutcome::Chunk { chunk } => chunk,
            RemoteArtifactOutcome::Error { error } => {
                anyhow::bail!(
                    "remote artifact read failed [{}]: {}",
                    error.code,
                    error.message
                )
            }
            _ => anyhow::bail!("remote artifact service returned an unexpected outcome"),
        };
        if chunk.job_id != snapshot.job_id
            || chunk.dispatch_id != snapshot.dispatch_id
            || chunk.request_digest != snapshot.request_digest
            || chunk.offset != offset
            || chunk.artifact != expected_artifact
        {
            anyhow::bail!("remote report chunk binding does not match the terminal job");
        }
        let bytes = STANDARD
            .decode(&chunk.contents_base64)
            .context("remote report chunk is not canonical base64")?;
        if bytes.is_empty() || STANDARD.encode(&bytes) != chunk.contents_base64 {
            anyhow::bail!("remote report chunk is empty or non-canonical base64");
        }
        let next = offset
            .checked_add(u64::try_from(bytes.len()).context("report chunk size overflowed")?)
            .context("remote report offset overflowed")?;
        if next > summary.report.bytes || chunk.eof != (next == summary.report.bytes) {
            anyhow::bail!("remote report chunk EOF or size metadata is inconsistent");
        }
        report.extend_from_slice(&bytes);
        offset = next;
    }
    let actual_digest = format!("sha256:{:x}", Sha256::digest(&report));
    if actual_digest != summary.report.sha256 {
        anyhow::bail!("remote run report digest does not match its descriptor");
    }
    let result: RunResult =
        serde_json::from_slice(&report).context("remote run report is not a valid RunResult")?;
    verify_result(
        &result,
        snapshot,
        expected_suite,
        expected_scenarios,
        scenario_surfaces,
    )?;
    Ok(result)
}

fn verify_result(
    result: &RunResult,
    snapshot: &RemoteJobSnapshot,
    expected_suite: &str,
    expected_scenarios: &[String],
    scenario_surfaces: &BTreeMap<String, WorkerSurface>,
) -> Result<()> {
    let summary = snapshot.result.as_ref().context("run summary missing")?;
    if result.run_id != summary.run_id || result.suite != expected_suite {
        anyhow::bail!("remote run report identity does not match its summary");
    }
    let expected_status = match summary.status {
        RemoteJobState::Passed => RunStatus::Passed,
        RemoteJobState::Failed => result.status,
        _ => anyhow::bail!("remote report summary has an invalid terminal state"),
    };
    if result.status != expected_status
        || (summary.status == RemoteJobState::Failed && result.status == RunStatus::Passed)
    {
        anyhow::bail!("remote run report status does not match its summary");
    }

    let actual_ids = result
        .scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_ids = expected_scenarios
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual_ids.len() != result.scenarios.len()
        || expected_ids.len() != expected_scenarios.len()
        || actual_ids != expected_ids
    {
        anyhow::bail!("remote run report does not contain the exact dispatched scenario set");
    }
    for scenario in &result.scenarios {
        let expected = scenario_surfaces
            .get(&scenario.id)
            .context("remote run report names an unknown scenario")?;
        if scenario.surface != core_surface(*expected) {
            anyhow::bail!("remote run report scenario surface does not match the suite");
        }
    }
    if scenario_counts(&result.scenarios) != summary.scenarios {
        anyhow::bail!("remote run report scenario counts do not match its summary");
    }
    Ok(())
}

pub(super) fn observations(result: &RunResult) -> Vec<DistributedScenarioObservation> {
    result
        .scenarios
        .iter()
        .map(|scenario| {
            let failure_code = failure_code(scenario).map(ToOwned::to_owned);
            DistributedScenarioObservation {
                id: scenario.id.clone(),
                outcome: classify(scenario, failure_code.as_deref()),
                duration_ms: scenario.duration_ms,
                failure_code,
            }
        })
        .collect()
}

fn classify(scenario: &ScenarioResult, failure_code: Option<&str>) -> DistributedScenarioOutcome {
    match scenario.status {
        RunStatus::Passed => DistributedScenarioOutcome::Passed,
        RunStatus::TimedOut => DistributedScenarioOutcome::TimedOut,
        RunStatus::Cancelled => DistributedScenarioOutcome::Cancelled,
        RunStatus::Failed
            if scenario.cleanup_error.is_none()
                && failure_code.is_some_and(is_explicit_test_failure) =>
        {
            DistributedScenarioOutcome::TestFailed
        }
        RunStatus::Failed => DistributedScenarioOutcome::InfrastructureFailed,
    }
}

fn is_explicit_test_failure(code: &str) -> bool {
    code.starts_with("test.assert.")
        || matches!(
            code,
            "test.contract.mismatch" | "test.contract.state_mismatch"
        )
}

fn failure_code(scenario: &ScenarioResult) -> Option<&str> {
    scenario
        .cleanup_error
        .as_ref()
        .or(scenario.error.as_ref())
        .or_else(|| scenario.steps.iter().find_map(|step| step.error.as_ref()))
        .map(|error| error.code.as_str())
}

fn scenario_counts(scenarios: &[ScenarioResult]) -> RemoteScenarioCounts {
    let mut counts = RemoteScenarioCounts {
        passed: 0,
        failed: 0,
        timed_out: 0,
        cancelled: 0,
    };
    for scenario in scenarios {
        let count = match scenario.status {
            RunStatus::Passed => &mut counts.passed,
            RunStatus::Failed => &mut counts.failed,
            RunStatus::TimedOut => &mut counts.timed_out,
            RunStatus::Cancelled => &mut counts.cancelled,
        };
        *count = count.saturating_add(1);
    }
    counts
}

fn core_surface(surface: WorkerSurface) -> Surface {
    match surface {
        WorkerSurface::Web => Surface::Web,
        WorkerSurface::Gui => Surface::Gui,
        WorkerSurface::Tui => Surface::Tui,
    }
}

#[cfg(test)]
mod tests {
    use super::observations;
    use a3s_test_core::Surface;
    use a3s_test_runner::{RunError, RunResult, RunStatus, ScenarioResult, StepResult};
    use a3s_test_worker::DistributedScenarioOutcome;

    fn scenario(code: &str, cleanup: bool) -> ScenarioResult {
        ScenarioResult {
            id: "checkout".to_string(),
            name: "Checkout".to_string(),
            surface: Surface::Web,
            status: RunStatus::Failed,
            duration_ms: 50,
            steps: vec![StepResult {
                id: "assert".to_string(),
                status: RunStatus::Failed,
                duration_ms: 10,
                attempts: 1,
                output: None,
                error: Some(RunError {
                    code: code.to_string(),
                    message: "failure".to_string(),
                }),
            }],
            error: None,
            cleanup_error: cleanup.then(|| RunError {
                code: "test.run.cleanup_timeout".to_string(),
                message: "cleanup".to_string(),
            }),
        }
    }

    #[test]
    fn only_explicit_assertion_and_contract_mismatches_are_test_failures() {
        for (code, expected) in [
            (
                "test.assert.visible",
                DistributedScenarioOutcome::TestFailed,
            ),
            (
                "test.contract.mismatch",
                DistributedScenarioOutcome::TestFailed,
            ),
            (
                "test.contract.inconclusive",
                DistributedScenarioOutcome::InfrastructureFailed,
            ),
            (
                "test.driver.web.command_failed",
                DistributedScenarioOutcome::InfrastructureFailed,
            ),
        ] {
            let result = RunResult {
                run_id: "run".to_string(),
                suite: "suite".to_string(),
                status: RunStatus::Failed,
                scenarios: vec![scenario(code, false)],
            };
            assert_eq!(observations(&result)[0].outcome, expected, "{code}");
        }
        let cleanup = RunResult {
            run_id: "run".to_string(),
            suite: "suite".to_string(),
            status: RunStatus::Failed,
            scenarios: vec![scenario("test.assert.visible", true)],
        };
        assert_eq!(
            observations(&cleanup)[0].outcome,
            DistributedScenarioOutcome::InfrastructureFailed
        );
    }
}
