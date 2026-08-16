use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_agent::{
    ActionKind, AgentDecision, AgentGoal, AgentLoop, AgentOptions, AgentStatus, CapabilityPolicy,
    LlmError, LlmIdentity, LlmProvider, LlmUsage, NavigationScope, StructuredLlmRequest,
    StructuredLlmResponse,
};
use a3s_test_core::{
    Action, DriverError, DriverSession, Evidence, ModifierKey, PageContextLocator, PageContextNode,
    PageContextNodeState, PageContextObservation, PageContextSnapshot, StepOutput, Surface,
    SurfaceObservation, Target, TestStep,
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use url::Url;

struct ScriptedProvider {
    requests: Mutex<Vec<StructuredLlmRequest>>,
    responses: Mutex<VecDeque<Result<StructuredLlmResponse, LlmError>>>,
}

impl ScriptedProvider {
    fn new(responses: impl IntoIterator<Item = StructuredLlmResponse>) -> Self {
        Self::from_results(responses.into_iter().map(Ok))
    }

    fn from_results(
        responses: impl IntoIterator<Item = Result<StructuredLlmResponse, LlmError>>,
    ) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses.into_iter().collect()),
        }
    }
}

#[async_trait]
impl LlmProvider for ScriptedProvider {
    fn identity(&self) -> LlmIdentity {
        LlmIdentity {
            provider: "scripted-test-provider".to_string(),
            model: "test-model".to_string(),
        }
    }

    async fn complete(
        &self,
        request: StructuredLlmRequest,
    ) -> Result<StructuredLlmResponse, LlmError> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(LlmError::new(
                    "test.provider.exhausted",
                    "scripted response queue is empty",
                    false,
                ))
            })
    }
}

struct FakeSession {
    observations: VecDeque<SurfaceObservation>,
    actions: Vec<Action>,
    validated_revisions: Vec<u64>,
}

impl FakeSession {
    fn new(observations: impl IntoIterator<Item = SurfaceObservation>) -> Self {
        Self {
            observations: observations.into_iter().collect(),
            actions: Vec::new(),
            validated_revisions: Vec::new(),
        }
    }
}

#[async_trait]
impl DriverSession for FakeSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        self.observations.pop_front().ok_or_else(|| {
            DriverError::new(
                "test.session.observation_exhausted",
                "scripted observation queue is empty",
            )
        })
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.actions.push(step.action.clone());
        Ok(StepOutput::new("action executed"))
    }

    async fn validate_page_context_revision(
        &mut self,
        expected_revision: u64,
    ) -> Result<(), DriverError> {
        self.validated_revisions.push(expected_revision);
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn executes_schema_constrained_llm_decisions_until_success() {
    let provider = Arc::new(ScriptedProvider::new([
        response(
            AgentDecision::Act {
                action: Action::Click {
                    target: Target::Ref {
                        value: "@e1".to_string(),
                    },
                },
            },
            usage(20, 5, 40),
        ),
        response(
            AgentDecision::Finish {
                summary: "The document was created".to_string(),
            },
            usage(25, 4, 45),
        ),
    ]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Click],
        NavigationScope::Denied,
    ));
    let agent = AgentLoop::new(provider.clone(), policy, options()).expect("valid agent");
    let mut session = FakeSession::new([
        SurfaceObservation::new("Create button is visible"),
        SurfaceObservation::new("Document is open"),
    ]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::Succeeded);
    assert_eq!(result.prompt_version, "a3s-test-agent/v2");
    assert_eq!(result.summary.as_deref(), Some("The document was created"));
    assert_eq!(result.turns.len(), 2);
    assert_eq!(result.usage, usage(45, 9, 85));
    assert_eq!(
        session.actions,
        vec![Action::Click {
            target: Target::Ref {
                value: "@e1".to_string()
            }
        }]
    );
    assert_eq!(result.turns[0].decision_digest.len(), 64);

    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].response_schema.is_object());
    let schema = requests[0].response_schema.to_string();
    assert!(schema.contains("\"act\""));
    assert!(schema.contains("\"finish\""));
    assert!(schema.contains("\"navigate\""));
    assert!(!schema.contains("verify_contract"));
    assert_eq!(requests[0].prompt_version, "a3s-test-agent/v2");
    assert!(requests[0].image_attachments.is_empty());
    assert!(requests[0].context.history.is_empty());
    assert_eq!(requests[1].context.history.len(), 1);
    assert_eq!(requests[1].context.remaining.turns, 3);
    assert_eq!(requests[1].context.remaining.tokens, 975);
    assert_eq!(requests[1].context.remaining.cost_microusd, 960);
}

