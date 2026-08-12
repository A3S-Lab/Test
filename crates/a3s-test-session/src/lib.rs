//! Surface-neutral application services for long-lived agent test sessions.
//!
//! Protocol projections such as MCP call this layer. It owns session identity,
//! observation-bound refs, serialization of turns, and bounded cleanup while
//! delegating all surface behavior to `SurfaceDriver` objects.

mod manager_repair;
mod page_context;
mod protocol;
mod repair;
mod verification;
mod workspace_repair;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use a3s_test_core::{
    DriverError, DriverSession, PageContextInspectRequest, RepairActor, RepairBatch,
    RepairEvidencePhase, RepairEvidenceRequest, RepairFinding, RepairStatus, RepairStatusEvent,
    ScenarioContext, Surface, SurfaceDriver, SurfaceObservation, TestStep,
};
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tokio::task::JoinHandle;

static MANAGER_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub use page_context::{
    action_uses_observation_target, action_uses_page_context_ref, bind_page_context_refs,
    preferred_page_context_target, resolve_page_context_refs, PageContextBindings,
};
use protocol::{validate_session_id, validate_start};
pub use protocol::{
    ActSessionRequest, ActionResult, FinishSessionRequest, ObservationResult, RepairVerifyRequest,
    SessionError, SessionFailure, SessionFinishStatus, SessionFinished, SessionStarted,
    StartSessionRequest,
};
pub use repair::{RepairEventRecord, RepairLedger, RepairRecord, RepairTransition};
pub use verification::{build_repair_verification, validate_repair_verification_request};
pub use workspace_repair::{RepairWorkspace, RepairWorkspaceLock};

#[derive(Clone, Debug)]
pub struct SessionManagerOptions {
    pub artifacts_root: PathBuf,
    pub cleanup_timeout: Duration,
    pub max_sessions: usize,
}

impl SessionManagerOptions {
    fn validate(&self) -> Result<(), SessionError> {
        if self.artifacts_root.as_os_str().is_empty() {
            return Err(SessionError::new(
                "test.session.config_invalid",
                "session artifact root must not be empty",
            ));
        }
        if self.cleanup_timeout.is_zero() {
            return Err(SessionError::new(
                "test.session.config_invalid",
                "session cleanup timeout must be greater than zero",
            ));
        }
        if !(1..=64).contains(&self.max_sessions) {
            return Err(SessionError::new(
                "test.session.config_invalid",
                "maximum active sessions must be between 1 and 64",
            ));
        }
        Ok(())
    }
}

pub struct AgentSessionManager {
    drivers: HashMap<Surface, Arc<dyn SurfaceDriver>>,
    options: SessionManagerOptions,
    registry: Arc<StdMutex<SessionRegistry>>,
    cleanup_notify: Arc<Notify>,
    run_namespace: String,
    repair_workspace: RepairWorkspace,
}

#[derive(Default)]
struct SessionRegistry {
    opening: HashSet<String>,
    closing: HashSet<String>,
    sessions: HashMap<String, Arc<AsyncMutex<ManagedSession>>>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ManagedSessionLifecycle {
    Active,
    CleanupRequired,
}

struct ManagedSession {
    id: String,
    surface: Surface,
    goal: String,
    success_criteria: Vec<String>,
    auto_resolve_repairs: bool,
    driver: Box<dyn DriverSession>,
    next_turn: u64,
    next_observation: u64,
    latest_observation: Option<u64>,
    page_context_targets: PageContextBindings,
    repair_ledger: RepairLedger,
    started_at_ms: u64,
    lifecycle: ManagedSessionLifecycle,
}

impl AgentSessionManager {
    pub fn new(
        drivers: Vec<Arc<dyn SurfaceDriver>>,
        options: SessionManagerOptions,
    ) -> Result<Self, SessionError> {
        options.validate()?;
        let mut by_surface = HashMap::new();
        for driver in drivers {
            let surface = driver.surface();
            if by_surface.insert(surface, driver).is_some() {
                return Err(SessionError::new(
                    "test.session.driver_duplicate",
                    format!("multiple drivers were registered for {surface:?}"),
                ));
            }
        }
        if by_surface.is_empty() {
            return Err(SessionError::new(
                "test.session.driver_missing",
                "at least one surface driver must be registered",
            ));
        }
        let sequence = MANAGER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let repair_workspace = RepairWorkspace::from_artifacts_root(&options.artifacts_root);
        Ok(Self {
            drivers: by_surface,
            options,
            registry: Arc::new(StdMutex::new(SessionRegistry::default())),
            cleanup_notify: Arc::new(Notify::new()),
            run_namespace: format!("agent-{}-{sequence}", std::process::id()),
            repair_workspace,
        })
    }

