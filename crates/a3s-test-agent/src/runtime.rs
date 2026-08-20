use std::future::Future;
use std::sync::Arc;

use a3s_test_core::{
    action_uses_page_context_ref, bind_page_context_refs, refresh_page_context_bindings,
    resolve_page_context_refs, DriverSession, PageContextBindings, Surface, TestStep,
};
use schemars::schema_for;
use sha2::{Digest, Sha256};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::{
    ActionHistory, ActionPolicy, AgentDecision, AgentError, AgentGoal, AgentOptions,
    AgentRunResult, AgentStatus, AgentTurn, LlmIdentity, LlmImageAttachment, LlmProvider, LlmUsage,
    PlannerContext, PolicyContext, RemainingBudget, StructuredLlmRequest,
};

const SYSTEM_INSTRUCTION: &str = "\
You are the planning component of an end-to-end test agent. Inspect the typed \
surface observation, attached grounding images, and prior action history, then return exactly one JSON \
object matching the supplied response schema. Propose only an action needed \
to reach the goal, finish only when the success criteria are visible in the \
observation, and fail when the goal cannot be completed safely. Do not emit \
commands, prose outside the JSON object, or keyword-routed shortcuts.";
pub const AGENT_PROMPT_VERSION: &str = "a3s-test-agent/v2";

pub struct AgentLoop {
    provider: Arc<dyn LlmProvider>,
    policy: Arc<dyn ActionPolicy>,
    options: AgentOptions,
    identity: LlmIdentity,
    response_schema: serde_json::Value,
}