#[tokio::test]
async fn denies_runner_owned_contract_actions_even_when_a_policy_lists_them() {
    let provider = Arc::new(ScriptedProvider::new([response(
        AgentDecision::Act {
            action: Action::VerifyContract {
                contract: "./contract.acl".to_string(),
                variant: "desktop".to_string(),
                state: "ready".to_string(),
            },
        },
        usage(5, 3, 8),
    )]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::VerifyContract],
        NavigationScope::Denied,
    ));
    let agent = AgentLoop::new(provider, policy, options()).expect("valid agent");
    let mut session = FakeSession::new([SurfaceObservation::new("Page is ready")]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::PolicyDenied);
    assert_eq!(
        result.error.as_ref().map(|error| error.code.as_str()),
        Some("test.agent.policy.runner_action_denied")
    );
    assert!(session.actions.is_empty());
}

#[tokio::test]
async fn forwards_grounding_images_as_explicit_multimodal_attachments() {
    let provider = Arc::new(ScriptedProvider::new([response(
        AgentDecision::Finish {
            summary: "The visual criterion is satisfied".to_string(),
        },
        usage(10, 2, 5),
    )]));
    let policy = Arc::new(CapabilityPolicy::new([], NavigationScope::Denied));
    let agent = AgentLoop::new(provider.clone(), policy, options()).expect("valid agent");
    let mut session =
        FakeSession::new([
            SurfaceObservation::new("GUI window").with_evidence(Evidence {
                name: "gui-frame".to_string(),
                path: "/artifacts/gui-frame.png".to_string(),
                media_type: "image/png".to_string(),
            }),
        ]);

    let result = agent
        .run(
            &goal(),
            Surface::Gui,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::Succeeded);
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests[0].image_attachments.len(), 1);
    assert_eq!(requests[0].image_attachments[0].media_type, "image/png");
    assert_eq!(
        requests[0].image_attachments[0].path,
        "/artifacts/gui-frame.png"
    );
}

#[tokio::test]
async fn rejects_a_decision_that_does_not_match_the_schema() {
    let provider = Arc::new(ScriptedProvider::new([StructuredLlmResponse {
        decision: serde_json::json!({
            "type": "act",
            "action": {"type": "invented_action"}
        }),
        usage: usage(2, 2, 1),
        request_id: Some("request-invalid".to_string()),
    }]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Click],
        NavigationScope::Denied,
    ));
    let agent = AgentLoop::new(provider, policy, options()).expect("valid agent");
    let mut session = FakeSession::new([SurfaceObservation::new("Page is ready")]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::Failed);
    assert_eq!(
        result.error.as_ref().map(|error| error.code.as_str()),
        Some("test.agent.decision_invalid")
    );
    assert!(session.actions.is_empty());
}

#[tokio::test]
async fn blocks_cross_origin_navigation_before_surface_execution() {
    let provider = Arc::new(ScriptedProvider::new([response(
        AgentDecision::Act {
            action: Action::Navigate {
                url: "https://untrusted.test/login".to_string(),
            },
        },
        usage(5, 3, 8),
    )]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Navigate],
        NavigationScope::Origins(vec![
            Url::parse("https://office.example.test").expect("origin")
        ]),
    ));
    let agent = AgentLoop::new(provider, policy, options()).expect("valid agent");
    let mut session = FakeSession::new([SurfaceObservation::new("Start page")]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::PolicyDenied);
    assert_eq!(
        result.error.as_ref().map(|error| error.code.as_str()),
        Some("test.agent.policy.navigation_origin_denied")
    );
    assert!(session.actions.is_empty());
}

