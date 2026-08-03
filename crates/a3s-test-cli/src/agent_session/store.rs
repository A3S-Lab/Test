use std::path::{Path, PathBuf};

use a3s_test_core::{Action, StepOutput, Surface};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

pub(crate) const SESSION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentSessionStatus {
    Active,
    Passed,
    Failed,
    Aborted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredBrowserDriver {
    A3s,
    Standalone,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct StoredBrowserConfig {
    pub(crate) driver: StoredBrowserDriver,
    pub(crate) executable: PathBuf,
    pub(crate) headed: bool,
    pub(crate) command_timeout_ms: u64,
    pub(crate) idle_timeout_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct AgentSessionState {
    pub(crate) schema_version: u32,
    pub(crate) session: String,
    pub(crate) workspace: PathBuf,
    pub(crate) surface: Surface,
    pub(crate) status: AgentSessionStatus,
    pub(crate) goal: String,
    pub(crate) success_criteria: Vec<String>,
    pub(crate) allowed_origins: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) browser_allowed_domains: Option<Vec<String>>,
    pub(crate) browser: StoredBrowserConfig,
    pub(crate) namespace: String,
    pub(crate) driver_session: String,
    pub(crate) runtime_dir: PathBuf,
    pub(crate) artifacts_dir: PathBuf,
    pub(crate) active_video_path: Option<String>,
    pub(crate) next_sequence: u64,
    pub(crate) next_observation_id: u64,
    pub(crate) latest_observation: Option<u64>,
    pub(crate) started_at_ms: u64,
    pub(crate) updated_at_ms: u64,
    pub(crate) summary: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct AgentSessionError {
    pub(crate) code: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct AgentSessionEvent {
    pub(crate) sequence: u64,
    pub(crate) timestamp_ms: u64,
    pub(crate) kind: String,
    pub(crate) observation_id: Option<u64>,
    pub(crate) action: Option<Action>,
    pub(crate) output: Option<StepOutput>,
    pub(crate) error: Option<AgentSessionError>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct AgentSessionReport {
    pub(crate) schema_version: u32,
    pub(crate) session: String,
    pub(crate) surface: Surface,
    pub(crate) status: AgentSessionStatus,
    pub(crate) goal: String,
    pub(crate) success_criteria: Vec<String>,
    pub(crate) allowed_origins: Vec<String>,
    #[serde(default)]
    pub(crate) browser_allowed_domains: Vec<String>,
    pub(crate) event_count: u64,
    pub(crate) artifacts_dir: PathBuf,
    pub(crate) events_path: PathBuf,
    pub(crate) started_at_ms: u64,
    pub(crate) finished_at_ms: u64,
    pub(crate) summary: String,
}

pub(crate) struct AgentSessionStore {
    root: PathBuf,
    state_path: PathBuf,
    events_path: PathBuf,
    report_path: PathBuf,
    artifacts_dir: PathBuf,
}

impl AgentSessionStore {
    pub(crate) fn for_workspace(workspace: &Path, session: &str) -> Self {
        let root = workspace
            .join(".a3s-test")
            .join("agent-sessions")
            .join(session);
        Self {
            state_path: root.join("session.json"),
            events_path: root.join("events.jsonl"),
            report_path: root.join("report.json"),
            artifacts_dir: root.join("artifacts"),
            root,
        }
    }

    pub(crate) fn sessions_root(workspace: &Path) -> PathBuf {
        workspace.join(".a3s-test").join("agent-sessions")
    }

    pub(crate) fn exists(&self) -> bool {
        self.state_path.is_file()
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn artifacts_dir(&self) -> &Path {
        &self.artifacts_dir
    }

    pub(crate) fn events_path(&self) -> &Path {
        &self.events_path
    }

    pub(crate) fn report_path(&self) -> &Path {
        &self.report_path
    }

    pub(crate) async fn create_directories(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.artifacts_dir)
            .await
            .with_context(|| {
                format!(
                    "failed to create agent session directory {}",
                    self.artifacts_dir.display()
                )
            })
    }

    pub(crate) async fn load(&self) -> Result<AgentSessionState> {
        let bytes = tokio::fs::read(&self.state_path).await.with_context(|| {
            format!("failed to read agent session {}", self.state_path.display())
        })?;
        let state: AgentSessionState = serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "invalid agent session metadata {}",
                self.state_path.display()
            )
        })?;
        if state.schema_version != SESSION_SCHEMA_VERSION {
            anyhow::bail!(
                "unsupported agent session schema {}; expected {}",
                state.schema_version,
                SESSION_SCHEMA_VERSION
            );
        }
        Ok(state)
    }

    pub(crate) async fn save(&self, state: &AgentSessionState) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(state)?;
        let temporary = self
            .root
            .join(format!(".session.json.{}.tmp", std::process::id()));
        tokio::fs::write(&temporary, bytes).await.with_context(|| {
            format!(
                "failed to write temporary session metadata {}",
                temporary.display()
            )
        })?;
        #[cfg(windows)]
        if self.state_path.exists() {
            tokio::fs::remove_file(&self.state_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to replace session metadata {}",
                        self.state_path.display()
                    )
                })?;
        }
        tokio::fs::rename(&temporary, &self.state_path)
            .await
            .with_context(|| {
                format!(
                    "failed to publish session metadata {}",
                    self.state_path.display()
                )
            })
    }

    pub(crate) async fn append_event(&self, event: &AgentSessionEvent) -> Result<()> {
        let mut encoded = serde_json::to_vec(event)?;
        encoded.push(b'\n');
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_path)
            .await
            .with_context(|| {
                format!(
                    "failed to open agent event log {}",
                    self.events_path.display()
                )
            })?;
        file.write_all(&encoded).await.with_context(|| {
            format!(
                "failed to append agent event log {}",
                self.events_path.display()
            )
        })
    }

    pub(crate) async fn write_report(&self, report: &AgentSessionReport) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(report)?;
        tokio::fs::write(&self.report_path, bytes)
            .await
            .with_context(|| {
                format!(
                    "failed to write agent session report {}",
                    self.report_path.display()
                )
            })
    }
}