impl AgentLoop {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        policy: Arc<dyn ActionPolicy>,
        options: AgentOptions,
    ) -> Result<Self, AgentError> {
        options.validate()?;
        let identity = provider.identity();
        identity.validate()?;
        let response_schema =
            serde_json::to_value(schema_for!(AgentDecision)).map_err(|error| {
                AgentError::new(
                    "test.agent.schema_generation_failed",
                    format!("failed to serialize agent decision schema: {error}"),
                )
            })?;
        Ok(Self {
            provider,
            policy,
            options,
            identity,
            response_schema,
        })
    }

    pub async fn run(
        &self,
        goal: &AgentGoal,
        surface: Surface,
        session: &mut dyn DriverSession,
        cancellation: CancellationToken,
    ) -> AgentRunResult {
        if let Err(error) = goal.validate() {
            return self.result(
                AgentStatus::Failed,
                None,
                LlmUsage::default(),
                Vec::new(),
                Some(error),
            );
        }

        let deadline = Instant::now() + self.options.timeout;
        let mut usage = LlmUsage::default();
        let mut history = Vec::new();
        let mut turns = Vec::with_capacity(self.options.max_turns as usize);

        for turn in 1..=self.options.max_turns {
            let mut observation =
                match await_stage(&cancellation, deadline, session.observe()).await {
                    Stage::Completed(Ok(observation)) => observation,
                    Stage::Completed(Err(error)) => {
                        return self.result(
                            AgentStatus::Failed,
                            None,
                            usage,
                            turns,
                            Some(AgentError::new(error.code(), error.message())),
                        );
                    }
                    Stage::Cancelled => {
                        return self.result(
                            AgentStatus::Cancelled,
                            None,
                            usage,
                            turns,
                            Some(AgentError::new(
                                "test.agent.cancelled",
                                "agent run was cancelled while observing the surface",
                            )),
                        );
                    }
                    Stage::TimedOut => {
                        return self.result(
                            AgentStatus::TimedOut,
                            None,
                            usage,
                            turns,
                            Some(AgentError::new(
                                "test.agent.timeout",
                                "agent deadline expired while observing the surface",
                            )),
                        );
                    }
                };
            let page_context_bindings = bind_page_context_refs(&mut observation);

            let context = PlannerContext {
                goal: goal.clone(),
                surface,
                turn,
                observation: observation.clone(),
                history: history.clone(),
                remaining: self.remaining(turn, usage, deadline),
            };
            let context_bytes = match serde_json::to_vec(&context) {
                Ok(encoded) => encoded.len(),
                Err(error) => {
                    return self.result(
                        AgentStatus::Failed,
                        None,
                        usage,
                        turns,
                        Some(AgentError::new(
                            "test.agent.context_serialization_failed",
                            format!("failed to serialize planner context: {error}"),
                        )),
                    );
                }
            };
            if context_bytes > self.options.max_context_bytes {
                return self.result(
                    AgentStatus::BudgetExceeded,
                    None,
                    usage,
                    turns,
                    Some(AgentError::new(
                        "test.agent.budget.context_exceeded",
                        format!(
                            "planner context is {context_bytes} bytes, exceeding the {} byte limit",
                            self.options.max_context_bytes
                        ),
                    )),
                );
            }
            let request = StructuredLlmRequest {
                prompt_version: AGENT_PROMPT_VERSION.to_string(),
                system_instruction: SYSTEM_INSTRUCTION.to_string(),
                image_attachments: observation
                    .evidence
                    .iter()
                    .filter(|evidence| evidence.media_type.starts_with("image/"))
                    .map(|evidence| LlmImageAttachment {
                        name: evidence.name.clone(),
                        path: evidence.path.clone(),
                        media_type: evidence.media_type.clone(),
                    })
                    .collect(),
                context,
                response_schema: self.response_schema.clone(),
            };
            let llm_started = Instant::now();
            let response =
                match await_stage(&cancellation, deadline, self.provider.complete(request)).await {
                    Stage::Completed(Ok(response)) => response,
                    Stage::Completed(Err(error)) => {
                        let retryable = error.retryable();
                        return self.result(
                            AgentStatus::Failed,
                            None,
                            usage,
                            turns,
                            Some(
                                AgentError::new(error.code(), error.message())
                                    .with_retryable(retryable),
                            ),
                        );
                    }
                    Stage::Cancelled => {
                        return self.result(
                            AgentStatus::Cancelled,
                            None,
                            usage,
                            turns,
                            Some(AgentError::new(
                                "test.agent.cancelled",
                                "agent run was cancelled while waiting for the LLM",
                            )),
                        );
                    }
                    Stage::TimedOut => {
                        return self.result(
                            AgentStatus::TimedOut,
                            None,
                            usage,
                            turns,
                            Some(AgentError::new(
                                "test.agent.timeout",
                                "agent deadline expired while waiting for the LLM",
                            )),
                        );
                    }
                };
            let llm_duration_ms = millis(llm_started.elapsed());

            usage = usage.saturating_add(response.usage);
            let digest = match decision_digest(&response.decision) {
                Ok(digest) => digest,
                Err(error) => {
                    return self.result(AgentStatus::Failed, None, usage, turns, Some(error));
                }
            };
            let decision = match serde_json::from_value::<AgentDecision>(response.decision.clone())
            {
                Ok(decision) => decision,
                Err(error) => {
                    let error = AgentError::new(
                        "test.agent.decision_invalid",
                        format!("LLM decision does not match the required schema: {error}"),
                    );
                    turns.push(AgentTurn {
                        turn,
                        observation,
                        decision: None,
                        decision_digest: digest,
                        request_id: response.request_id,
                        usage: response.usage,
                        llm_duration_ms,
                        output: None,
                        error: Some(error.clone()),
                    });
                    return self.result(AgentStatus::Failed, None, usage, turns, Some(error));
                }
            };

            let mut agent_turn = AgentTurn {
                turn,
                observation,
                decision: Some(decision.clone()),
                decision_digest: digest,
                request_id: response.request_id,
                usage: response.usage,
                llm_duration_ms,
                output: None,
                error: None,
            };
            if usage.total_tokens() > self.options.max_total_tokens {
                let error = AgentError::new(
                    "test.agent.budget.tokens_exceeded",
                    "LLM usage exceeded the configured total token budget",
                );
                agent_turn.error = Some(error.clone());
                turns.push(agent_turn);
                return self.result(AgentStatus::BudgetExceeded, None, usage, turns, Some(error));
            }
            if usage.cost_microusd > self.options.max_cost_microusd {
                let error = AgentError::new(
                    "test.agent.budget.cost_exceeded",
                    "LLM usage exceeded the configured cost budget",
                );
                agent_turn.error = Some(error.clone());
                turns.push(agent_turn);
                return self.result(AgentStatus::BudgetExceeded, None, usage, turns, Some(error));
            }

            match decision {
                AgentDecision::Finish { summary } => {
                    turns.push(agent_turn);
                    return self.result(AgentStatus::Succeeded, Some(summary), usage, turns, None);
                }
                AgentDecision::Fail { reason } => {
                    let error = AgentError::new("test.agent.model_failed", reason);
                    agent_turn.error = Some(error.clone());
                    turns.push(agent_turn);
                    return self.result(AgentStatus::Failed, None, usage, turns, Some(error));
                }
                AgentDecision::Act { action } => {
                    let policy_context = PolicyContext {
                        goal,
                        surface,
                        observation: &agent_turn.observation,
                        history: &history,
                    };
                    if let Err(error) = self.policy.validate(&policy_context, &action) {
                        agent_turn.error = Some(error.clone());
                        turns.push(agent_turn);
                        return self.result(
                            AgentStatus::PolicyDenied,
                            None,
                            usage,
                            turns,
                            Some(error),
                        );
                    }

                    let mut bindings = page_context_bindings;
                    let expected_revision = match observation_target_revision(&action, &bindings) {
                        Ok(revision) => revision,
                        Err(error) => {
                            agent_turn.error = Some(error.clone());
                            turns.push(agent_turn);
                            return self.result(
                                AgentStatus::PolicyDenied,
                                None,
                                usage,
                                turns,
                                Some(error),
                            );
                        }
                    };
                    if let Some(revision) = expected_revision {
                        match await_stage(
                            &cancellation,
                            deadline,
                            session.page_context_delta(revision),
                        )
                        .await
                        {
                            Stage::Completed(Ok(Some(context))) => {
                                if let Err(error) =
                                    refresh_page_context_bindings(&mut bindings, &context)
                                {
                                    let error = AgentError::new(
                                        "test.agent.policy.observation_ref_invalid",
                                        error.message(),
                                    );
                                    agent_turn.error = Some(error.clone());
                                    turns.push(agent_turn);
                                    return self.result(
                                        AgentStatus::PolicyDenied,
                                        None,
                                        usage,
                                        turns,
                                        Some(error),
                                    );
                                }
                            }
                            Stage::Completed(Ok(None)) => {}
                            Stage::Completed(Err(error)) => {
                                let error = AgentError::new(error.code(), error.message());
                                agent_turn.error = Some(error.clone());
                                turns.push(agent_turn);
                                return self.result(
                                    AgentStatus::PolicyDenied,
                                    None,
                                    usage,
                                    turns,
                                    Some(error),
                                );
                            }
                            Stage::Cancelled => {
                                let error = AgentError::new(
                                    "test.agent.cancelled",
                                    "agent run was cancelled while refreshing page context",
                                );
                                agent_turn.error = Some(error.clone());
                                turns.push(agent_turn);
                                return self.result(
                                    AgentStatus::Cancelled,
                                    None,
                                    usage,
                                    turns,
                                    Some(error),
                                );
                            }
                            Stage::TimedOut => {
                                let error = AgentError::new(
                                    "test.agent.timeout",
                                    "agent deadline expired while refreshing page context",
                                );
                                agent_turn.error = Some(error.clone());
                                turns.push(agent_turn);
                                return self.result(
                                    AgentStatus::TimedOut,
                                    None,
                                    usage,
                                    turns,
                                    Some(error),
                                );
                            }
                        }
                    }
                    let action = match resolve_observation_target(action, &bindings) {
                        Ok(action) => action,
                        Err(error) => {
                            agent_turn.error = Some(error.clone());
                            turns.push(agent_turn);
                            return self.result(
                                AgentStatus::PolicyDenied,
                                None,
                                usage,
                                turns,
                                Some(error),
                            );
                        }
                    };

                    let step = TestStep {
                        id: format!("agent-turn-{turn}"),
                        action: action.clone(),
                        stability: None,
                        assertion_mode: Default::default(),
                        wait_mode: Default::default(),
                    };
                    match await_stage(&cancellation, deadline, session.execute(&step)).await {
                        Stage::Completed(Ok(output)) => {
                            agent_turn.output = Some(output.clone());
                            history.push(ActionHistory {
                                turn,
                                action,
                                output,
                            });
                            turns.push(agent_turn);
                        }
                        Stage::Completed(Err(error)) => {
                            let error = AgentError::new(error.code(), error.message());
                            agent_turn.error = Some(error.clone());
                            turns.push(agent_turn);
                            return self.result(
                                AgentStatus::Failed,
                                None,
                                usage,
                                turns,
                                Some(error),
                            );
                        }
                        Stage::Cancelled => {
                            let error = AgentError::new(
                                "test.agent.cancelled",
                                "agent run was cancelled while executing a surface action",
                            );
                            agent_turn.error = Some(error.clone());
                            turns.push(agent_turn);
                            return self.result(
                                AgentStatus::Cancelled,
                                None,
                                usage,
                                turns,
                                Some(error),
                            );
                        }
                        Stage::TimedOut => {
                            let error = AgentError::new(
                                "test.agent.timeout",
                                "agent deadline expired while executing a surface action",
                            );
                            agent_turn.error = Some(error.clone());
                            turns.push(agent_turn);
                            return self.result(
                                AgentStatus::TimedOut,
                                None,
                                usage,
                                turns,
                                Some(error),
                            );
                        }
                    }
                }
            }
        }

        self.result(
            AgentStatus::BudgetExceeded,
            None,
            usage,
            turns,
            Some(AgentError::new(
                "test.agent.budget.turns_exceeded",
                "agent used every configured turn without finishing",
            )),
        )
    }

    fn remaining(&self, turn: u32, usage: LlmUsage, deadline: Instant) -> RemainingBudget {
        RemainingBudget {
            turns: self.options.max_turns.saturating_sub(turn - 1),
            tokens: self
                .options
                .max_total_tokens
                .saturating_sub(usage.total_tokens()),
            cost_microusd: self
                .options
                .max_cost_microusd
                .saturating_sub(usage.cost_microusd),
            time_ms: millis(deadline.saturating_duration_since(Instant::now())),
        }
    }

    fn result(
        &self,
        status: AgentStatus,
        summary: Option<String>,
        usage: LlmUsage,
        turns: Vec<AgentTurn>,
        error: Option<AgentError>,
    ) -> AgentRunResult {
        self.options
            .provenance_redactor
            .redact_result(AgentRunResult {
                provider: self.identity.clone(),
                prompt_version: AGENT_PROMPT_VERSION.to_string(),
                status,
                summary,
                usage,
                turns,
                error,
            })
    }
}

