use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_agent::{
    ActionKind, AgentDecision, AgentGoal, AgentLoop, AgentOptions, AgentStatus, CapabilityPolicy,
    LlmError, LlmIdentity, LlmProvider, LlmUsage, NavigationScope, ProvenanceRedactor,
    StructuredLlmRequest, StructuredLlmResponse, REDACTED_VALUE,
};
use a3s_test_core::{
    Action, DriverError, DriverSession, Evidence, PageContextComponent, PageContextLocator,
    PageContextNode, PageContextNodeState, PageContextObservation, PageContextSnapshot,
    PageContextSource, StepOutput, Surface, SurfaceObservation, Target, TestStep,
};
use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

const EXACT_SECRET: &str = "top-secret-value";
const INPUT_SECRET: &str = "unregistered-input-password";

struct RecordingProvider {
    requests: Mutex<Vec<StructuredLlmRequest>>,
    responses: Mutex<VecDeque<Result<StructuredLlmResponse, LlmError>>>,
}

impl RecordingProvider {
    fn new(responses: impl IntoIterator<Item = Result<StructuredLlmResponse, LlmError>>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses.into_iter().collect()),
        }
    }
}

#[async_trait]
impl LlmProvider for RecordingProvider {
    fn identity(&self) -> LlmIdentity {
        LlmIdentity {
            provider: format!("provider-{EXACT_SECRET}"),
            model: "model-safe".to_string(),
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
            .expect("scripted provider response")
    }
}

struct SecretSession {
    observations: VecDeque<SurfaceObservation>,
    actions: Vec<Action>,
}

#[async_trait]
impl DriverSession for SecretSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        self.observations.pop_front().ok_or_else(|| {
            DriverError::new(
                "test.session.observation_exhausted",
                "secret provenance observation queue is empty",
            )
        })
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.actions.push(step.action.clone());
        Ok(
            StepOutput::new(format!("executed with {EXACT_SECRET}")).with_data(json!({
                "Authorization": "Bearer implicit-output-secret",
                "visible": "safe output",
            })),
        )
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn redacts_the_complete_result_without_changing_execution_inputs() {
    let provider = Arc::new(RecordingProvider::new([
        Ok(response(
            AgentDecision::Act {
                action: Action::Fill {
                    target: Target::Label {
                        value: format!("Account {EXACT_SECRET}"),
                    },
                    value: INPUT_SECRET.to_string(),
                },
            },
            format!("request-{EXACT_SECRET}"),
        )),
        Ok(response(
            AgentDecision::Finish {
                summary: format!("finished with {EXACT_SECRET}"),
            },
            format!("finish-{EXACT_SECRET}"),
        )),
    ]));
    let policy = Arc::new(CapabilityPolicy::new(
        [ActionKind::Fill],
        NavigationScope::Denied,
    ));
    let options = AgentOptions {
        max_turns: 2,
        max_total_tokens: 1_000,
        max_cost_microusd: 1_000,
        max_context_bytes: 64 * 1_024,
        timeout: Duration::from_secs(2),
        provenance_redactor: ProvenanceRedactor::from_exact_secrets([EXACT_SECRET])
            .expect("valid redactor"),
    };
    let agent = AgentLoop::new(provider.clone(), policy, options).expect("valid agent");
    let mut session = SecretSession {
        observations: [observation("first"), observation("second")]
            .into_iter()
            .collect(),
        actions: Vec::new(),
    };

    let result = agent
        .run(
            &AgentGoal {
                instruction: "Fill the account field".to_string(),
                success_criteria: vec!["The form is accepted".to_string()],
            },
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::Succeeded);
    assert_eq!(result.turns[0].decision_digest.len(), 64);
    let encoded = serde_json::to_string(&result).expect("serializable result");
    assert!(!encoded.contains(EXACT_SECRET));
    assert!(!encoded.contains(INPUT_SECRET));
    assert!(!encoded.contains("implicit-observation-secret"));
    assert!(!encoded.contains("implicit-output-secret"));
    assert!(!encoded.contains("implicit-context-secret"));
    assert!(!encoded.contains("implicit-node-secret"));
    assert!(encoded.contains(REDACTED_VALUE));
    assert!(encoded.contains("safe observation"));
    assert!(encoded.contains("safe output"));

    assert_eq!(
        session.actions,
        [Action::Fill {
            target: Target::Label {
                value: format!("Account {EXACT_SECRET}"),
            },
            value: INPUT_SECRET.to_string(),
        }]
    );
    let requests = provider.requests.lock().unwrap();
    assert!(
        serde_json::to_string(&requests[0])
            .expect("serializable request")
            .contains(EXACT_SECRET),
        "the trusted provider request should retain the operational context"
    );
    assert!(
        serde_json::to_string(&requests[1])
            .expect("serializable history")
            .contains("implicit-output-secret"),
        "redaction must not corrupt history used by the planner"
    );
}

#[tokio::test]
async fn redacts_registered_values_from_provider_errors() {
    let provider = Arc::new(RecordingProvider::new([Err(LlmError::new(
        "llm.transport_failed",
        format!("upstream echoed {EXACT_SECRET}"),
        true,
    ))]));
    let policy = Arc::new(CapabilityPolicy::new([], NavigationScope::Denied));
    let options = AgentOptions {
        provenance_redactor: ProvenanceRedactor::from_exact_secrets([EXACT_SECRET])
            .expect("valid redactor"),
        ..AgentOptions::default()
    };
    let agent = AgentLoop::new(provider, policy, options).expect("valid agent");
    let mut session = SecretSession {
        observations: [observation("provider-error")].into_iter().collect(),
        actions: Vec::new(),
    };

    let result = agent
        .run(
            &AgentGoal {
                instruction: "Inspect the page".to_string(),
                success_criteria: vec!["The page is ready".to_string()],
            },
            Surface::Web,
            &mut session,
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, AgentStatus::Failed);
    assert!(result.error.as_ref().is_some_and(|error| error.retryable));
    let encoded = serde_json::to_string(&result).expect("serializable result");
    assert!(!encoded.contains(EXACT_SECRET));
    assert!(encoded.contains(REDACTED_VALUE));
}

fn observation(label: &str) -> SurfaceObservation {
    SurfaceObservation::new(format!("{label} observation contains {EXACT_SECRET}"))
        .with_data(json!({
            "password": "implicit-observation-secret",
            "visible": "safe observation",
            format!("key-{EXACT_SECRET}"): "safe dynamic key",
        }))
        .with_evidence(Evidence {
            name: format!("evidence-{EXACT_SECRET}"),
            path: format!("artifacts/{EXACT_SECRET}.png"),
            media_type: "image/png".to_string(),
        })
        .with_page_context(secret_page_context())
}

fn secret_page_context() -> PageContextObservation {
    PageContextObservation::from_snapshot(PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("0.2.0".to_string()),
        revision: Some(3),
        page: None,
        components: vec![PageContextComponent {
            id: "checkout".to_string(),
            name: format!("Checkout {EXACT_SECRET}"),
            parent_id: None,
            source: Some(PageContextSource {
                file: format!("src/{EXACT_SECRET}/Checkout.tsx"),
                line: Some(1),
                column: Some(1),
                end_line: None,
                end_column: None,
            }),
            ready: true,
            facts: serde_json::Map::from_iter([(
                "authorization".to_string(),
                json!("Bearer implicit-context-secret"),
            )]),
            boxes: Vec::new(),
        }],
        nodes: vec![PageContextNode {
            id: "submit".to_string(),
            r#ref: Some("@c1".to_string()),
            parent_id: None,
            component_id: Some("checkout".to_string()),
            tag: "button".to_string(),
            role: Some("button".to_string()),
            name: Some(format!("Submit {EXACT_SECRET}")),
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
            locators: vec![PageContextLocator::Label {
                value: format!("Submit {EXACT_SECRET}"),
            }],
            classes: None,
            attributes: Some(serde_json::Map::from_iter([(
                "password".to_string(),
                json!("implicit-node-secret"),
            )])),
            computed_styles: None,
            source_mapping: None,
        }],
        facts: serde_json::Map::new(),
        ui: None,
        removed_node_ids: Vec::new(),
        truncated: false,
        next_cursor: None,
    })
}

fn response(decision: AgentDecision, request_id: String) -> StructuredLlmResponse {
    StructuredLlmResponse {
        decision: serde_json::to_value(decision).expect("serializable decision"),
        usage: LlmUsage {
            input_tokens: 10,
            output_tokens: 2,
            cost_microusd: 3,
        },
        request_id: Some(request_id),
    }
}
