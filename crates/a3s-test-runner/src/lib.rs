//! Cancellation-safe orchestration for A3S Test.

mod assertion_stability;
mod hidden_wait;

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use a3s_test_core::{
    bind_page_context_observation_refs, validate_action_page_context_refs, Action, AssertionMode,
    ContractOutcome, DriverError, DriverSession, Expectation, ScenarioContext, StepOutput, Surface,
    SurfaceContract, SurfaceDriver, Target, TestScenario, TestStep, TestSuite, WaitMode,
};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

pub const HIDDEN_WAIT_POLL_INTERVAL_MS: u64 = 50;
pub const MAX_HIDDEN_WAIT_PROBES: u64 = 1_201;

#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 1,
            backoff: Duration::from_millis(100),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RunnerOptions {
    pub cleanup_timeout: Duration,
    pub quality_projection_timeout: Duration,
    pub retry_policy: RetryPolicy,
    pub max_parallel_scenarios: usize,
}

impl Default for RunnerOptions {
    fn default() -> Self {
        Self {
            cleanup_timeout: Duration::from_secs(10),
            quality_projection_timeout: Duration::from_secs(2),
            retry_policy: RetryPolicy::default(),
            max_parallel_scenarios: 1,
        }
    }
}

pub struct Runner {
    drivers: HashMap<Surface, Arc<dyn SurfaceDriver>>,
    contracts: ContractRegistry,
    options: RunnerOptions,
    artifacts_root: PathBuf,
}

pub struct SessionStepResult {
    pub status: RunStatus,
    pub attempts: u32,
    pub output: Option<StepOutput>,
    pub error: Option<DriverError>,
}

#[derive(Clone, Default)]
pub struct ContractRegistry {
    contracts: HashMap<String, Arc<SurfaceContract>>,
}

impl ContractRegistry {
    pub fn new<K>(contracts: impl IntoIterator<Item = (K, SurfaceContract)>) -> Result<Self, String>
    where
        K: Into<String>,
    {
        let mut by_reference = HashMap::new();
        for (reference, contract) in contracts {
            let reference = reference.into();
            if reference.trim().is_empty() {
                return Err("contract references must not be empty".to_string());
            }
            if by_reference
                .insert(reference.clone(), Arc::new(contract))
                .is_some()
            {
                return Err(format!(
                    "duplicate admitted contract reference '{reference}'"
                ));
            }
        }
        Ok(Self {
            contracts: by_reference,
        })
    }

    fn get(&self, reference: &str) -> Option<&SurfaceContract> {
        self.contracts.get(reference).map(AsRef::as_ref)
    }
}

impl Runner {
    pub fn new(
        drivers: Vec<Arc<dyn SurfaceDriver>>,
        options: RunnerOptions,
    ) -> Result<Self, String> {
        if options.cleanup_timeout.is_zero() {
            return Err("cleanup timeout must be greater than zero".to_string());
        }
        if options.quality_projection_timeout.is_zero() {
            return Err("quality projection timeout must be greater than zero".to_string());
        }
        if options.retry_policy.max_retries > 10 {
            return Err("infrastructure retries cannot exceed 10".to_string());
        }
        if !(1..=64).contains(&options.max_parallel_scenarios) {
            return Err("parallel scenario limit must be between 1 and 64".to_string());
        }

        let mut by_surface = HashMap::new();
        for driver in drivers {
            let surface = driver.surface();
            if by_surface.insert(surface, driver).is_some() {
                return Err(format!("duplicate driver for {surface:?}"));
            }
        }

        Ok(Self {
            drivers: by_surface,
            contracts: ContractRegistry::default(),
            options,
            artifacts_root: PathBuf::from(".a3s-test"),
        })
    }

    #[must_use]
    pub fn with_contracts(mut self, contracts: ContractRegistry) -> Self {
        self.contracts = contracts;
        self
    }

    pub fn with_artifacts_root(mut self, artifacts_root: PathBuf) -> Result<Self, String> {
        if artifacts_root.as_os_str().is_empty() {
            return Err("artifact root must not be empty".to_string());
        }
        self.artifacts_root = artifacts_root;
        Ok(self)
    }

