use std::time::Duration;

use a3s_test_core::{Action, StepOutput, Surface, SurfaceObservation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AgentError, ProvenanceRedactor};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentGoal {
    pub instruction: String,
    pub success_criteria: Vec<String>,
}

impl AgentGoal {
    pub fn validate(&self) -> Result<(), AgentError> {
        const MAX_INSTRUCTION_BYTES: usize = 64 * 1_024;
        const MAX_SUCCESS_CRITERIA: usize = 64;
        const MAX_CRITERION_BYTES: usize = 16 * 1_024;

        if self.instruction.trim().is_empty() || self.instruction.len() > MAX_INSTRUCTION_BYTES {
            return Err(AgentError::new(
                "test.agent.goal.instruction_required",
                format!("agent instruction must contain 1 to {MAX_INSTRUCTION_BYTES} bytes"),
            ));
        }
        if self.success_criteria.is_empty()
            || self.success_criteria.len() > MAX_SUCCESS_CRITERIA
            || self.success_criteria.iter().any(|criterion| {
                criterion.trim().is_empty() || criterion.len() > MAX_CRITERION_BYTES
            })
        {
            return Err(AgentError::new(
                "test.agent.goal.success_criteria_required",
                format!(
                    "agent goal requires 1 to {MAX_SUCCESS_CRITERIA} success criteria of 1 to {MAX_CRITERION_BYTES} bytes each"
                ),
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
    /// Sanitizes the serializable result without changing provider or driver inputs.
    pub provenance_redactor: ProvenanceRedactor,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            max_turns: 12,
            max_total_tokens: 64_000,
            max_cost_microusd: 1_000_000,
            max_context_bytes: 512 * 1_024,
            timeout: Duration::from_secs(120),
            provenance_redactor: ProvenanceRedactor::default(),
        }
    }
}

impl AgentOptions {
    pub fn validate(&self) -> Result<(), AgentError> {
        const MAX_TURNS: u32 = 256;
        const MAX_TOTAL_TOKENS: u64 = 100_000_000;
        const MAX_COST_MICROUSD: u64 = 1_000_000_000;
        const MAX_CONTEXT_BYTES: usize = 64 * 1_024 * 1_024;
        const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

        if !(1..=MAX_TURNS).contains(&self.max_turns) {
            return Err(AgentError::new(
                "test.agent.config.turns_invalid",
                format!("maximum turns must be between 1 and {MAX_TURNS}"),
            ));
        }
        if !(1..=MAX_TOTAL_TOKENS).contains(&self.max_total_tokens) {
            return Err(AgentError::new(
                "test.agent.config.tokens_invalid",
                format!("maximum total tokens must be between 1 and {MAX_TOTAL_TOKENS}"),
            ));
        }
        if self.max_cost_microusd > MAX_COST_MICROUSD {
            return Err(AgentError::new(
                "test.agent.config.cost_invalid",
                format!("maximum cost must not exceed {MAX_COST_MICROUSD} micro-USD"),
            ));
        }
        if !(1..=MAX_CONTEXT_BYTES).contains(&self.max_context_bytes) {
            return Err(AgentError::new(
                "test.agent.config.context_invalid",
                format!("maximum context bytes must be between 1 and {MAX_CONTEXT_BYTES}"),
            ));
        }
        if self.timeout.is_zero() || self.timeout > MAX_TIMEOUT {
            return Err(AgentError::new(
                "test.agent.config.timeout_invalid",
                "agent timeout must be between 1 millisecond and 24 hours",
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

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionHistory {
    pub turn: u32,
    pub action: Action,
    pub output: StepOutput,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemainingBudget {
    pub turns: u32,
    pub tokens: u64,
    pub cost_microusd: u64,
    pub time_ms: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
