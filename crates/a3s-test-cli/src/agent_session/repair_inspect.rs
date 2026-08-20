use std::process::ExitCode;

use anyhow::Result;

use super::args::RepairInspectArgs;
use super::{canonical_workspace, emit, load_session_state, load_store};
use a3s_test_session::RepairLedger;

pub(super) async fn execute(args: RepairInspectArgs) -> Result<ExitCode> {
    let workspace = canonical_workspace().await?;
    let store = load_store(&workspace, &args.session)?;
    let state = load_session_state(&store, &workspace, &args.session).await?;
    let ledger = RepairLedger::load(store.root().join("repairs.jsonl")).await?;
    let record = ledger
        .inspect_loop(&state.session, &args.finding_id)
        .map_err(anyhow::Error::new)?;
    let human = format!(
        "Repair '{}' is {:?}; resume with {:?}",
        record.finding_id, record.status, record.resume.action
    );
    emit(args.json, record, human)?;
    Ok(ExitCode::SUCCESS)
}
