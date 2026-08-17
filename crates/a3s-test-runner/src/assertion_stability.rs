use std::time::{Duration, Instant};

use a3s_test_core::{AssertionStability, DriverError, DriverSession, StepOutput, TestStep};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{millis, wait_for_retry, RetryWait, Runner, StepAttempt, StepExecution, StepVerdict};

pub(super) struct AssertionSampling {
    stability: AssertionStability,
    deadline: Instant,
    cancellation: CancellationToken,
    first_output: StepOutput,
    attempts: u32,
}

impl AssertionSampling {
    pub(super) fn new(
        stability: AssertionStability,
        deadline: Instant,
        cancellation: CancellationToken,
        first_output: StepOutput,
        attempts: u32,
    ) -> Self {
        Self {
            stability,
            deadline,
            cancellation,
            first_output,
            attempts,
        }
    }
}

impl Runner {
    pub(super) async fn sample_assertion_stability(
        &self,
        session: &mut dyn DriverSession,
        step: &TestStep,
        sampling: AssertionSampling,
    ) -> (StepExecution, u32) {
        let AssertionSampling {
            stability,
            deadline,
            cancellation,
            first_output,
            mut attempts,
        } = sampling;
        let started = Instant::now();
        let required = Duration::from_millis(stability.stable_for_ms);
        let interval = Duration::from_millis(stability.sample_interval_ms);
        let mut samples = 1_u64;
        let first_assertion = first_output.data;

        loop {
            let remaining = required.saturating_sub(started.elapsed());
            let wait = interval.min(remaining);
            match wait_for_retry(deadline, &cancellation, wait).await {
                RetryWait::Continue => {}
                RetryWait::Cancelled => return (StepExecution::Cancelled(None), attempts),
                RetryWait::TimedOut => return (StepExecution::TimedOut(None), attempts),
            }

            let (execution, sample_attempts) = self
                .execute_with_retries(session, step, deadline, cancellation.clone())
                .await;
            attempts = attempts.saturating_add(sample_attempts);
            let StepExecution::Completed(attempt) = execution else {
                return (execution, attempts);
            };
            samples = samples.saturating_add(1);
            let observed_ms = millis(started.elapsed());

            match *attempt {
                StepAttempt {
                    verdict: StepVerdict::Passed(output),
                    quality_report: None,
                } => {
                    if started.elapsed() >= required {
                        return (
                            StepExecution::Completed(Box::new(StepAttempt::passed(
                                with_stability_data(
                                    output,
                                    &first_assertion,
                                    stability,
                                    samples,
                                    observed_ms,
                                    "passed",
                                ),
                            ))),
                            attempts,
                        );
                    }
                }
                StepAttempt {
                    verdict: StepVerdict::Passed(output),
                    quality_report: Some(_),
                } => {
                    return (
                        StepExecution::Completed(Box::new(StepAttempt::failed(
                            DriverError::new(
                                "test.run.stability_output_invalid",
                                "assertion stability received an unexpected advisory report",
                            ),
                            Some(with_stability_data(
                                output,
                                &first_assertion,
                                stability,
                                samples,
                                observed_ms,
                                "inconclusive",
                            )),
                        ))),
                        attempts,
                    );
                }
                StepAttempt {
                    verdict: StepVerdict::Failed { error, output },
                    quality_report: None,
                } if error.code().starts_with("test.assert.") => {
                    let code = error.code().to_string();
                    let message = error.message().to_string();
                    let output = output.unwrap_or_else(|| {
                        StepOutput::new("assertion became false during stability sampling")
                    });
                    return (
                        StepExecution::Completed(Box::new(StepAttempt::failed(
                            DriverError::new(
                                "test.assert.unstable",
                                format!(
                                    "assertion became false on stability sample {samples} after {observed_ms} ms ({code}: {message})"
                                ),
                            ),
                            Some(with_stability_data(
                                output,
                                &first_assertion,
                                stability,
                                samples,
                                observed_ms,
                                "unstable",
                            )),
                        ))),
                        attempts,
                    );
                }
                StepAttempt {
                    verdict: StepVerdict::Failed { error, output },
                    quality_report,
                } => {
                    let output = output.unwrap_or_else(|| {
                        StepOutput::new("assertion stability could not be verified")
                    });
                    return (
                        StepExecution::Completed(Box::new(StepAttempt {
                            verdict: StepVerdict::Failed {
                                error,
                                output: Some(with_stability_data(
                                    output,
                                    &first_assertion,
                                    stability,
                                    samples,
                                    observed_ms,
                                    "inconclusive",
                                )),
                            },
                            quality_report,
                        })),
                        attempts,
                    );
                }
            }
        }
    }
}

fn with_stability_data(
    mut output: StepOutput,
    first_assertion: &Value,
    stability: AssertionStability,
    samples: u64,
    observed_ms: u64,
    outcome: &str,
) -> StepOutput {
    let assertion = std::mem::take(&mut output.data);
    output.summary = format!(
        "{}; assertion stability {outcome} after {observed_ms} ms across {samples} samples",
        output.summary
    );
    output.data = json!({
        "assertion": {
            "first": first_assertion,
            "last": assertion,
        },
        "stability": {
            "outcome": outcome,
            "required_ms": stability.stable_for_ms,
            "sample_interval_ms": stability.sample_interval_ms,
            "samples": samples,
            "observed_ms": observed_ms,
        }
    });
    output
}