    /// Execute an admitted step against an already-open session with the same
    /// retry, stability, and negative-visibility policies used by full runs.
    pub async fn execute_admitted_step(
        &self,
        session: &mut dyn DriverSession,
        step: &TestStep,
        deadline: Instant,
        cancellation: CancellationToken,
    ) -> SessionStepResult {
        let (execution, attempts) = self
            .execute_step(session, step, deadline, cancellation)
            .await;
        match execution {
            StepExecution::Completed(attempt) => match attempt.verdict {
                StepVerdict::Passed(output) => SessionStepResult {
                    status: RunStatus::Passed,
                    attempts,
                    output: Some(output),
                    error: None,
                },
                StepVerdict::Failed { error, output } => SessionStepResult {
                    status: RunStatus::Failed,
                    attempts,
                    output,
                    error: Some(error),
                },
            },
            StepExecution::TimedOut(output) => SessionStepResult {
                status: RunStatus::TimedOut,
                attempts,
                output,
                error: Some(DriverError::new(
                    "test.run.timeout",
                    "scenario deadline exceeded",
                )),
            },
            StepExecution::Cancelled(output) => SessionStepResult {
                status: RunStatus::Cancelled,
                attempts,
                output,
                error: Some(DriverError::new("test.run.cancelled", "run cancelled")),
            },
        }
    }

    pub async fn run(&self, suite: &TestSuite, cancellation: CancellationToken) -> RunResult {
        let run_id = new_run_id();
        // Buffered scenario futures own their inputs so this run future remains
        // Send when a Runner is embedded behind an async-trait executor.
        let mut indexed = stream::iter(suite.scenarios.clone().into_iter().enumerate())
            .map(|(index, scenario)| {
                let cancellation = cancellation.clone();
                let run_id = run_id.clone();
                async move {
                    (
                        index,
                        self.run_scenario(&run_id, &scenario, cancellation).await,
                    )
                }
            })
            .buffer_unordered(self.options.max_parallel_scenarios)
            .collect::<Vec<_>>()
            .await;
        indexed.sort_by_key(|(index, _)| *index);
        let scenarios = indexed
            .into_iter()
            .map(|(_, scenario)| scenario)
            .collect::<Vec<_>>();

        let status = aggregate_status(scenarios.iter().map(|scenario| scenario.status));
        RunResult {
            run_id,
            suite: suite.name.clone(),
            status,
            scenarios,
        }
    }

