//! Cancellation-safe orchestration for A3S Test.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use a3s_test_core::{
    DriverError, DriverSession, ScenarioContext, StepOutput, Surface, SurfaceDriver, TestScenario,
    TestStep, TestSuite,
};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

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
    pub retry_policy: RetryPolicy,
    pub max_parallel_scenarios: usize,
}

impl Default for RunnerOptions {
    fn default() -> Self {
        Self {
            cleanup_timeout: Duration::from_secs(10),
            retry_policy: RetryPolicy::default(),
            max_parallel_scenarios: 1,
        }
    }
}

pub struct Runner {
    drivers: HashMap<Surface, Arc<dyn SurfaceDriver>>,
    options: RunnerOptions,
}

impl Runner {
    pub fn new(
        drivers: Vec<Arc<dyn SurfaceDriver>>,
        options: RunnerOptions,
    ) -> Result<Self, String> {
        if options.cleanup_timeout.is_zero() {
            return Err("cleanup timeout must be greater than zero".to_string());
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
            options,
        })
    }

    pub async fn run(&self, suite: &TestSuite, cancellation: CancellationToken) -> RunResult {
        let run_id = new_run_id();
        let mut indexed = stream::iter(suite.scenarios.iter().enumerate())
            .map(|(index, scenario)| {
                let cancellation = cancellation.clone();
                let run_id = &run_id;
                async move {
                    (
                        index,
                        self.run_scenario(run_id, scenario, cancellation).await,
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
        let deadline = started + Duration::from_millis(scenario.timeout_ms);
        let context = ScenarioContext {
            run_id: run_id.to_string(),
            scenario_id: scenario.id.clone(),
            artifacts_dir: PathBuf::from(".a3s-test")
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

            let step_result = match execution {
                StepExecution::Completed(result) => match *result {
                    Ok(output) => {
                        StepResult::passed(&step.id, step_started.elapsed(), attempts, output)
                    }
                    Err(error) => {
                        status = RunStatus::Failed;
                        StepResult::failed(&step.id, step_started.elapsed(), attempts, error)
                    }
                },
                StepExecution::TimedOut => {
                    status = RunStatus::TimedOut;
                    StepResult::terminal(
                        &step.id,
                        RunStatus::TimedOut,
                        step_started.elapsed(),
                        attempts,
                        "test.run.timeout",
                        "scenario deadline exceeded",
                    )
                }
                StepExecution::Cancelled => {
                    status = RunStatus::Cancelled;
                    StepResult::terminal(
                        &step.id,
                        RunStatus::Cancelled,
                        step_started.elapsed(),
                        attempts,
                        "test.run.cancelled",
                        "run cancelled",
                    )
                }
            };
            steps.push(step_result);

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
        let mut attempts = 0_u32;
        loop {
            attempts = attempts.saturating_add(1);
            let execution = tokio::select! {
                biased;
                () = cancellation.cancelled() => StepExecution::Cancelled,
                result = tokio::time::timeout_at(deadline.into(), session.execute(step)) => {
                    match result {
                        Ok(result) => StepExecution::Completed(Box::new(result)),
                        Err(_) => StepExecution::TimedOut,
                    }
                }
            };
            let retryable = matches!(
                &execution,
                StepExecution::Completed(result)
                    if matches!(result.as_ref(), Err(error) if error.retryable())
            );
            if !retryable || attempts > self.options.retry_policy.max_retries {
                return (execution, attempts);
            }

            match wait_for_retry(deadline, &cancellation, self.options.retry_policy.backoff).await {
                RetryWait::Continue => {}
                RetryWait::Cancelled => return (StepExecution::Cancelled, attempts),
                RetryWait::TimedOut => return (StepExecution::TimedOut, attempts),
            }
        }
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
    Completed(Box<Result<StepOutput, DriverError>>),
    TimedOut,
    Cancelled,
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

    fn failed(id: &str, duration: Duration, attempts: u32, error: DriverError) -> Self {
        Self {
            id: id.to_string(),
            status: RunStatus::Failed,
            duration_ms: millis(duration),
            attempts,
            output: None,
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
    ) -> Self {
        Self {
            id: id.to_string(),
            status,
            duration_ms: millis(duration),
            attempts,
            output: None,
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