#[tokio::test]
async fn blocks_cross_origin_tab_and_video_navigation_before_surface_execution() {
    let actions = [
        Action::Tab {
            operation: a3s_test_core::TabOperation::New {
                url: Some("https://untrusted.test/tab".to_string()),
                label: None,
            },
        },
        Action::Video {
            operation: a3s_test_core::VideoOperation::Start {
                path: "capture.webm".to_string(),
                url: Some("https://untrusted.test/video".to_string()),
            },
        },
    ];

    for action in actions {
        let provider = Arc::new(ScriptedProvider::new([response(
            AgentDecision::Act {
                action: action.clone(),
            },
            usage(5, 3, 8),
        )]));
        let policy = Arc::new(CapabilityPolicy::new(
            [ActionKind::Tab, ActionKind::Video],
            NavigationScope::Origins(vec![
                Url::parse("https://trusted.test").expect("trusted URL")
            ]),
        ));
        let agent = AgentLoop::new(provider, policy, options()).expect("valid agent");
        let mut session = FakeSession::new([SurfaceObservation::new("Page is ready")]);

        let result = agent
            .run(
                &goal(),
                Surface::Web,
                &mut session,
                CancellationToken::new(),
            )
            .await;

        assert_eq!(result.status, AgentStatus::PolicyDenied);
        assert_eq!(
            result.error.as_ref().map(|error| error.code.as_str()),
            Some("test.agent.policy.navigation_origin_denied")
        );
        assert!(session.actions.is_empty());
    }
}

#[tokio::test]
async fn resolves_page_context_refs_before_surface_execution() {
    let provider = Arc::new(ScriptedProvider::new([response(
        AgentDecision::Act {
            action: Action::Click {
                target: Target::Ref {
                    value: "@c1".to_string(),
                },
            },
        },
        usage(5, 3, 8),
    )]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Click],
        NavigationScope::Denied,
    ));
    let agent = AgentLoop::new(provider, policy, options()).expect("valid agent");
    let mut session = FakeSession::new([page_context_observation()]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::Failed);
    assert_eq!(session.validated_revisions, [7]);
    assert_eq!(
        session.actions,
        [Action::Click {
            target: Target::TestId {
                value: "submit".to_string()
            }
        }]
    );
}

#[tokio::test]
async fn refuses_to_execute_a_decision_that_exceeds_the_token_budget() {
    let provider = Arc::new(ScriptedProvider::new([response(
        AgentDecision::Act {
            action: Action::Click {
                target: Target::Css {
                    selector: "#save".to_string(),
                },
            },
        },
        usage(6, 5, 1),
    )]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Click],
        NavigationScope::Denied,
    ));
    let mut constrained = options();
    constrained.max_total_tokens = 10;
    let agent = AgentLoop::new(provider, policy, constrained).expect("valid agent");
    let mut session = FakeSession::new([SurfaceObservation::new("Save is visible")]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::BudgetExceeded);
    assert_eq!(
        result.error.as_ref().map(|error| error.code.as_str()),
        Some("test.agent.budget.tokens_exceeded")
    );
    assert!(session.actions.is_empty());
}

#[tokio::test]
async fn preserves_retryability_when_the_llm_provider_fails() {
    let provider = Arc::new(ScriptedProvider::from_results([Err(LlmError::new(
        "llm.rate_limited",
        "provider rate limit exceeded",
        true,
    ))]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Click],
        NavigationScope::Denied,
    ));
    let agent = AgentLoop::new(provider, policy, options()).expect("valid agent");
    let mut session = FakeSession::new([SurfaceObservation::new("Page is ready")]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::Failed);
    let error = result.error.expect("provider error");
    assert_eq!(error.code, "llm.rate_limited");
    assert!(error.retryable);
    assert!(session.actions.is_empty());
}

#[tokio::test]
async fn honours_cancellation_before_observing_or_calling_the_llm() {
    let provider = Arc::new(ScriptedProvider::new([]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Click],
        NavigationScope::Denied,
    ));
    let agent = AgentLoop::new(provider.clone(), policy, options()).expect("valid agent");
    let mut session = FakeSession::new([SurfaceObservation::new("Page is ready")]);
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let result = agent
        .run(&goal(), Surface::Web, &mut session, cancellation)
        .await;

    assert_eq!(result.status, AgentStatus::Cancelled);
    assert!(provider.requests.lock().unwrap().is_empty());
    assert!(session.actions.is_empty());
}

