use std::process::ExitCode;

use anyhow::{Context, Result};
use serde_json::json;

use super::args::{ListArgs, SessionArgs};
use super::store::AgentSessionStore;
use super::{canonical_workspace, emit, load_session_state, load_store};

pub(super) async fn show(args: SessionArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let state = load_session_state(&store, &workspace, &args.session).await?;
    emit(
        args.json,
        serde_json::to_value(&state)?,
        format!("{}: {:?} — {}", state.session, state.status, state.goal),
    )?;
    Ok(ExitCode::SUCCESS)
}

pub(super) async fn list(args: ListArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let root = AgentSessionStore::sessions_root(&workspace);
    let mut sessions = Vec::new();
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            emit(args.json, json!([]), "No agent test sessions".to_string())?;
            return Ok(ExitCode::SUCCESS);
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to list {}", root.display()));
        }
    };
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let Some(session) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let store = AgentSessionStore::for_workspace(&workspace, &session);
        if let Ok(state) = load_session_state(&store, &workspace, &session).await {
            sessions.push(state);
        }
    }
    sessions.sort_by(|left, right| left.session.cmp(&right.session));
    let human = if sessions.is_empty() {
        "No agent test sessions".to_string()
    } else {
        sessions
            .iter()
            .map(|state| format!("{}: {:?}", state.session, state.status))
            .collect::<Vec<_>>()
            .join("\n")
    };
    emit(args.json, serde_json::to_value(&sessions)?, human)?;
    Ok(ExitCode::SUCCESS)
}
