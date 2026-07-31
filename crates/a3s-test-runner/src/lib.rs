//! Cancellation-safe orchestration for A3S Test.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use a3s_test_core::{
    DriverError, ScenarioContext, StepOutput, Surface, SurfaceDriver, TestScenario, TestSuite,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug)]
pub struct RunnerOptions {
    pub cleanup_timeout: Duration,
}

impl Default for RunnerOptions {
    fn default() -> Self {
        Self {
            cleanup_timeout: Duration::from_secs(10),
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
        let mut scenarios = Vec::with_capacity(suite.scenarios.len());

        for scenario in &suite.scenarios {
            if cancellation.is_cancelled() {
                scenarios.push(ScenarioResult::not_started(
                    scenario,
                    RunStatus::Cancelled,
                    "test.run.cancelled",
                    "run cancelled before the scenario started",
                ));
                break;
            }

            scenarios.push(
                self.run_scenario(&run_id, scenario, cancellation.clone())
                    .await,
            );

            if scenarios
                .last()
                .is_some_and(|result| result.status == RunStatus::Cancelled)
            {
                break;
            }
        }

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

        let mut session = match open {
            Ok(Ok(session)) => session,
            Ok(Err(error)) => {
                return ScenarioResult::not_started_from_driver(scenario, RunStatus::Failed, error);
            }
            Err(_) => {
                return ScenarioResult::not_started(
                    scenario,
                    RunStatus::TimedOut,
                    "test.run.timeout",
                    "scenario timed out while opening the surface",
                );
            }
        };

        let mut status = RunStatus::Passed;
        let mut steps = Vec::with_capacity(scenario.steps.len());

        for step in &scenario.steps {
            let step_started = Instant::now();
            let execution = tokio::select! {
                biased;
                () = cancellation.cancelled() => StepExecution::Cancelled,
                result = tokio::time::timeout_at(deadline.into(), session.execute(step)) => {
                    match result {
                        Ok(result) => StepExecution::Completed(result),
                        Err(_) => StepExecution::TimedOut,
                    }
                }
            };

            let step_result = match execution {
                StepExecution::Completed(Ok(output)) => {
                    StepResult::passed(&step.id, step_started.elapsed(), output)
                }
                StepExecution::Completed(Err(error)) => {
                    status = RunStatus::Failed;
                    StepResult::failed(&step.id, step_started.elapsed(), error)
                }
                StepExecution::TimedOut => {
                    status = RunStatus::TimedOut;
                    StepResult::terminal(
                        &step.id,
                        RunStatus::TimedOut,
                        step_started.elapsed(),
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
}

enum StepExecution {
    Completed(Result<StepOutput, DriverError>),
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
    pub output: Option<StepOutput>,
    pub error: Option<RunError>,
}

impl StepResult {
    fn passed(id: &str, duration: Duration, output: StepOutput) -> Self {
        Self {
            id: id.to_string(),
            status: RunStatus::Passed,
            duration_ms: millis(duration),
            output: Some(output),
            error: None,
        }
    }

    fn failed(id: &str, duration: Duration, error: DriverError) -> Self {
        Self {
            id: id.to_string(),
            status: RunStatus::Failed,
            duration_ms: millis(duration),
            output: None,
            error: Some(RunError::from_driver(error)),
        }
    }

    fn terminal(
        id: &str,
        status: RunStatus,
        duration: Duration,
        code: &str,
        message: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            status,
            duration_ms: millis(duration),
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
