use a3s_test_core::{Action, DriverError, StepOutput};
use anyhow::Result;

use super::store::{AgentSessionError, AgentSessionEvent, AgentSessionState, AgentSessionStore};

pub(super) async fn append_success_event(
    store: &AgentSessionStore,
    state: &mut AgentSessionState,
    kind: &str,
    observation_id: Option<u64>,
    action: Action,
    output: StepOutput,
) -> Result<()> {
    let event = AgentSessionEvent {
        sequence: state.next_sequence,
        timestamp_ms: super::unix_ms(),
        kind: kind.to_string(),
        observation_id,
        action: Some(action),
        output: Some(output),
        error: None,
    };
    store.append_event(&event).await?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.updated_at_ms = event.timestamp_ms;
    Ok(())
}

pub(super) async fn append_terminal_event(
    store: &AgentSessionStore,
    state: &mut AgentSessionState,
    kind: &str,
) -> Result<()> {
    let event = AgentSessionEvent {
        sequence: state.next_sequence,
        timestamp_ms: super::unix_ms(),
        kind: kind.to_string(),
        observation_id: None,
        action: None,
        output: None,
        error: None,
    };
    store.append_event(&event).await?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.updated_at_ms = event.timestamp_ms;
    Ok(())
}

pub(super) async fn record_failure(
    store: &AgentSessionStore,
    state: &mut AgentSessionState,
    kind: &str,
    action: Option<Action>,
    error: &DriverError,
) -> Result<()> {
    let event = AgentSessionEvent {
        sequence: state.next_sequence,
        timestamp_ms: super::unix_ms(),
        kind: kind.to_string(),
        observation_id: None,
        action,
        output: None,
        error: Some(AgentSessionError {
            code: error.code().to_string(),
            message: error.message().to_string(),
        }),
    };
    store.append_event(&event).await?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.updated_at_ms = event.timestamp_ms;
    store.save(state).await
}