    pub async fn start(
        &self,
        request: StartSessionRequest,
    ) -> Result<SessionStarted, SessionError> {
        validate_start(&request)?;
        let driver = Arc::clone(self.drivers.get(&request.surface).ok_or_else(|| {
            SessionError::new(
                "test.session.driver_missing",
                format!("no driver is registered for {:?}", request.surface),
            )
        })?);
        {
            let mut registry = self.lock_registry();
            if registry.sessions.contains_key(&request.session)
                || registry.closing.contains(&request.session)
                || !registry.opening.insert(request.session.clone())
            {
                return Err(SessionError::new(
                    "test.session.already_exists",
                    format!("session '{}' already exists", request.session),
                ));
            }
            if registry.sessions.len() + registry.opening.len() + registry.closing.len()
                > self.options.max_sessions
            {
                registry.opening.remove(&request.session);
                return Err(SessionError::new(
                    "test.session.capacity_exceeded",
                    "active session capacity has been reached",
                ));
            }
        }

        let mut reservation = OpeningReservation::new(self, request.session.clone());

        let context = ScenarioContext {
            run_id: self.run_namespace.clone(),
            scenario_id: request.session.clone(),
            artifacts_dir: self.options.artifacts_root.join(&request.session),
        };
        let opened = driver.open(&context).await;
        let driver = opened.map_err(SessionError::from_driver)?;
        let started_at_ms = unix_ms();
        let response = SessionStarted {
            session: request.session.clone(),
            surface: request.surface,
            goal: request.goal.clone(),
            success_criteria: request.success_criteria.clone(),
            auto_resolve_repairs: request.auto_resolve_repairs,
            started_at_ms,
        };
        let repair_ledger = RepairLedger::load(context.artifacts_dir.join("repairs.jsonl")).await?;
        let mut registry = self.lock_registry();
        reservation.release(&mut registry);
        registry.sessions.insert(
            request.session.clone(),
            Arc::new(AsyncMutex::new(ManagedSession {
                id: request.session,
                surface: request.surface,
                goal: request.goal,
                success_criteria: request.success_criteria,
                auto_resolve_repairs: request.auto_resolve_repairs,
                driver,
                next_turn: 1,
                next_observation: 1,
                latest_observation: None,
                page_context_targets: PageContextBindings::default(),
                repair_ledger,
                started_at_ms,
                lifecycle: ManagedSessionLifecycle::Active,
            })),
        );
        Ok(response)
    }

    #[must_use]
    pub fn surfaces(&self) -> Vec<Surface> {
        [Surface::Web, Surface::Gui, Surface::Tui]
            .into_iter()
            .filter(|surface| self.drivers.contains_key(surface))
            .collect()
    }

