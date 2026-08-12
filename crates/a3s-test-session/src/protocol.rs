use a3s_test_core::{
    Action, DriverError, RepairCheckResult, StepOutput, Surface, SurfaceObservation,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StartSessionRequest {
    pub session: String,
    pub surface: Surface,
    pub goal: String,
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub auto_resolve_repairs: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionStarted {
    pub session: String,
    pub surface: Surface,
    pub goal: String,
    pub success_criteria: Vec<String>,
    pub auto_resolve_repairs: bool,
    pub started_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObservationResult {
    pub session: String,
    pub observation_id: u64,
    pub observation: SurfaceObservation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActSessionRequest {
    pub session: String,
    pub observation_id: Option<u64>,
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ActionResult {
    pub session: String,
    pub output: StepOutput,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairVerifyRequest {
    pub session: String,
    pub finding_id: String,
    pub request_id: String,
    pub success_criteria_passed: Option<bool>,
    pub changed_files: Vec<String>,
    pub checks: Vec<RepairCheckResult>,
    pub acl_candidate: Option<String>,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionFinishStatus {
    Passed,
    Failed,
    Aborted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FinishSessionRequest {
    pub session: String,
    pub status: SessionFinishStatus,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SessionFinished {
    pub session: String,
    pub surface: Surface,
    pub goal: String,
    pub success_criteria: Vec<String>,
    pub status: SessionFinishStatus,
    pub summary: String,
    pub turns: u64,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub cleanup_error: Option<SessionFailure>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl SessionFailure {
    pub(crate) fn from_driver(error: DriverError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.message().to_string(),
            retryable: error.retryable(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionError {
    code: String,
    message: String,
    retryable: bool,
}

impl SessionError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }

    pub(crate) fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub(crate) fn from_driver(error: DriverError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.message().to_string(),
            retryable: error.retryable(),
        }
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SessionError {}

pub(crate) fn validate_start(request: &StartSessionRequest) -> Result<(), SessionError> {
    validate_session_id(&request.session)?;
    if request.goal.trim().is_empty() {
        return Err(SessionError::new(
            "test.session.goal_invalid",
            "session goal must not be empty",
        ));
    }
    if request.success_criteria.is_empty()
        || request
            .success_criteria
            .iter()
            .any(|criterion| criterion.trim().is_empty())
    {
        return Err(SessionError::new(
            "test.session.criteria_invalid",
            "at least one non-empty success criterion is required",
        ));
    }
    Ok(())
}

pub(crate) fn validate_session_id(session: &str) -> Result<(), SessionError> {
    if session.is_empty()
        || session.len() > 48
        || !session
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(SessionError::new(
            "test.session.id_invalid",
            "session id must be 1-48 ASCII letters, digits, '-' or '_'",
        ));
    }
    Ok(())
}
