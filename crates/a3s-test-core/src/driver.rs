use std::path::PathBuf;

use async_trait::async_trait;

use crate::{
    ContractReport, DriverError, PageContextObservation, RepairAclProof, RepairEvidenceBundle,
    RepairEvidenceRequest, RepairFinding, RepairHumanAction, RepairStatusEvent, StepOutput,
    Surface, SurfaceObservation, TestStep,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageContextInspectScope {
    Page,
    Node(String),
    Component(String),
    Region {
        space: String,
        x: i64,
        y: i64,
        width: u64,
        height: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageContextInspectRequest {
    pub detail: String,
    pub scope: PageContextInspectScope,
    pub cursor: Option<String>,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioContext {
    pub run_id: String,
    pub scenario_id: String,
    pub artifacts_dir: PathBuf,
}

#[async_trait]
pub trait DriverSession: Send {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        Err(DriverError::new(
            "test.driver.observation_unsupported",
            "this surface driver does not expose agent observations",
        ))
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError>;

    async fn take_repairs(&mut self, _limit: usize) -> Result<Vec<RepairFinding>, DriverError> {
        Err(DriverError::new(
            "test.driver.repair_unsupported",
            "this surface driver does not expose a repair queue",
        ))
    }

    async fn wait_for_repairs(
        &mut self,
        _limit: usize,
        _timeout_ms: u64,
        _batch_window_ms: u64,
    ) -> Result<Vec<RepairFinding>, DriverError> {
        Err(DriverError::new(
            "test.driver.repair_watch_unsupported",
            "this surface driver does not support bounded repair watching",
        ))
    }

    async fn apply_repair_event(&mut self, _event: &RepairStatusEvent) -> Result<(), DriverError> {
        Err(DriverError::new(
            "test.driver.repair_unsupported",
            "this surface driver does not project repair status",
        ))
    }

    /// Project a deterministic quality report into an optional human-review UI.
    ///
    /// This advisory projection must never change the runner verdict. Drivers
    /// without a compatible embedded surface return `Ok(false)`.
    async fn project_quality_report(
        &mut self,
        _report: &ContractReport,
    ) -> Result<bool, DriverError> {
        Ok(false)
    }

    async fn take_repair_actions(
        &mut self,
        _limit: usize,
    ) -> Result<Vec<RepairHumanAction>, DriverError> {
        Ok(Vec::new())
    }

    async fn capture_repair_evidence(
        &mut self,
        _request: &RepairEvidenceRequest,
    ) -> Result<RepairEvidenceBundle, DriverError> {
        Err(DriverError::new(
            "test.driver.repair_evidence_unsupported",
            "this surface driver cannot capture A3S-owned repair evidence",
        ))
    }

    async fn prove_repair_acl(
        &mut self,
        _finding_id: &str,
        _attempt_id: &str,
        _finding_url: &str,
        _candidate: &str,
    ) -> Result<RepairAclProof, DriverError> {
        Err(DriverError::new(
            "test.driver.repair_acl_proof_unsupported",
            "this surface driver cannot prove repair ACL in a fresh browser session",
        ))
    }

    async fn validate_page_context_revision(
        &mut self,
        _expected_revision: u64,
    ) -> Result<(), DriverError> {
        Err(DriverError::new(
            "test.driver.page_context_validation_unsupported",
            "this surface driver cannot validate a page context revision",
        ))
    }

    async fn inspect_page_context(
        &mut self,
        _request: &PageContextInspectRequest,
    ) -> Result<PageContextObservation, DriverError> {
        Err(DriverError::new(
            "test.driver.page_context_inspect_unsupported",
            "this surface driver does not support scoped page-context inspection",
        ))
    }

    async fn page_console_error_count(&mut self) -> Result<u32, DriverError> {
        Err(DriverError::new(
            "test.driver.console_count_unsupported",
            "this surface driver does not expose a bounded console error count",
        ))
    }

    async fn page_error_count(&mut self) -> Result<u32, DriverError> {
        Err(DriverError::new(
            "test.driver.page_error_count_unsupported",
            "this surface driver does not expose a bounded page error count",
        ))
    }

    async fn close(&mut self) -> Result<(), DriverError>;
}

#[async_trait]
pub trait SurfaceDriver: Send + Sync {
    fn surface(&self) -> Surface;

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError>;
}