    pub async fn observe(&self, session: &str) -> Result<ObservationResult, SessionError> {
        let managed = self.get(session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        managed.latest_observation = None;
        managed.page_context_targets = PageContextBindings::default();
        let mut observation = managed
            .driver
            .observe()
            .await
            .map_err(SessionError::from_driver)?;
        let observation_id = managed.next_observation;
        managed.next_observation = managed.next_observation.checked_add(1).ok_or_else(|| {
            SessionError::new(
                "test.session.observation_limit_reached",
                "session observation sequence overflowed",
            )
        })?;
        managed.page_context_targets = bind_page_context_refs(&mut observation);
        managed.latest_observation = Some(observation_id);
        managed.next_turn = managed.next_turn.saturating_add(1);
        Ok(ObservationResult {
            session: managed.id.clone(),
            observation_id,
            observation,
        })
    }

    pub async fn act(&self, request: ActSessionRequest) -> Result<ActionResult, SessionError> {
        let managed = self.get(&request.session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        if action_uses_observation_target(&request.action) {
            let latest = managed.latest_observation.ok_or_else(|| {
                SessionError::new(
                    "test.session.observation_required",
                    "observation-bound targets require a fresh observation",
                )
            })?;
            if request.observation_id != Some(latest) {
                return Err(SessionError::new(
                    "test.session.stale_observation",
                    format!("target belongs to observation {latest}"),
                ));
            }
        }
        let expected_revision = if action_uses_page_context_ref(&request.action) {
            Some(managed.page_context_targets.revision.ok_or_else(|| {
                SessionError::new(
                    "test.session.context_revision_missing",
                    "page context ref is missing its observation revision",
                )
            })?)
        } else {
            None
        };
        managed.latest_observation = None;
        let bindings = std::mem::take(&mut managed.page_context_targets);
        if let Some(revision) = expected_revision {
            managed
                .driver
                .validate_page_context_revision(revision)
                .await
                .map_err(SessionError::from_driver)?;
        }
        let action = resolve_page_context_refs(request.action, &bindings)?;
        let step = TestStep {
            id: format!("agent-turn-{}", managed.next_turn),
            action,
        };
        managed.next_turn = managed.next_turn.saturating_add(1);
        let output = managed
            .driver
            .execute(&step)
            .await
            .map_err(SessionError::from_driver)?;
        Ok(ActionResult {
            session: managed.id.clone(),
            output,
        })
    }

    pub async fn inspect_page_context(
        &self,
        session: &str,
        request: PageContextInspectRequest,
    ) -> Result<ObservationResult, SessionError> {
        let managed = self.get(session).await?;
        let mut managed = managed.lock().await;
        ensure_active(&managed)?;
        managed.latest_observation = None;
        managed.page_context_targets = PageContextBindings::default();
        let page_context = managed
            .driver
            .inspect_page_context(&request)
            .await
            .map_err(SessionError::from_driver)?;
        let mut observation = SurfaceObservation::new("scoped page context inspected")
            .with_page_context(page_context);
        let observation_id = managed.next_observation;
        managed.next_observation = managed.next_observation.checked_add(1).ok_or_else(|| {
            SessionError::new(
                "test.session.observation_limit_reached",
                "session observation sequence overflowed",
            )
        })?;
        managed.page_context_targets = bind_page_context_refs(&mut observation);
        managed.latest_observation = Some(observation_id);
        managed.next_turn = managed.next_turn.saturating_add(1);
        Ok(ObservationResult {
            session: managed.id.clone(),
            observation_id,
            observation,
        })
    }

    pub async fn finish(
        &self,
        request: FinishSessionRequest,
    ) -> Result<SessionFinished, SessionError> {
        if request.summary.trim().is_empty() {
            return Err(SessionError::new(
                "test.session.summary_invalid",
                "session summary must not be empty",
            ));
        }
        let managed = {
            let mut registry = self.lock_registry();
            if registry.closing.contains(&request.session) {
                return Err(cleanup_in_progress(&request.session));
            }
            let managed = registry.sessions.remove(&request.session).ok_or_else(|| {
                SessionError::new(
                    "test.session.not_found",
                    format!("session '{}' is not active", request.session),
                )
            })?;
            registry.closing.insert(request.session.clone());
            managed
        };
        let mut reservation = ClosingReservation::new(self, request.session.clone(), &managed);
        let (session, surface, goal, success_criteria, turns, started_at_ms) = {
            let mut managed = managed.lock().await;
            let mut workspace = self.repair_workspace.acquire().await?;
            let session_id = managed.id.clone();
            managed
                .repair_ledger
                .interrupt_active_mutation_in_workspace(&session_id, unix_ms(), &mut workspace)
                .await?;
            managed.lifecycle = ManagedSessionLifecycle::CleanupRequired;
            (
                managed.id.clone(),
                managed.surface,
                managed.goal.clone(),
                managed.success_criteria.clone(),
                managed.next_turn.saturating_sub(1),
                managed.started_at_ms,
            )
        };
        let mut cleanup = reservation.spawn_cleanup();
        let cleanup_error = match tokio::time::timeout(self.options.cleanup_timeout, cleanup.wait())
            .await
        {
            Ok(result) => {
                let failure = result.err().map(SessionFailure::from_driver);
                cleanup.finalize(failure.as_ref().is_some_and(|failure| failure.retryable));
                failure
            }
            Err(_) => Some(SessionFailure {
                code: "test.session.cleanup_timeout".to_string(),
                message:
                    "surface cleanup exceeded the caller deadline and continues in the background"
                        .to_string(),
                retryable: true,
            }),
        };
        let status = if cleanup_error.is_some() {
            SessionFinishStatus::Failed
        } else {
            request.status
        };
        let finished = SessionFinished {
            session,
            surface,
            goal,
            success_criteria,
            status,
            summary: request.summary,
            turns,
            started_at_ms,
            finished_at_ms: unix_ms(),
            cleanup_error,
        };
        Ok(finished)
    }

    pub async fn abort(&self, session: &str) -> Result<SessionFinished, SessionError> {
        self.finish(FinishSessionRequest {
            session: session.to_string(),
            status: SessionFinishStatus::Aborted,
            summary: "Session aborted by the caller".to_string(),
        })
        .await
    }

    pub async fn close_all(&self) -> Vec<SessionFinished> {
        let sessions = {
            let registry = self.lock_registry();
            registry.sessions.keys().cloned().collect::<Vec<_>>()
        };
        let results = futures::future::join_all(sessions.iter().map(|session| self.abort(session)))
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect();
        self.wait_for_background_cleanup().await;
        results
    }

    async fn get(&self, session: &str) -> Result<Arc<AsyncMutex<ManagedSession>>, SessionError> {
        validate_session_id(session)?;
        let registry = self.lock_registry();
        if registry.closing.contains(session) {
            return Err(cleanup_in_progress(session));
        }
        registry.sessions.get(session).cloned().ok_or_else(|| {
            SessionError::new(
                "test.session.not_found",
                format!("session '{session}' is not active"),
            )
        })
    }

    fn lock_registry(&self) -> MutexGuard<'_, SessionRegistry> {
        self.registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn wait_for_background_cleanup(&self) {
        let deadline = tokio::time::Instant::now() + self.options.cleanup_timeout;
        loop {
            if self.lock_registry().closing.is_empty() {
                return;
            }
            if tokio::time::timeout_at(deadline, self.cleanup_notify.notified())
                .await
                .is_err()
            {
                return;
            }
        }
    }
}

struct OpeningReservation<'a> {
    manager: &'a AgentSessionManager,
    session: String,
    active: bool,
}

struct ClosingReservation<'a> {
    manager: &'a AgentSessionManager,
    session: String,
    managed: Arc<AsyncMutex<ManagedSession>>,
    active: bool,
}

impl<'a> ClosingReservation<'a> {
    fn new(
        manager: &'a AgentSessionManager,
        session: String,
        managed: &Arc<AsyncMutex<ManagedSession>>,
    ) -> Self {
        Self {
            manager,
            session,
            managed: Arc::clone(managed),
            active: true,
        }
    }

