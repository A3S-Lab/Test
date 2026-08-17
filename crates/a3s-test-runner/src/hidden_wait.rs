use std::time::{Duration, Instant};

use a3s_test_core::{
    Action, AssertionMode, DriverError, DriverSession, Expectation, StepOutput, Target, TestStep,
    WaitCondition, WaitMode,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{
    millis, wait_for_retry, RetryWait, Runner, StepAttempt, StepExecution, StepVerdict,
    HIDDEN_WAIT_POLL_INTERVAL_MS, MAX_HIDDEN_WAIT_PROBES,
};

impl Runner {
    pub(super) async fn execute_hidden_wait(
        &self,
        session: &mut dyn DriverSession,
        step: &TestStep,
        deadline: Instant,
        cancellation: CancellationToken,
    ) -> (StepExecution, u32) {
        let Action::Wait {
            condition: WaitCondition::Visible(target),
        } = &step.action
        else {
            return (
                StepExecution::Completed(Box::new(StepAttempt::failed(
                    DriverError::new(
                        "test.run.wait_mode_invalid",
                        "hidden wait mode requires a visible-target wait condition",
                    ),
                    None,
                ))),
                0,
            );
        };
        if step.assertion_mode != AssertionMode::Positive || step.stability.is_some() {
            return (
                StepExecution::Completed(Box::new(StepAttempt::failed(
                    DriverError::new(
                        "test.run.wait_mode_invalid",
                        "hidden waits cannot carry assertion or stability policy",
                    ),
                    None,
                ))),
                0,
            );
        }
        if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
            return (
                StepExecution::Completed(Box::new(StepAttempt::failed(
                    DriverError::new(
                        "test.run.wait_mode_invalid",
                        "hidden waits require a stable semantic or CSS locator, not an observation-bound ref or visual point",
                    ),
                    None,
                ))),
                0,
            );
        }

        let probe = TestStep {
            id: step.id.clone(),
            action: Action::Assert {
                expectation: Expectation::Visible(target.clone()),
            },
            stability: None,
            assertion_mode: AssertionMode::Positive,
            wait_mode: WaitMode::Positive,
        };
        let started = Instant::now();
        let interval = Duration::from_millis(HIDDEN_WAIT_POLL_INTERVAL_MS);
        let mut attempts = 0_u32;
        let mut probes = 0_u64;
        let mut first_visible = None;
        let mut last_visible = None;

        loop {
            let (execution, probe_attempts) = self
                .execute_with_retries(session, &probe, deadline, cancellation.clone())
                .await;
            attempts = attempts.saturating_add(probe_attempts);
            let attempt = match execution {
                StepExecution::Completed(attempt) => attempt,
                StepExecution::TimedOut(_) => {
                    let output = hidden_wait_output(
                        StepOutput::new("hidden wait reached the scenario deadline"),
                        target,
                        first_visible.as_ref(),
                        last_visible.as_ref(),
                        probes,
                        millis(started.elapsed()),
                        "timed_out",
                        None,
                        last_visible.as_ref().map(|_| true),
                    );
                    return (StepExecution::TimedOut(Some(output)), attempts);
                }
                StepExecution::Cancelled(_) => {
                    let output = hidden_wait_output(
                        StepOutput::new("hidden wait was cancelled"),
                        target,
                        first_visible.as_ref(),
                        last_visible.as_ref(),
                        probes,
                        millis(started.elapsed()),
                        "cancelled",
                        None,
                        last_visible.as_ref().map(|_| true),
                    );
                    return (StepExecution::Cancelled(Some(output)), attempts);
                }
            };
            probes = probes.saturating_add(1);

            match *attempt {
                StepAttempt {
                    verdict: StepVerdict::Passed(output),
                    quality_report: None,
                } => {
                    if first_visible.is_none() {
                        first_visible = Some(output.data.clone());
                    }
                    last_visible = Some(output.data);
                    if probes >= MAX_HIDDEN_WAIT_PROBES {
                        let output = hidden_wait_output(
                            StepOutput::new(
                                "target remained visible through the hidden wait probe limit",
                            ),
                            target,
                            first_visible.as_ref(),
                            last_visible.as_ref(),
                            probes,
                            millis(started.elapsed()),
                            "probe_limit",
                            None,
                            Some(true),
                        );
                        return (
                            StepExecution::Completed(Box::new(StepAttempt::failed(
                                DriverError::new(
                                    "test.run.hidden_wait_probe_limit",
                                    format!(
                                        "target remained visible through the static limit of {MAX_HIDDEN_WAIT_PROBES} probes"
                                    ),
                                ),
                                Some(output),
                            ))),
                            attempts,
                        );
                    }
                }
                StepAttempt {
                    verdict: StepVerdict::Passed(output),
                    quality_report: Some(_),
                } => {
                    let output = hidden_wait_output(
                        output,
                        target,
                        first_visible.as_ref(),
                        last_visible.as_ref(),
                        probes,
                        millis(started.elapsed()),
                        "inconclusive",
                        None,
                        None,
                    );
                    return (
                        StepExecution::Completed(Box::new(StepAttempt::failed(
                            DriverError::new(
                                "test.run.wait_output_invalid",
                                "hidden wait visibility probe returned an unexpected advisory report",
                            ),
                            Some(output),
                        ))),
                        attempts,
                    );
                }
                StepAttempt {
                    verdict: StepVerdict::Failed { error, .. },
                    quality_report: None,
                } if error.code() == "test.assert.visible" => {
                    let output = hidden_wait_output(
                        StepOutput::new("target has no visible match"),
                        target,
                        first_visible.as_ref(),
                        last_visible.as_ref(),
                        probes,
                        millis(started.elapsed()),
                        "matched",
                        Some(&error),
                        Some(false),
                    );
                    return (
                        StepExecution::Completed(Box::new(StepAttempt::passed(output))),
                        attempts,
                    );
                }
                StepAttempt {
                    verdict: StepVerdict::Failed { error, output },
                    quality_report,
                } => {
                    let output = hidden_wait_output(
                        output.unwrap_or_else(|| {
                            StepOutput::new("hidden wait visibility could not be verified")
                        }),
                        target,
                        first_visible.as_ref(),
                        last_visible.as_ref(),
                        probes,
                        millis(started.elapsed()),
                        "inconclusive",
                        Some(&error),
                        None,
                    );
                    return (
                        StepExecution::Completed(Box::new(StepAttempt {
                            verdict: StepVerdict::Failed {
                                error,
                                output: Some(output),
                            },
                            quality_report,
                        })),
                        attempts,
                    );
                }
            }

            match wait_for_retry(deadline, &cancellation, interval).await {
                RetryWait::Continue => {}
                RetryWait::Cancelled => {
                    let output = hidden_wait_output(
                        StepOutput::new("hidden wait was cancelled"),
                        target,
                        first_visible.as_ref(),
                        last_visible.as_ref(),
                        probes,
                        millis(started.elapsed()),
                        "cancelled",
                        None,
                        last_visible.as_ref().map(|_| true),
                    );
                    return (StepExecution::Cancelled(Some(output)), attempts);
                }
                RetryWait::TimedOut => {
                    let output = hidden_wait_output(
                        StepOutput::new("hidden wait reached the scenario deadline"),
                        target,
                        first_visible.as_ref(),
                        last_visible.as_ref(),
                        probes,
                        millis(started.elapsed()),
                        "timed_out",
                        None,
                        last_visible.as_ref().map(|_| true),
                    );
                    return (StepExecution::TimedOut(Some(output)), attempts);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn hidden_wait_output(
    mut output: StepOutput,
    target: &Target,
    first_visible: Option<&Value>,
    last_visible: Option<&Value>,
    probes: u64,
    observed_ms: u64,
    outcome: &str,
    probe_error: Option<&DriverError>,
    visible: Option<bool>,
) -> StepOutput {
    let terminal_probe = std::mem::take(&mut output.data);
    let probe_error = probe_error.map(|error| {
        json!({
            "code": error.code(),
            "message": error.message(),
        })
    });
    output.summary = format!(
        "{}; hidden wait {outcome} after {observed_ms} ms across {probes} probes",
        output.summary
    );
    output.data = json!({
        "expected": "hidden",
        "visible": visible,
        "target": target,
        "first_visible": first_visible,
        "last_visible": last_visible,
        "terminal_probe": terminal_probe,
        "probe_error": probe_error,
        "wait": {
            "condition": "hidden",
            "outcome": outcome,
            "poll_interval_ms": HIDDEN_WAIT_POLL_INTERVAL_MS,
            "max_probes": MAX_HIDDEN_WAIT_PROBES,
            "probes": probes,
            "observed_ms": observed_ms,
        }
    });
    output
}
