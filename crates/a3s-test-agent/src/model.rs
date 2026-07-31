use std::time::Duration;

use a3s_test_core::{Action, StepOutput, Surface, SurfaceObservation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AgentError;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentGoal {
    pub instruction: String,
    pub success_criteria: Vec<String>,
}

impl AgentGoal {
    pub(crate) fn validate(&self) -> Result<(), AgentError> {
        if self.instruction.trim().is_empty() {
            return Err(AgentError::new(
                "test.agent.goal.instruction_required",
                "agent instruction must not be empty",
            ));
        }
        if self.success_criteria.is_empty()
            || self
                .success_criteria
                .iter()
                .any(|criterion| criterion.trim().is_empty())
        {
            return Err(AgentError::new(
                "test.agent.goal.success_criteria_required",
                "at least one non-empty success criterion is required",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AgentOptions {
    pub max_turns: u32,
    pub max_total_tokens: u64,
    pub max_cost_microusd: u64,
    pub max_context_bytes: usize,
    pub timeout: Duration,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            max_turns: 12,
            max_total_tokens: 64_000,
            max_cost_microusd: 1_000_000,
            max_context_bytes: 512 * 1_024,
            timeout: Duration::from_secs(120),
        }
    }
}

impl AgentOptions {
    pub(crate) fn validate(&self) -> Result<(), AgentError> {
        if self.max_turns == 0 {
            return Err(AgentError::new(
                "test.agent.config.turns_invalid",
                "maximum turns must be greater than zero",
            ));
        }
        if self.max_total_tokens == 0 {
            return Err(AgentError::new(
                "test.agent.config.tokens_invalid",
                "maximum total tokens must be greater than zero",
            ));
        }
        if self.max_context_bytes == 0 {
            return Err(AgentError::new(
                "test.agent.config.context_invalid",
                "maximum context bytes must be greater than zero",
            ));
        }
        if self.timeout.is_zero() {
            return Err(AgentError::new(
                "test.agent.config.timeout_invalid",
                "agent timeout must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentDecision {
    Act { action: Action },
    Finish { summary: String },
    Fail { reason: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Succeeded,
    Failed,
    PolicyDenied,
    BudgetExceeded,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmIdentity {
    pub provider: String,
    pub model: String,
}

impl LlmIdentity {
    pub(crate) fn validate(&self) -> Result<(), AgentError> {
        if self.provider.trim().is_empty() || self.model.trim().is_empty() {
            return Err(AgentError::new(
                "test.agent.provider.identity_invalid",
                "LLM provider and model identities must not be empty",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microusd: u64,
}

impl LlmUsage {
    #[must_use]
    pub fn total_tokens(self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    #[must_use]
    pub(crate) fn saturating_add(self, other: Self) -> Self {
        Self {
            input_tokens: self.input_tokens.saturating_add(other.input_tokens),
            output_tokens: self.output_tokens.saturating_add(other.output_tokens),
            cost_microusd: self.cost_microusd.saturating_add(other.cost_microusd),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ActionHistory {
    pub turn: u32,
    pub action: Action,
    pub output: StepOutput,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RemainingBudget {
    pub turns: u32,
    pub tokens: u64,
    pub cost_microusd: u64,
    pub time_ms: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlannerContext {
    pub goal: AgentGoal,
    pub surface: Surface,
    pub turn: u32,
    pub observation: SurfaceObservation,
    pub history: Vec<ActionHistory>,
    pub remaining: RemainingBudget,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentTurn {
    pub turn: u32,
    pub observation: SurfaceObservation,
    pub decision: Option<AgentDecision>,
    pub decision_digest: String,
    pub request_id: Option<String>,
    pub usage: LlmUsage,
    pub llm_duration_ms: u64,
    pub output: Option<StepOutput>,
    pub error: Option<AgentError>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentRunResult {
    pub provider: LlmIdentity,
    pub prompt_version: String,
    pub status: AgentStatus,
    pub summary: Option<String>,
    pub usage: LlmUsage,
    pub turns: Vec<AgentTurn>,
    pub error: Option<AgentError>,
}
