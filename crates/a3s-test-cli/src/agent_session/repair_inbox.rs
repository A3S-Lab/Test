use std::path::Path;
use std::process::ExitCode;

use a3s_test_session::{
    RepairInbox, RepairLedger, MAX_REPAIR_INBOX_SESSIONS, MAX_REPAIR_LEDGER_BYTES,
};
use anyhow::{Context, Result};

use super::args::RepairInboxArgs;
use super::store::{is_link_like, AgentSessionStore};
use super::{
    canonical_workspace, emit, load_session_state, load_store, unix_ms, validate_session_id,
};

const MAX_WORKSPACE_INBOX_LEDGER_BYTES: u64 = 64 * 1_024 * 1_024;

pub(super) async fn execute(args: RepairInboxArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let ledgers = if let Some(session) = args.session.as_deref() {
        vec![load_session_ledger(&workspace, session, None).await?]
    } else {
        load_workspace_ledgers(&workspace).await?
    };
    let references = ledgers
        .iter()
        .map(|(session, ledger)| (session.as_str(), ledger));
    let inbox = RepairInbox::derive(
        args.session.as_deref(),
        references,
        unix_ms(),
        args.include_terminal,
        args.limit,
    )
    .map_err(anyhow::Error::new)?;
    let human = if inbox.items.is_empty() {
        "No matching repair loops".to_string()
    } else {
        inbox
            .items
            .iter()
            .map(|item| {
                format!(
                    "{}/{}: {:?} -> {:?}",
                    item.session, item.finding_id, item.status, item.next.action
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    emit(args.json, inbox, human)?;
    Ok(ExitCode::SUCCESS)
}

async fn load_workspace_ledgers(workspace: &Path) -> Result<Vec<(String, RepairLedger)>> {
    let root = AgentSessionStore::sessions_root(workspace);
    validate_sessions_root(workspace, &root).await?;
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to list {}", root.display()));
        }
    };
    let mut sessions = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = tokio::fs::symlink_metadata(&path)
            .await
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if is_link_like(&metadata) {
            anyhow::bail!(
                "agent session discovery refuses linked entry {}",
                path.display()
            );
        }
        if !metadata.file_type().is_dir() {
            continue;
        }
        let session = entry
            .file_name()
            .to_str()
            .map(str::to_string)
            .with_context(|| {
                format!(
                    "agent session name is not valid UTF-8: {}",
                    entry.path().display()
                )
            })?;
        validate_session_id(&session)?;
        sessions.push(session);
        if sessions.len() > MAX_REPAIR_INBOX_SESSIONS {
            anyhow::bail!(
                "repair inbox refuses more than {MAX_REPAIR_INBOX_SESSIONS} session directories"
            );
        }
    }
    sessions.sort();

    let mut total_bytes = 0_u64;
    let mut ledgers = Vec::with_capacity(sessions.len());
    for session in sessions {
        let loaded = load_session_ledger(workspace, &session, Some(&mut total_bytes)).await?;
        ledgers.push(loaded);
    }
    Ok(ledgers)
}

async fn load_session_ledger(
    workspace: &Path,
    session: &str,
    total_bytes: Option<&mut u64>,
) -> Result<(String, RepairLedger)> {
    let store = load_store(workspace, session)?;
    let state = load_session_state(&store, workspace, session).await?;
    let path = store.root().join("repairs.jsonl");
    let length = regular_ledger_length(&path).await?;
    if length > MAX_REPAIR_LEDGER_BYTES {
        anyhow::bail!(
            "repair ledger exceeds the {} byte limit: {}",
            MAX_REPAIR_LEDGER_BYTES,
            path.display()
        );
    }
    if let Some(total) = total_bytes {
        *total = total
            .checked_add(length)
            .context("workspace repair ledger byte count overflowed")?;
        if *total > MAX_WORKSPACE_INBOX_LEDGER_BYTES {
            anyhow::bail!(
                "workspace repair inbox exceeds the {MAX_WORKSPACE_INBOX_LEDGER_BYTES} byte scan limit; use --session to inspect one ledger"
            );
        }
    }
    let ledger = RepairLedger::load(path).await.map_err(anyhow::Error::new)?;
    Ok((state.session, ledger))
}

async fn regular_ledger_length(path: &Path) -> Result<u64> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_file() && !is_link_like(&metadata) => {
            Ok(metadata.len())
        }
        Ok(_) => anyhow::bail!("repair ledger must be a regular file: {}", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

async fn validate_sessions_root(workspace: &Path, sessions_root: &Path) -> Result<()> {
    let state_root = workspace.join(".a3s-test");
    for path in [state_root.as_path(), sessions_root] {
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) if metadata.file_type().is_dir() && !is_link_like(&metadata) => {}
            Ok(_) => anyhow::bail!(
                "agent session discovery root must be a regular directory: {}",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
            }
        }
    }
    let canonical = tokio::fs::canonicalize(sessions_root)
        .await
        .with_context(|| format!("failed to resolve {}", sessions_root.display()))?;
    if !canonical.starts_with(workspace) {
        anyhow::bail!(
            "agent session discovery root escapes the workspace: {}",
            sessions_root.display()
        );
    }
    Ok(())
}
