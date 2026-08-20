use std::collections::HashSet;
use std::path::PathBuf;

use a3s_test_core::RepairBatch;
use a3s_test_session::RepairRecord;
use anyhow::Result;
use serde::Serialize;

use crate::agent_session::{watch_session, RepairPickup, RepairWatchRequest, RepairWatchResult};

const PROTOCOL: &str = "a3s.test.local-repair-bridge/1";

pub(super) struct LocalRepairBridge {
    workspace: PathBuf,
    session: String,
    pickup: RepairPickup,
    emitted: HashSet<(String, u64)>,
}

impl LocalRepairBridge {
    pub(super) fn new(workspace: PathBuf, session: String) -> Self {
        Self {
            workspace,
            session,
            pickup: RepairPickup::QueuedFirst,
            emitted: HashSet::new(),
        }
    }

    pub(super) fn metadata(&self) -> RepairBridgeMetadata {
        RepairBridgeMetadata {
            protocol: PROTOCOL,
            state: "watching",
            event: "repair_batch",
        }
    }

    pub(super) async fn next(&mut self) -> Result<Option<RepairBridgeBatch>> {
        let result = watch_session(RepairWatchRequest::bridge(
            self.workspace.clone(),
            self.session.clone(),
            self.pickup,
        ))
        .await?;
        self.pickup = RepairPickup::PageSubmission;
        Ok(self.select_unemitted(result))
    }

    fn select_unemitted(&mut self, result: RepairWatchResult) -> Option<RepairBridgeBatch> {
        let repairs = result
            .repairs
            .into_iter()
            .filter(|repair| {
                self.emitted
                    .insert((repair.finding.id.clone(), repair.sequence))
            })
            .collect::<Vec<_>>();
        if repairs.is_empty() {
            return None;
        }
        let finding_ids = repairs
            .iter()
            .map(|repair| repair.finding.id.as_str())
            .collect::<HashSet<_>>();
        let batches = result
            .batches
            .into_iter()
            .filter(|batch| {
                batch
                    .finding_ids
                    .iter()
                    .any(|finding_id| finding_ids.contains(finding_id.as_str()))
            })
            .collect();
        Some(RepairBridgeBatch {
            protocol_revision: result.protocol_revision,
            session: result.session,
            repairs,
            batches,
            ledger_path: result.ledger_path,
        })
    }
}

#[derive(Serialize)]
pub(super) struct RepairBridgeMetadata {
    protocol: &'static str,
    state: &'static str,
    event: &'static str,
}

pub(super) struct RepairBridgeBatch {
    pub(super) protocol_revision: u32,
    pub(super) session: String,
    pub(super) repairs: Vec<RepairRecord>,
    pub(super) batches: Vec<RepairBatch>,
    pub(super) ledger_path: PathBuf,
}

#[derive(Serialize)]
pub(super) struct RepairBridgeEvent<'a> {
    protocol: &'static str,
    event: &'static str,
    project: &'a str,
    protocol_revision: u32,
    session: &'a str,
    repairs: &'a [RepairRecord],
    batches: &'a [RepairBatch],
    ledger_path: &'a PathBuf,
}

impl<'a> RepairBridgeEvent<'a> {
    pub(super) fn new(project: &'a str, batch: &'a RepairBridgeBatch) -> Self {
        Self {
            protocol: PROTOCOL,
            event: "repair_batch",
            project,
            protocol_revision: batch.protocol_revision,
            session: &batch.session,
            repairs: &batch.repairs,
            batches: &batch.batches,
            ledger_path: &batch.ledger_path,
        }
    }
}