#[tokio::test]
async fn bounds_serialized_context_before_calling_the_llm() {
    let provider = Arc::new(ScriptedProvider::new([]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Click],
        NavigationScope::Denied,
    ));
    let mut constrained = options();
    constrained.max_context_bytes = 8;
    let agent = AgentLoop::new(provider.clone(), policy, constrained).expect("valid agent");
    let mut session = FakeSession::new([SurfaceObservation::new(
        "A deliberately non-trivial observation",
    )]);

    let result = agent
        .run(
            &goal(),
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::BudgetExceeded);
    assert_eq!(
        result.error.as_ref().map(|error| error.code.as_str()),
        Some("test.agent.budget.context_exceeded")
    );
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[test]
fn advanced_actions_have_distinct_policy_capabilities() {
    let target = Target::Css {
        selector: "#target".to_string(),
    };
    let actions = [
        (
            Action::Hover {
                target: target.clone(),
            },
            ActionKind::Hover,
        ),
        (
            Action::Focus {
                target: target.clone(),
            },
            ActionKind::Focus,
        ),
        (
            Action::DoubleClick {
                target: target.clone(),
            },
            ActionKind::DoubleClick,
        ),
        (
            Action::ContextClick {
                target: target.clone(),
            },
            ActionKind::ContextClick,
        ),
        (
            Action::Type {
                target: target.clone(),
                value: "text".to_string(),
            },
            ActionKind::Type,
        ),
        (
            Action::Check {
                target: target.clone(),
            },
            ActionKind::Check,
        ),
        (
            Action::Uncheck {
                target: target.clone(),
            },
            ActionKind::Uncheck,
        ),
        (
            Action::Select {
                target: target.clone(),
                values: vec!["value".to_string()],
            },
            ActionKind::Select,
        ),
        (
            Action::Drag {
                source: target.clone(),
                target: target.clone(),
            },
            ActionKind::Drag,
        ),
        (
            Action::Wheel {
                target: Some(target),
                delta_x: 0,
                delta_y: 120,
                modifiers: vec![ModifierKey::Control],
            },
            ActionKind::Wheel,
        ),
        (
            Action::Viewport {
                width: 1280,
                height: 720,
                scale: None,
            },
            ActionKind::Viewport,
        ),
    ];

    for (action, expected) in actions {
        assert_eq!(ActionKind::from(&action), expected);
    }
}

fn goal() -> AgentGoal {
    AgentGoal {
        instruction: "Create a blank document".to_string(),
        success_criteria: vec!["A document editor is visible".to_string()],
    }
}

fn options() -> AgentOptions {
    AgentOptions {
        max_turns: 4,
        max_total_tokens: 1_000,
        max_cost_microusd: 1_000,
        max_context_bytes: 64 * 1_024,
        timeout: Duration::from_secs(2),
        provenance_redactor: Default::default(),
    }
}

fn response(decision: AgentDecision, usage: LlmUsage) -> StructuredLlmResponse {
    StructuredLlmResponse {
        decision: serde_json::to_value(decision).expect("serializable decision"),
        usage,
        request_id: Some("scripted-request".to_string()),
    }
}

fn usage(input_tokens: u64, output_tokens: u64, cost_microusd: u64) -> LlmUsage {
    LlmUsage {
        input_tokens,
        output_tokens,
        cost_microusd,
    }
}

fn page_context_observation() -> SurfaceObservation {
    SurfaceObservation::new("Checkout")
        .with_data(serde_json::json!({ "url": "https://example.test/" }))
        .with_page_context(PageContextObservation::from_snapshot(PageContextSnapshot {
            protocol: Some("a3s.test.page-context/1".to_string()),
            sdk_version: Some("0.2.0".to_string()),
            revision: Some(7),
            page: None,
            components: Vec::new(),
            nodes: vec![PageContextNode {
                id: String::new(),
                r#ref: Some("@c1".to_string()),
                parent_id: None,
                component_id: None,
                tag: "button".to_string(),
                role: Some("button".to_string()),
                name: Some("Submit".to_string()),
                text: Some("Submit".to_string()),
                description: None,
                test_id: Some("submit".to_string()),
                geometry: None,
                state: PageContextNodeState {
                    visible: true,
                    disabled: Some(false),
                    checked: None,
                    selected: None,
                    expanded: None,
                    focused: None,
                    readonly: None,
                    required: None,
                    invalid: None,
                },
                locators: vec![PageContextLocator::TestId {
                    value: "submit".to_string(),
                }],
                classes: None,
                attributes: None,
                computed_styles: None,
            }],
            facts: Default::default(),
            ui: None,
            removed_node_ids: Vec::new(),
            truncated: false,
            next_cursor: None,
        }))
}
