use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, ContractReport, DriverError, DriverSession, PageContextObservation, ScenarioContext,
    StepOutput, Surface, SurfaceContractDraft, SurfaceDriver, SurfaceObservation, TestScenario,
    TestStep, TestSuite,
};
use a3s_test_runner::{ContractRegistry, RunStatus, Runner, RunnerOptions};
use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

const CONTRACT: &str = r#"
surface_contract "checkout" {
    context {
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }
    provenance "requirements" {
        kind = "prd"
        uri = "./checkout.md"
        digest = "sha256:56ea72bad66743f4dadee9515096bb39a200bf9ca8d5669293f41912c55ec14e"
        status = "reviewed"
        confidence = 100
    }
    variant "desktop" {
        state = "ready"
        element "submit" {
            test_id = "place-order"
            role = "button"
            name = "Place order"
            severity = "blocking"
        }
    }
}
"#;

#[derive(Clone)]
struct ObservationDriver {
    observation: SurfaceObservation,
    executions: Arc<AtomicUsize>,
    observations: Arc<AtomicUsize>,
    projections: Arc<AtomicUsize>,
    projection_fails: bool,
    projection_hangs: bool,
}

struct ObservationSession {
    observation: SurfaceObservation,
    executions: Arc<AtomicUsize>,
    observations: Arc<AtomicUsize>,
    projections: Arc<AtomicUsize>,
    projection_fails: bool,
    projection_hangs: bool,
}

struct ProjectionBehavior {
    fails: bool,
    hangs: bool,
    timeout: Duration,
}

impl Default for ProjectionBehavior {
    fn default() -> Self {
        Self {
            fails: false,
            hangs: false,
            timeout: RunnerOptions::default().quality_projection_timeout,
        }
    }
}

#[async_trait]
impl SurfaceDriver for ObservationDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(ObservationSession {
            observation: self.observation.clone(),
            executions: Arc::clone(&self.executions),
            observations: Arc::clone(&self.observations),
            projections: Arc::clone(&self.projections),
            projection_fails: self.projection_fails,
            projection_hangs: self.projection_hangs,
        }))
    }
}