    fn restore(&mut self) {
        let mut registry = self.manager.lock_registry();
        registry.closing.remove(&self.session);
        registry
            .sessions
            .insert(self.session.clone(), Arc::clone(&self.managed));
        self.active = false;
        self.manager.cleanup_notify.notify_one();
    }

    fn spawn_cleanup(&mut self) -> CleanupTaskGuard {
        let managed = Arc::clone(&self.managed);
        let task_managed = Arc::clone(&managed);
        let task = tokio::spawn(async move {
            let mut managed = task_managed.lock().await;
            managed.driver.close().await
        });
        self.active = false;
        CleanupTaskGuard {
            registry: Arc::clone(&self.manager.registry),
            cleanup_notify: Arc::clone(&self.manager.cleanup_notify),
            session: self.session.clone(),
            managed,
            task: Some(task),
        }
    }
}

impl Drop for ClosingReservation<'_> {
    fn drop(&mut self) {
        if self.active {
            self.restore();
        }
    }
}

struct CleanupTaskGuard {
    registry: Arc<StdMutex<SessionRegistry>>,
    cleanup_notify: Arc<Notify>,
    session: String,
    managed: Arc<AsyncMutex<ManagedSession>>,
    task: Option<JoinHandle<Result<(), DriverError>>>,
}