    async fn run_scenario(
        &self,
        run_id: &str,
        scenario: &TestScenario,
        cancellation: CancellationToken,
    ) -> ScenarioResult {
        let Some(driver) = self.drivers.get(&scenario.surface) else {
            return ScenarioResult::not_started(
                scenario,
                RunStatus::Failed,
                "test.run.driver_missing",
                "no driver is registered for this surface",
            );
        };

        let started = Instant::now();
        let mut deadline = started + Duration::from_millis(scenario.timeout_ms);
        let mut quality_projection_budget = self.options.quality_projection_timeout;
        let context = ScenarioContext {
            run_id: run_id.to_string(),
            scenario_id: scenario.id.clone(),
            artifacts_dir: self
                .artifacts_root
                .clone()
                .join("runs")
                .join(run_id)
                .join(artifact_component(&scenario.id)),
        };

        let mut open_retries = 0;
        let mut session = loop {
            let open = tokio::select! {
                biased;
                () = cancellation.cancelled() => {
                    return ScenarioResult::not_started(
                        scenario,
                        RunStatus::Cancelled,
                        "test.run.cancelled",
                        "run cancelled while opening the surface",
                    );
                }
                result = tokio::time::timeout_at(deadline.into(), driver.open(&context)) => result,
            };

            match open {
                Ok(Ok(session)) => break session,
                Ok(Err(error))
                    if error.retryable()
                        && open_retries < self.options.retry_policy.max_retries =>
                {
                    open_retries += 1;
                    match wait_for_retry(deadline, &cancellation, self.options.retry_policy.backoff)
                        .await
                    {
                        RetryWait::Continue => {}
                        RetryWait::Cancelled => {
                            return ScenarioResult::not_started(
                                scenario,
                                RunStatus::Cancelled,
                                "test.run.cancelled",
                                "run cancelled while retrying surface setup",
                            );
                        }
                        RetryWait::TimedOut => {
                            return ScenarioResult::not_started(
                                scenario,
                                RunStatus::TimedOut,
                                "test.run.timeout",
                                "scenario timed out while retrying surface setup",
                            );
                        }
                    }
                }
                Ok(Err(error)) => {
                    return ScenarioResult::not_started_from_driver(
                        scenario,
                        RunStatus::Failed,
                        error,
                    );
                }
                Err(_) => {
                    return ScenarioResult::not_started(
                        scenario,
                        RunStatus::TimedOut,
                        "test.run.timeout",
                        "scenario timed out while opening the surface",
                    );
                }
            }
        };

        let mut status = RunStatus::Passed;
        let mut steps = Vec::with_capacity(scenario.steps.len());

        for step in &scenario.steps {
            let step_started = Instant::now();
            let (execution, attempts) = self
                .execute_step(session.as_mut(), step, deadline, cancellation.clone())
                .await;

            let (step_result, quality_report) = match execution {
                StepExecution::Completed(result) => {
                    let StepAttempt {
                        verdict,
                        quality_report,
                    } = *result;
                    let step_result = match verdict {
                        StepVerdict::Passed(output) => {
                            StepResult::passed(&step.id, step_started.elapsed(), attempts, output)
                        }
                        StepVerdict::Failed { error, output } => {
                            status = RunStatus::Failed;
                            StepResult::failed(
                                &step.id,
                                step_started.elapsed(),
                                attempts,
                                error,
                                output,
                            )
                        }
                    };
                    (step_result, quality_report)
                }
                StepExecution::TimedOut(output) => {
                    status = RunStatus::TimedOut;
                    (
                        StepResult::terminal(
                            &step.id,
                            RunStatus::TimedOut,
                            step_started.elapsed(),
                            attempts,
                            "test.run.timeout",
                            "scenario deadline exceeded",
                            output,
                        ),
                        None,
                    )
                }
                StepExecution::Cancelled(output) => {
                    status = RunStatus::Cancelled;
                    (
                        StepResult::terminal(
                            &step.id,
                            RunStatus::Cancelled,
                            step_started.elapsed(),
                            attempts,
                            "test.run.cancelled",
                            "run cancelled",
                            output,
                        ),
                        None,
                    )
                }
            };
            steps.push(step_result);

            if let Some(report) = quality_report.as_ref() {
                let projection_started = Instant::now();
                self.project_quality_report(
                    session.as_mut(),
                    report,
                    quality_projection_budget,
                    &cancellation,
                )
                .await;
                let elapsed = projection_started.elapsed().min(quality_projection_budget);
                quality_projection_budget = quality_projection_budget.saturating_sub(elapsed);
                deadline = deadline.checked_add(elapsed).unwrap_or(deadline);
            }

            if status != RunStatus::Passed {
                break;
            }
        }

        let cleanup_error =
            match tokio::time::timeout(self.options.cleanup_timeout, session.close()).await {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(RunError::from_driver(error)),
                Err(_) => Some(RunError {
                    code: "test.run.cleanup_timeout".to_string(),
                    message: "surface cleanup exceeded its deadline".to_string(),
                }),
            };
        if status == RunStatus::Passed && cleanup_error.is_some() {
            status = RunStatus::Failed;
        }

        ScenarioResult {
            id: scenario.id.clone(),
            name: scenario.name.clone(),
            surface: scenario.surface,
            status,
            duration_ms: millis(started.elapsed()),
            steps,
            error: None,
            cleanup_error,
        }
    }

