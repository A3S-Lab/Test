use a3s_test_core::{
    Action, DriverError, DriverSession, RepairAclProof, RepairEvidenceBundle,
    RepairEvidenceRequest, RepairFinding, RepairHumanAction, RepairStatusEvent, ScenarioContext,
    Surface, SurfaceDriver, TestStep,
};
use a3s_test_driver_web::AgentBrowserDriver;
use async_trait::async_trait;

pub(crate) struct McpWebDriver {
    inner: AgentBrowserDriver,
    initial_url: String,
}

struct McpWebSession {
    inner: Box<dyn DriverSession>,
}

impl McpWebDriver {
    pub(crate) fn new(inner: AgentBrowserDriver, initial_url: String) -> Self {
        Self { inner, initial_url }
    }
}

#[cfg(test)]
impl McpWebDriver {
    pub(crate) fn with_executor(
        config: a3s_test_driver_web::AgentBrowserConfig,
        executor: std::sync::Arc<dyn a3s_test_driver_web::CommandExecutor>,
        initial_url: String,
    ) -> Self {
        Self::new(
            AgentBrowserDriver::with_executor(config, executor),
            initial_url,
        )
    }
}

#[async_trait]
impl SurfaceDriver for McpWebDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError> {
        let mut session = self.inner.open(context).await?;
        let step = TestStep {
            id: "mcp-initial-navigation".to_string(),
            action: Action::Navigate {
                url: self.initial_url.clone(),
            },
        };
        if let Err(error) = session.execute(&step).await {
            return match session.close().await {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(DriverError::new(
                    "test.driver.web.initial_navigation_cleanup_failed",
                    format!(
                        "initial navigation failed: {}; exact browser cleanup also failed: {}",
                        error.message(),
                        cleanup_error.message()
                    ),
                )),
            };
        }
        Ok(Box::new(McpWebSession { inner: session }))
    }
}

#[async_trait]
impl DriverSession for McpWebSession {
    async fn observe(&mut self) -> Result<a3s_test_core::SurfaceObservation, DriverError> {
        self.inner.observe().await
    }

    async fn execute(&mut self, step: &TestStep) -> Result<a3s_test_core::StepOutput, DriverError> {
        self.inner.execute(step).await
    }

    async fn take_repairs(&mut self, limit: usize) -> Result<Vec<RepairFinding>, DriverError> {
        self.inner.take_repairs(limit).await
    }

    async fn wait_for_repairs(
        &mut self,
        limit: usize,
        timeout_ms: u64,
        batch_window_ms: u64,
    ) -> Result<Vec<RepairFinding>, DriverError> {
        self.inner
            .wait_for_repairs(limit, timeout_ms, batch_window_ms)
            .await
    }

    async fn apply_repair_event(&mut self, event: &RepairStatusEvent) -> Result<(), DriverError> {
        self.inner.apply_repair_event(event).await
    }

    async fn take_repair_actions(
        &mut self,
        limit: usize,
    ) -> Result<Vec<RepairHumanAction>, DriverError> {
        self.inner.take_repair_actions(limit).await
    }

    async fn capture_repair_evidence(
        &mut self,
        request: &RepairEvidenceRequest,
    ) -> Result<RepairEvidenceBundle, DriverError> {
        self.inner.capture_repair_evidence(request).await
    }

    async fn prove_repair_acl(
        &mut self,
        finding_id: &str,
        attempt_id: &str,
        finding_url: &str,
        candidate: &str,
    ) -> Result<RepairAclProof, DriverError> {
        self.inner
            .prove_repair_acl(finding_id, attempt_id, finding_url, candidate)
            .await
    }

    async fn validate_page_context_revision(
        &mut self,
        expected_revision: u64,
    ) -> Result<(), DriverError> {
        self.inner
            .validate_page_context_revision(expected_revision)
            .await
    }

    async fn inspect_page_context(
        &mut self,
        request: &a3s_test_core::PageContextInspectRequest,
    ) -> Result<a3s_test_core::PageContextObservation, DriverError> {
        self.inner.inspect_page_context(request).await
    }

    async fn page_console_error_count(&mut self) -> Result<u32, DriverError> {
        self.inner.page_console_error_count().await
    }

    async fn page_error_count(&mut self) -> Result<u32, DriverError> {
        self.inner.page_error_count().await
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        self.inner.close().await
    }
}