#[async_trait]
impl DriverSession for ObservationSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        self.observations.fetch_add(1, Ordering::SeqCst);
        Ok(self.observation.clone())
    }

    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(StepOutput::new("driver action"))
    }

    async fn project_quality_report(
        &mut self,
        _report: &ContractReport,
    ) -> Result<bool, DriverError> {
        self.projections.fetch_add(1, Ordering::SeqCst);
        if self.projection_hangs {
            return std::future::pending().await;
        }
        if self.projection_fails {
            Err(DriverError::new(
                "test.driver.quality_projection_failed",
                "planned quality projection failure",
            ))
        } else {
            Ok(true)
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn runner_observes_and_reconciles_a_contract_without_dispatching_it_to_the_driver() {
    let executions = Arc::new(AtomicUsize::new(0));
    let observations = Arc::new(AtomicUsize::new(0));
    let projections = Arc::new(AtomicUsize::new(0));
    let runner = runner(
        matching_observation("button"),
        Arc::clone(&executions),
        Arc::clone(&observations),
        Arc::clone(&projections),
        false,
    );

    let result = runner.run(&suite(), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(observations.load(Ordering::SeqCst), 1);
    assert_eq!(projections.load(Ordering::SeqCst), 1);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(step.status, RunStatus::Passed);
    assert_eq!(
        step.output.as_ref().expect("contract report").data["outcome"],
        "passed"
    );
}

#[tokio::test]
async fn runner_preserves_the_contract_report_when_a_mismatch_fails_the_step() {
    let executions = Arc::new(AtomicUsize::new(0));
    let observations = Arc::new(AtomicUsize::new(0));
    let projections = Arc::new(AtomicUsize::new(0));
    let runner = runner(
        matching_observation("link"),
        Arc::clone(&executions),
        Arc::clone(&observations),
        Arc::clone(&projections),
        false,
    );

    let result = runner.run(&suite(), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(observations.load(Ordering::SeqCst), 1);
    assert_eq!(projections.load(Ordering::SeqCst), 1);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(
        step.error.as_ref().map(|error| error.code.as_str()),
        Some("test.contract.mismatch")
    );
    let report = &step.output.as_ref().expect("failed report retained").data;
    assert_eq!(report["outcome"], "failed");
    assert_eq!(report["findings"][0]["rule_id"], "contract.element.role");
    assert_eq!(report["findings"][0]["expected"], "button");
    assert_eq!(report["findings"][0]["actual"], "link");
}

#[tokio::test]
async fn runner_fails_closed_when_the_referenced_contract_was_not_admitted() {
    let observations = Arc::new(AtomicUsize::new(0));
    let driver = ObservationDriver {
        observation: matching_observation("button"),
        executions: Arc::new(AtomicUsize::new(0)),
        observations: Arc::clone(&observations),
        projections: Arc::new(AtomicUsize::new(0)),
        projection_fails: false,
        projection_hangs: false,
    };
    let runner = Runner::new(vec![Arc::new(driver)], RunnerOptions::default()).expect("runner");

    let result = runner.run(&suite(), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(observations.load(Ordering::SeqCst), 0);
    assert_eq!(
        result.scenarios[0].steps[0]
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("test.run.contract_missing")
    );
}

#[tokio::test]
async fn advisory_findings_are_reported_without_failing_the_step() {
    let projections = Arc::new(AtomicUsize::new(0));
    let runner = runner_with_contract(
        &CONTRACT.replace("severity = \"blocking\"", "severity = \"important\""),
        matching_observation("link"),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        Arc::clone(&projections),
        ProjectionBehavior::default(),
    );

    let result = runner.run(&suite(), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(projections.load(Ordering::SeqCst), 1);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(step.status, RunStatus::Passed);
    assert_eq!(
        step.output.as_ref().expect("advisory report").summary,
        "surface contract passed with advisory findings"
    );
    let report = &step.output.as_ref().expect("advisory report").data;
    assert_eq!(report["outcome"], "passed");
    assert_eq!(report["findings"][0]["severity"], "important");
}

#[tokio::test]
async fn quality_projection_failure_does_not_change_the_verdict_or_report() {
    let projections = Arc::new(AtomicUsize::new(0));
    let runner = runner(
        matching_observation("link"),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        Arc::clone(&projections),
        true,
    );

    let result = runner.run(&suite(), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(projections.load(Ordering::SeqCst), 1);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(
        step.error.as_ref().map(|error| error.code.as_str()),
        Some("test.contract.mismatch")
    );
    assert_eq!(
        step.output.as_ref().expect("failed report retained").data["outcome"],
        "failed"
    );
}

#[tokio::test]
async fn hanging_quality_projection_is_bounded_without_changing_the_verdict_or_report() {
    let projections = Arc::new(AtomicUsize::new(0));
    let runner = runner_with_projection(
        matching_observation("button"),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        Arc::clone(&projections),
        ProjectionBehavior {
            hangs: true,
            timeout: Duration::from_millis(10),
            ..ProjectionBehavior::default()
        },
    );

    let result = runner.run(&suite(), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(projections.load(Ordering::SeqCst), 1);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(step.status, RunStatus::Passed);
    assert_eq!(
        step.output.as_ref().expect("contract report").data["outcome"],
        "passed"
    );
}

#[tokio::test]
async fn inconclusive_contract_reports_are_projected_before_the_step_fails_closed() {
    let projections = Arc::new(AtomicUsize::new(0));
    let runner = runner(
        SurfaceObservation::new("observation without page context"),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        Arc::clone(&projections),
        false,
    );

    let result = runner.run(&suite(), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(projections.load(Ordering::SeqCst), 1);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(
        step.error.as_ref().map(|error| error.code.as_str()),
        Some("test.contract.inconclusive")
    );
    assert_eq!(
        step.output.as_ref().expect("inconclusive report").data["outcome"],
        "inconclusive"
    );
}

fn runner(
    observation: SurfaceObservation,
    executions: Arc<AtomicUsize>,
    observations: Arc<AtomicUsize>,
    projections: Arc<AtomicUsize>,
    projection_fails: bool,
) -> Runner {
    runner_with_projection(
        observation,
        executions,
        observations,
        projections,
        ProjectionBehavior {
            fails: projection_fails,
            ..ProjectionBehavior::default()
        },
    )
}

fn runner_with_projection(
    observation: SurfaceObservation,
    executions: Arc<AtomicUsize>,
    observations: Arc<AtomicUsize>,
    projections: Arc<AtomicUsize>,
    projection: ProjectionBehavior,
) -> Runner {
    runner_with_contract(
        CONTRACT,
        observation,
        executions,
        observations,
        projections,
        projection,
    )
}

fn runner_with_contract(
    contract_source: &str,
    observation: SurfaceObservation,
    executions: Arc<AtomicUsize>,
    observations: Arc<AtomicUsize>,
    projections: Arc<AtomicUsize>,
    projection: ProjectionBehavior,
) -> Runner {
    let contract = SurfaceContractDraft::from_acl(contract_source)
        .expect("contract syntax")
        .admit()
        .expect("contract admission");
    let contracts =
        ContractRegistry::new([("./contracts/checkout.acl", contract)]).expect("contract registry");
    Runner::new(
        vec![Arc::new(ObservationDriver {
            observation,
            executions,
            observations,
            projections,
            projection_fails: projection.fails,
            projection_hangs: projection.hangs,
        })],
        RunnerOptions {
            quality_projection_timeout: projection.timeout,
            ..RunnerOptions::default()
        },
    )
    .expect("runner")
    .with_contracts(contracts)
}

fn suite() -> TestSuite {
    TestSuite {
        name: "contract-run".to_string(),
        version: 1,
        scenarios: vec![TestScenario {
            id: "checkout".to_string(),
            name: "Checkout".to_string(),
            surface: Surface::Web,
            timeout_ms: 1_000,
            steps: vec![TestStep {
                id: "verify-ready".to_string(),
                action: Action::VerifyContract {
                    contract: "./contracts/checkout.acl".to_string(),
                    variant: "desktop".to_string(),
                    state: "ready".to_string(),
                },
                stability: None,
                assertion_mode: Default::default(),
                wait_mode: Default::default(),
            }],
        }],
    }
}

fn matching_observation(role: &str) -> SurfaceObservation {
    let context: PageContextObservation = serde_json::from_value(json!({
        "present": true,
        "protocol": "a3s.test.page-context/1",
        "sdk_version": "0.2.0",
        "revision": 4,
        "snapshot": {
            "protocol": "a3s.test.page-context/1",
            "sdkVersion": "0.2.0",
            "revision": 4,
            "page": {
                "id": "checkout",
                "url": "https://example.test/checkout",
                "route": "/checkout",
                "title": "Checkout",
                "ready": true,
                "viewport": { "width": 1280.0, "height": 800.0, "dpr": 2.0 },
                "document": { "width": 1280.0, "height": 1200.0 },
                "scroll": { "x": 0.0, "y": 0.0 },
                "language": "en",
                "theme": "light"
            },
            "components": [],
            "nodes": [{
                "id": "submit-node",
                "tag": "button",
                "role": role,
                "name": "Place order",
                "testId": "place-order",
                "state": { "visible": true, "disabled": false },
                "locators": []
            }],
            "facts": {},
            "removedNodeIds": [],
            "truncated": false,
            "nextCursor": null
        }
    }))
    .expect("typed page context");
    SurfaceObservation::new("atomic observation")
        .with_data(json!({ "snapshot": "@e1 [button] Place order" }))
        .with_page_context(context)
}