    async fn execute_step(
        &self,
        session: &mut dyn DriverSession,
        step: &TestStep,
        deadline: Instant,
        cancellation: CancellationToken,
    ) -> (StepExecution, u32) {
        if step.wait_mode == WaitMode::Hidden {
            return self
                .execute_hidden_wait(session, step, deadline, cancellation)
                .await;
        }
        let Some(stability) = step.stability else {
            return self
                .execute_with_retries(session, step, deadline, cancellation)
                .await;
        };
        if !matches!(step.action, Action::Assert { .. }) {
            return (
                StepExecution::Completed(Box::new(StepAttempt::failed(
                    DriverError::new(
                        "test.run.stability_action_invalid",
                        "assertion stability is valid only for expect steps",
                    ),
                    None,
                ))),
                0,
            );
        }
        if !stability.is_valid() {
            return (
                StepExecution::Completed(Box::new(StepAttempt::failed(
                    DriverError::new(
                        "test.run.stability_invalid",
                        "assertion stability is outside the admitted duration, interval, or sample bounds",
                    ),
                    None,
                ))),
                0,
            );
        }

        let (initial, attempts) = self
            .execute_with_retries(session, step, deadline, cancellation.clone())
            .await;
        let StepExecution::Completed(initial) = initial else {
            return (initial, attempts);
        };
        match *initial {
            StepAttempt {
                verdict: StepVerdict::Passed(output),
                quality_report: None,
            } => {
                self.sample_assertion_stability(
                    session,
                    step,
                    assertion_stability::AssertionSampling::new(
                        stability,
                        deadline,
                        cancellation,
                        output,
                        attempts,
                    ),
                )
                .await
            }
            StepAttempt {
                verdict: StepVerdict::Passed(output),
                quality_report: Some(_),
            } => (
                StepExecution::Completed(Box::new(StepAttempt::failed(
                    DriverError::new(
                        "test.run.stability_output_invalid",
                        "assertion stability received an unexpected advisory report",
                    ),
                    Some(output),
                ))),
                attempts,
            ),
            attempt => (StepExecution::Completed(Box::new(attempt)), attempts),
        }
    }

