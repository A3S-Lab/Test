use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ACTION_PROTOCOL_REVISION: u32 = 3;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Navigate {
        url: String,
    },
    Snapshot {
        interactive: bool,
    },
    Click {
        target: Target,
    },
    Hover {
        target: Target,
    },
    Focus {
        target: Target,
    },
    DoubleClick {
        target: Target,
    },
    ContextClick {
        target: Target,
    },
    Fill {
        target: Target,
        value: String,
    },
    Type {
        target: Target,
        value: String,
    },
    Check {
        target: Target,
    },
    Uncheck {
        target: Target,
    },
    Select {
        target: Target,
        values: Vec<String>,
    },
    Drag {
        source: Target,
        target: Target,
    },
    Press {
        key: String,
    },
    Wheel {
        target: Option<Target>,
        delta_x: i32,
        delta_y: i32,
        modifiers: Vec<ModifierKey>,
    },
    Viewport {
        width: u32,
        height: u32,
        scale: Option<u32>,
    },
    Wait {
        condition: WaitCondition,
    },
    Assert {
        expectation: Expectation,
    },
    Screenshot {
        path: String,
    },
    Tab {
        operation: TabOperation,
    },
    Frame {
        target: FrameTarget,
    },
    Dialog {
        operation: DialogOperation,
    },
    Upload {
        target: Target,
        paths: Vec<String>,
    },
    Download {
        target: Target,
        path: String,
    },
    NetworkRoute {
        pattern: String,
        route: NetworkRoute,
    },
    NetworkUnroute {
        pattern: Option<String>,
    },
    Har {
        operation: CaptureOperation,
    },
    Trace {
        operation: CaptureOperation,
    },
    Video {
        operation: VideoOperation,
    },
    Accessibility {
        path: String,
        interactive: bool,
    },
    Console {
        path: String,
        clear: bool,
    },
    PageErrors {
        path: String,
        clear: bool,
    },
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ModifierKey {
    Alt,
    Control,
    Meta,
    Shift,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum TabOperation {
    List,
    New {
        url: Option<String>,
        label: Option<String>,
    },
    Switch {
        tab: String,
    },
    Close {
        tab: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FrameTarget {
    Main,
    Selector(String),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DialogOperation {
    Status,
    Accept { text: Option<String> },
    Dismiss,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum NetworkRoute {
    Abort,
    Body(String),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaptureOperation {
    Start,
    Stop { path: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum VideoOperation {
    Start { path: String, url: Option<String> },
    Stop,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Ref { value: String },
    Css { selector: String },
    Role { role: String, name: String },
    Text { value: String, exact: bool },
    TestId { value: String },
    Label { value: String },
    Placeholder { value: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WaitCondition {
    Load(LoadState),
    Text(String),
    Url(String),
    Visible(Target),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadState {
    NetworkIdle,
    DomContentLoaded,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceObservation {
    pub summary: String,
    pub data: Value,
    pub evidence: Vec<Evidence>,
}

impl SurfaceObservation {
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