impl CleanupTaskGuard {
    async fn wait(&mut self) -> Result<(), DriverError> {
        match self.task.as_mut() {
            Some(task) => task
                .await
                .unwrap_or_else(|error| Err(cleanup_task_error(error))),
            None => Err(DriverError::new(
                "test.session.cleanup_task_missing",
                "surface cleanup task is unavailable",
            )),
        }
    }

    fn finalize(&mut self, restore: bool) {
        finalize_cleanup(
            &self.registry,
            &self.cleanup_notify,
            &self.session,
            &self.managed,
            restore,
        );
        self.task.take();
    }
}

impl Drop for CleanupTaskGuard {
    fn drop(&mut self) {
        let Some(task) = self.task.take() else {
            return;
        };
        let registry = Arc::clone(&self.registry);
        let cleanup_notify = Arc::clone(&self.cleanup_notify);
        let session = self.session.clone();
        let managed = Arc::clone(&self.managed);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let result = task
                    .await
                    .unwrap_or_else(|error| Err(cleanup_task_error(error)));
                let restore = result.as_ref().is_err_and(DriverError::retryable);
                finalize_cleanup(&registry, &cleanup_notify, &session, &managed, restore);
            });
        } else {
            finalize_cleanup(&registry, &cleanup_notify, &session, &managed, true);
        }
    }
}

fn finalize_cleanup(
    registry: &StdMutex<SessionRegistry>,
    cleanup_notify: &Notify,
    session: &str,
    managed: &Arc<AsyncMutex<ManagedSession>>,
    restore: bool,
) {
    let mut registry = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    registry.closing.remove(session);
    if restore {
        registry
            .sessions
            .insert(session.to_string(), Arc::clone(managed));
    }
    cleanup_notify.notify_one();
}

fn cleanup_task_error(error: tokio::task::JoinError) -> DriverError {
    DriverError::new(
        "test.session.cleanup_task_failed",
        format!("surface cleanup task failed: {error}"),
    )
    .with_retryable(true)
}

impl<'a> OpeningReservation<'a> {
    fn new(manager: &'a AgentSessionManager, session: String) -> Self {
        Self {
            manager,
            session,
            active: true,
        }
    }

    fn release(&mut self, registry: &mut SessionRegistry) {
        registry.opening.remove(&self.session);
        self.active = false;
    }
}

impl Drop for OpeningReservation<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.manager.lock_registry().opening.remove(&self.session);
    }
}

fn ensure_active(session: &ManagedSession) -> Result<(), SessionError> {
    if session.lifecycle == ManagedSessionLifecycle::CleanupRequired {
        return Err(SessionError::new(
            "test.session.cleanup_required",
            "session cleanup must be retried with finish or abort before another turn",
        ));
    }
    Ok(())
}

fn cleanup_in_progress(session: &str) -> SessionError {
    SessionError::new(
        "test.session.cleanup_in_progress",
        format!("session '{session}' cleanup is already in progress"),
    )
    .with_retryable(true)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

fn auto_resolution_request_id(verification_request_id: &str) -> String {
    let prefix = verification_request_id
        .chars()
        .take(115)
        .collect::<String>();
    format!("auto-resolve-{prefix}")
}
