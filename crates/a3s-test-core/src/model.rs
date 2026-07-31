use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    Web,
    Gui,
    Tui,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TestSuite {
    pub name: String,
    pub version: u32,
    pub scenarios: Vec<TestScenario>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TestScenario {
    pub id: String,
    pub name: String,
    pub surface: Surface,
    pub timeout_ms: u64,
    pub steps: Vec<TestStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TestStep {
    pub id: String,
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Navigate { url: String },
    Snapshot { interactive: bool },
    Click { target: Target },
    Fill { target: Target, value: String },
    Press { key: String },
    Wait { condition: WaitCondition },
    Assert { expectation: Expectation },
    Screenshot { path: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Target {
    Ref { value: String },
    Css { selector: String },
    Role { role: String, name: String },
    Text { value: String, exact: bool },
    TestId(String),
    Label(String),
    Placeholder(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum WaitCondition {
    Load(LoadState),
    Text(String),
    Url(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadState {
    NetworkIdle,
    DomContentLoaded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Expectation {
    TextVisible(String),
    Url(String),
    Visible(Target),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Evidence {
    pub name: String,
    pub path: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StepOutput {
    pub summary: String,
    pub data: Value,
    pub evidence: Vec<Evidence>,
}

impl StepOutput {
    #[must_use]
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            data: Value::Null,
            evidence: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }

    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }
}
