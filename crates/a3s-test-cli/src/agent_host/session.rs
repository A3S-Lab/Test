use a3s_test_agent::{ActionPolicy, AgentError, PolicyContext};
use a3s_test_core::{
    Action, DriverError, DriverSession, GroundingScreenshot, PageContextInspectRequest,
    PageContextObservation, RepairAclProof, RepairEvidenceBundle, RepairEvidenceRequest,
    RepairFinding, RepairHumanAction, RepairStatusEvent, StepOutput, SurfaceObservation, TestStep,
};
use async_trait::async_trait;
use url::Url;

use super::observed_web_url;

pub(super) struct AgentHostSession {
    inner: Box<dyn DriverSession>,
    allowed_origins: Vec<Url>,
}

impl AgentHostSession {
    pub(super) fn new(inner: Box<dyn DriverSession>, allowed_origins: Vec<Url>) -> Self {
        Self {
            inner,
            allowed_origins,
        }
    }

    fn validate_observation(&self, observation: &SurfaceObservation) -> Result<(), DriverError> {
        let value = observed_web_url(&observation.data).ok_or_else(|| {
            DriverError::new(
                "test.driver.web.output_invalid",
                "browser observation did not report its page URL",
            )
        })?;
        let parsed = Url::parse(value).map_err(|_| {
            DriverError::new(
                "test.driver.web.session_origin_lost",
                "browser observation returned an invalid page URL",
            )
        })?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(DriverError::new(
                "test.driver.web.session_origin_lost",
                format!(
                    "browser session left its Web page and reported the '{}' scheme",
                    parsed.scheme()
                ),
            ));
        }
        if self
            .allowed_origins
            .iter()
            .all(|allowed| allowed.origin() != parsed.origin())
        {
            return Err(DriverError::new(
                "test.driver.web.navigation_origin_denied",
                format!(
                    "browser observed navigation to unapproved origin '{}'",
                    parsed.origin().ascii_serialization()
                ),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl DriverSession for AgentHostSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        let observation = self.inner.observe().await?;
        self.validate_observation(&observation)?;
        Ok(observation)
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
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

    async fn page_context_delta(
        &mut self,
        since_revision: u64,
    ) -> Result<Option<PageContextObservation>, DriverError> {
        self.inner.page_context_delta(since_revision).await
    }

    async fn inspect_page_context(
        &mut self,
        request: &PageContextInspectRequest,
    ) -> Result<PageContextObservation, DriverError> {
        self.inner.inspect_page_context(request).await
    }

    async fn page_console_error_count(&mut self) -> Result<u32, DriverError> {
        self.inner.page_console_error_count().await
    }

    async fn page_error_count(&mut self) -> Result<u32, DriverError> {
        self.inner.page_error_count().await
    }

    async fn capture_grounding_screenshot(
        &mut self,
        requested_path: &str,
        expected_surface_revision: Option<u64>,
    ) -> Result<GroundingScreenshot, DriverError> {
        self.inner
            .capture_grounding_screenshot(requested_path, expected_surface_revision)
            .await
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        self.inner.close().await
    }
}

pub(super) struct OriginObservationPolicy {
    inner: a3s_test_agent::CapabilityPolicy,
    allowed_origins: Vec<Url>,
}

impl OriginObservationPolicy {
    pub(super) fn new(
        allowed_actions: Vec<a3s_test_agent::ActionKind>,
        allowed_origins: Vec<Url>,
    ) -> Self {
        Self {
            inner: a3s_test_agent::CapabilityPolicy::new(
                allowed_actions,
                a3s_test_agent::NavigationScope::Origins(allowed_origins.clone()),
            ),
            allowed_origins,
        }
    }
}

impl ActionPolicy for OriginObservationPolicy {
    fn validate(&self, context: &PolicyContext<'_>, action: &Action) -> Result<(), AgentError> {
        let current = observed_web_url(&context.observation.data).ok_or_else(|| {
            AgentError::new(
                "test.agent.policy.observation_url_missing",
                "current Web observation did not report its page URL",
            )
        })?;
        let current = Url::parse(current).map_err(|_| {
            AgentError::new(
                "test.agent.policy.observation_url_invalid",
                "current Web observation reported an invalid page URL",
            )
        })?;
        if self
            .allowed_origins
            .iter()
            .all(|allowed| allowed.origin() != current.origin())
        {
            return Err(AgentError::new(
                "test.agent.policy.observation_origin_denied",
                "current Web observation is outside the allowed origin set",
            ));
        }
        self.inner.validate(context, action)
    }
}