    async fn execute_with_retries(
        &self,
        session: &mut dyn DriverSession,
        step: &TestStep,
        deadline: Instant,
        cancellation: CancellationToken,
    ) -> (StepExecution, u32) {
        let mut attempts = 0_u32;
        loop {
            attempts = attempts.saturating_add(1);
            let execution = tokio::select! {
                biased;
                () = cancellation.cancelled() => StepExecution::Cancelled(None),
                result = tokio::time::timeout_at(deadline.into(), self.execute_once(session, step)) => {
                    match result {
                        Ok(result) => StepExecution::Completed(Box::new(result)),
                        Err(_) => StepExecution::TimedOut(None),
                    }
                }
            };
            let retryable = matches!(
                &execution,
                StepExecution::Completed(result)
                    if matches!(&result.verdict, StepVerdict::Failed { error, .. } if error.retryable())
            );
            if !retryable || attempts > self.options.retry_policy.max_retries {
                return (execution, attempts);
            }

            match wait_for_retry(deadline, &cancellation, self.options.retry_policy.backoff).await {
                RetryWait::Continue => {}
                RetryWait::Cancelled => return (StepExecution::Cancelled(None), attempts),
                RetryWait::TimedOut => return (StepExecution::TimedOut(None), attempts),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_once(&self, session: &mut dyn DriverSession, step: &TestStep) -> StepAttempt {
        if let Err(error) = validate_action_page_context_refs(&step.action) {
            return StepAttempt::failed(
                DriverError::new("test.run.context_ref_invalid", error.message()),
                None,
            );
        }
        if step.assertion_mode == AssertionMode::Hidden {
            return Self::execute_hidden_assertion(session, step).await;
        }
        let Action::VerifyContract {
            contract,
            variant,
            state,
        } = &step.action
        else {
            return match session.execute(step).await {
                Ok(mut output) => {
                    bind_output_page_context(&mut output);
                    StepAttempt::passed(output)
                }
                Err(error) => StepAttempt::failed(error, None),
            };
        };
        let Some(admitted) = self.contracts.get(contract) else {
            return StepAttempt::failed(
                DriverError::new(
                    "test.run.contract_missing",
                    format!("contract reference '{contract}' was not admitted before the run"),
                ),
                None,
            );
        };
        let observation = match session.observe().await {
            Ok(observation) => observation,
            Err(error) => {
                return StepAttempt::failed(error, None);
            }
        };
        let report = match admitted.reconcile(variant, state, &observation) {
            Ok(report) => report,
            Err(error) => {
                return StepAttempt::failed(DriverError::new(error.code(), error.message()), None);
            }
        };
        let data = match serde_json::to_value(&report) {
            Ok(data) => data,
            Err(error) => {
                return StepAttempt::failed(
                    DriverError::new(
                        "test.run.contract_report_invalid",
                        format!("failed to encode the contract report: {error}"),
                    ),
                    None,
                );
            }
        };
        let output = StepOutput::new(match report.outcome {
            ContractOutcome::Passed if report.findings.is_empty() => "surface contract passed",
            ContractOutcome::Passed => "surface contract passed with advisory findings",
            ContractOutcome::Failed => "surface contract failed",
            ContractOutcome::Inconclusive => "surface contract was inconclusive",
        })
        .with_data(data);
        let verdict = match report.outcome {
            ContractOutcome::Passed => StepVerdict::Passed(output),
            ContractOutcome::Failed => StepVerdict::Failed {
                error: DriverError::new(
                    "test.contract.mismatch",
                    format!(
                        "surface contract '{}' reported {} finding(s)",
                        report.contract,
                        report.findings.len()
                    ),
                ),
                output: Some(output),
            },
            ContractOutcome::Inconclusive => StepVerdict::Failed {
                error: DriverError::new(
                    "test.contract.inconclusive",
                    format!(
                        "surface contract '{}' could not be proven from the bounded observation",
                        report.contract
                    ),
                ),
                output: Some(output),
            },
        };
        StepAttempt {
            verdict,
            quality_report: Some(report),
        }
    }

    async fn execute_hidden_assertion(
        session: &mut dyn DriverSession,
        step: &TestStep,
    ) -> StepAttempt {
        let Action::Assert {
            expectation: Expectation::Visible(target),
        } = &step.action
        else {
            return StepAttempt::failed(
                DriverError::new(
                    "test.run.assertion_mode_invalid",
                    "hidden assertion mode requires a visible-target expectation",
                ),
                None,
            );
        };
        if matches!(target, Target::Ref { .. } | Target::VisualPoint { .. }) {
            return StepAttempt::failed(
                DriverError::new(
                    "test.run.assertion_mode_invalid",
                    "hidden assertions require a stable semantic or CSS locator, not an observation-bound ref or visual point",
                ),
                None,
            );
        }

        let mut probe = step.clone();
        probe.assertion_mode = AssertionMode::Positive;
        probe.stability = None;
        match session.execute(&probe).await {
            Ok(mut output) => {
                bind_output_page_context(&mut output);
                let probe_data = std::mem::take(&mut output.data);
                output.summary = format!("{}; expected no visible target match", output.summary);
                output.data = json!({
                    "expected": "hidden",
                    "visible": true,
                    "target": target,
                    "probe": probe_data,
                });
                StepAttempt::failed(
                    DriverError::new(
                        "test.assert.hidden",
                        "expected the target to be hidden or absent, but a visible match was found",
                    ),
                    Some(output),
                )
            }
            Err(error) if error.code() == "test.assert.visible" => {
                let output = StepOutput::new("target has no visible match").with_data(json!({
                    "expected": "hidden",
                    "visible": false,
                    "target": target,
                    "probe_error": {
                        "code": error.code(),
                        "message": error.message(),
                    },
                }));
                StepAttempt::passed(output)
            }
            Err(error) => StepAttempt::failed(error, None),
        }
    }

    async fn project_quality_report(
        &self,
        session: &mut dyn DriverSession,
        report: &a3s_test_core::ContractReport,
        budget: Duration,
        cancellation: &CancellationToken,
    ) {
        if budget.is_zero() {
            return;
        }
        tokio::select! {
            biased;
            () = cancellation.cancelled() => {}
            _ = tokio::time::timeout(budget, session.project_quality_report(report)) => {}
        }
    }
}

fn bind_output_page_context(output: &mut StepOutput) {
    if let Some(page_context) = output.page_context.as_mut() {
        let _ = bind_page_context_observation_refs(page_context);
    }
}

enum RetryWait {
    Continue,
    Cancelled,
    TimedOut,
}

async fn wait_for_retry(
    deadline: Instant,
    cancellation: &CancellationToken,
    backoff: Duration,
) -> RetryWait {
    tokio::select! {
        biased;
        () = cancellation.cancelled() => RetryWait::Cancelled,
        result = tokio::time::timeout_at(deadline.into(), tokio::time::sleep(backoff)) => {
            if result.is_ok() {
                RetryWait::Continue
            } else {
                RetryWait::TimedOut
            }
        }
    }
}

enum StepExecution {
    Completed(Box<StepAttempt>),
    TimedOut(Option<StepOutput>),
    Cancelled(Option<StepOutput>),
}

struct StepAttempt {
    verdict: StepVerdict,
    quality_report: Option<a3s_test_core::ContractReport>,
}

impl StepAttempt {
    fn passed(output: StepOutput) -> Self {
        Self {
            verdict: StepVerdict::Passed(output),
            quality_report: None,
        }
    }

    fn failed(error: DriverError, output: Option<StepOutput>) -> Self {
        Self {
            verdict: StepVerdict::Failed { error, output },
            quality_report: None,
        }
    }
}

enum StepVerdict {
    Passed(StepOutput),
    Failed {
        error: DriverError,
        output: Option<StepOutput>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Passed,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RunResult {
    pub run_id: String,
    pub suite: String,
    pub status: RunStatus,
    pub scenarios: Vec<ScenarioResult>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScenarioResult {
    pub id: String,
    pub name: String,
    pub surface: Surface,
    pub status: RunStatus,
    pub duration_ms: u64,
    pub steps: Vec<StepResult>,
    pub error: Option<RunError>,
    pub cleanup_error: Option<RunError>,
}

impl ScenarioResult {
    fn not_started(scenario: &TestScenario, status: RunStatus, code: &str, message: &str) -> Self {
        Self {
            id: scenario.id.clone(),
            name: scenario.name.clone(),
            surface: scenario.surface,
            status,
            duration_ms: 0,
            steps: Vec::new(),
            error: Some(RunError {
                code: code.to_string(),
                message: message.to_string(),
            }),
            cleanup_error: None,
        }
    }

    fn not_started_from_driver(
        scenario: &TestScenario,
        status: RunStatus,
        error: DriverError,
    ) -> Self {
        let mut result = Self::not_started(scenario, status, error.code(), error.message());
        result.error = Some(RunError::from_driver(error));
        result
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StepResult {
    pub id: String,
    pub status: RunStatus,
    pub duration_ms: u64,
    pub attempts: u32,
    pub output: Option<StepOutput>,
    pub error: Option<RunError>,
}

impl StepResult {
    fn passed(id: &str, duration: Duration, attempts: u32, output: StepOutput) -> Self {
        Self {
            id: id.to_string(),
            status: RunStatus::Passed,
            duration_ms: millis(duration),
            attempts,
            output: Some(output),
            error: None,
        }
    }

    fn failed(
        id: &str,
        duration: Duration,
        attempts: u32,
        error: DriverError,
        output: Option<StepOutput>,
    ) -> Self {
        Self {
            id: id.to_string(),
            status: RunStatus::Failed,
            duration_ms: millis(duration),
            attempts,
            output,
            error: Some(RunError::from_driver(error)),
        }
    }

    fn terminal(
        id: &str,
        status: RunStatus,
        duration: Duration,
        attempts: u32,
        code: &str,
        message: &str,
        output: Option<StepOutput>,
    ) -> Self {
        Self {
            id: id.to_string(),
            status,
            duration_ms: millis(duration),
            attempts,
            output,
            error: Some(RunError {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunError {
    pub code: String,
    pub message: String,
}

impl RunError {
    fn from_driver(error: DriverError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.message().to_string(),
        }
    }
}

fn aggregate_status(statuses: impl Iterator<Item = RunStatus>) -> RunStatus {
    statuses.fold(RunStatus::Passed, |aggregate, status| {
        if severity(status) > severity(aggregate) {
            status
        } else {
            aggregate
        }
    })
}

fn severity(status: RunStatus) -> u8 {
    match status {
        RunStatus::Passed => 0,
        RunStatus::Failed => 1,
        RunStatus::TimedOut => 2,
        RunStatus::Cancelled => 3,
    }
}

fn new_run_id() -> String {
    let sequence = NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed);
    format!("a3s-test-{}-{sequence}", std::process::id())
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn artifact_component(value: &str) -> String {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        return value.to_string();
    }

    let readable = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(32)
        .collect::<String>();
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    let prefix = if readable.is_empty() {
        "scenario"
    } else {
        &readable
    };
    format!("{prefix}-{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::artifact_component;

    #[test]
    fn artifact_component_cannot_traverse_directories() {
        let component = artifact_component("../../outside");
        assert!(!component.contains('/'));
        assert!(!component.contains(".."));
    }
}