fn resolve_observation_target(
    action: a3s_test_core::Action,
    bindings: &PageContextBindings,
) -> Result<a3s_test_core::Action, AgentError> {
    resolve_page_context_refs(action, bindings).map_err(|error| {
        AgentError::new("test.agent.policy.observation_ref_invalid", error.message())
    })
}

fn observation_target_revision(
    action: &a3s_test_core::Action,
    bindings: &PageContextBindings,
) -> Result<Option<u64>, AgentError> {
    if !action_uses_page_context_ref(action) {
        return Ok(None);
    }
    bindings.revision.map(Some).ok_or_else(|| {
        AgentError::new(
            "test.agent.policy.observation_revision_missing",
            "page context ref is missing its observation revision",
        )
    })
}

enum Stage<T> {
    Completed(T),
    Cancelled,
    TimedOut,
}

async fn await_stage<T>(
    cancellation: &CancellationToken,
    deadline: Instant,
    future: impl Future<Output = T>,
) -> Stage<T> {
    tokio::select! {
        biased;
        () = cancellation.cancelled() => Stage::Cancelled,
        result = tokio::time::timeout_at(deadline, future) => match result {
            Ok(result) => Stage::Completed(result),
            Err(_) => Stage::TimedOut,
        },
    }
}

fn decision_digest(value: &serde_json::Value) -> Result<String, AgentError> {
    let encoded = serde_json::to_vec(value).map_err(|error| {
        AgentError::new(
            "test.agent.decision_digest_failed",
            format!("failed to encode LLM decision for provenance: {error}"),
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
