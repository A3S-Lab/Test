use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{Display, Formatter};

use crate::model::{PageContextRect, PageContextViewport};

pub const UI_UNDERSTANDING_PROTOCOL: &str = "a3s.test.ui-understanding/1";
const MAX_UI_NODES: u64 = 1_000;
const MAX_UI_STATE_SAMPLES: u64 = 1_000;
const MAX_UI_STRING_BYTES: u64 = 16_384;
const MAX_UI_ENCODED_BYTES: u64 = 1_048_576;
const MAX_UI_DURATION_MS: u64 = 100;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiUnderstandingSnapshot {
    pub protocol: String,
    pub observation_id: String,
    pub page_revision: u64,
    pub viewport: PageContextViewport,
    pub scope: UiContextScope,
    pub budget: UiUnderstandingBudget,
    pub evidence: UiUnderstandingEvidence,
    pub style: UiStyleProfile,
    pub layout: UiLayoutGraph,
    pub components: Vec<UiComponentCluster>,
    pub state_diffs: Vec<UiStateDiff>,
    pub motion: UiMotionProfile,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiContextScope {
    Page,
    Node {
        #[serde(rename = "nodeId")]
        node_id: String,
    },
    Component {
        #[serde(rename = "componentId")]
        component_id: String,
    },
    Region {
        space: UiCoordinateSpace,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiCoordinateSpace {
    Viewport,
    Document,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiUnderstandingBudget {
    pub limits: UiUnderstandingBudgetLimits,
    pub used: UiUnderstandingBudgetUsed,
    pub truncated: bool,
    pub reasons: Vec<UiTruncationReason>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiUnderstandingBudgetLimits {
    pub nodes: u64,
    pub state_samples: u64,
    pub string_bytes: u64,
    pub encoded_bytes: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiUnderstandingBudgetUsed {
    pub nodes: u64,
    pub state_samples: u64,
    pub encoded_bytes: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTruncationReason {
    NodeLimit,
    StateSampleLimit,
    TimeLimit,
    EncodedSizeLimit,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiUnderstandingEvidence {
    pub source_kinds: Vec<UiEvidenceSourceKind>,
    pub sampled_node_ids: Vec<String>,
    pub total_candidate_nodes: u64,
    pub omitted_nodes: u64,
    pub inaccessible_style_sheets: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEvidenceSourceKind {
    ComputedStyle,
    DomStructure,
    LayoutGeometry,
    AccessibilityState,
    CssStylesheet,
    WebAnimations,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiStyleProfile {
    pub colors: Vec<UiObservedToken>,
    pub typography: Vec<UiTypographyToken>,
    pub spacing: Vec<UiObservedToken>,
    pub radii: Vec<UiObservedToken>,
    pub shadows: Vec<UiObservedToken>,
    pub z_indices: Vec<UiObservedToken>,
    pub custom_properties: Vec<UiCustomProperty>,
    pub responsive_conditions: Vec<UiResponsiveCondition>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiObservedToken {
    pub value: String,
    pub properties: Vec<String>,
    pub count: u64,
    pub node_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiTypographyToken {
    pub family: String,
    pub size: String,
    pub weight: String,
    pub line_height: String,
    pub letter_spacing: String,
    pub count: u64,
    pub node_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiCustomProperty {
    pub name: String,
    pub value: String,
    pub source: UiCustomPropertySource,
    pub confidence: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiCustomPropertySource {
    DocumentRoot,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiResponsiveCondition {
    pub condition: String,
    pub matches: bool,
    pub source: UiResponsiveConditionSource,
    pub confidence: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiResponsiveConditionSource {
    Stylesheet,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiLayoutGraph {
    pub nodes: Vec<UiLayoutNode>,
    pub edges: Vec<UiLayoutEdge>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiLayoutNode {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_id: Option<String>,
    pub display: String,
    pub position: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<PageContextRect>,
    pub overflow_x: String,
    pub overflow_y: String,
    pub order: String,
    pub stacking_context_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flex: Option<UiFlexLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<UiGridLayout>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiFlexLayout {
    pub direction: String,
    pub wrap: String,
    pub justify_content: String,
    pub align_items: String,
    pub align_content: String,
    pub gap: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiGridLayout {
    pub template_columns: String,
    pub template_rows: String,
    pub auto_flow: String,
    pub justify_items: String,
    pub align_items: String,
    pub gap: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiLayoutEdge {
    pub from_node_id: String,
    pub to_node_id: String,
    pub relation: UiLayoutEdgeRelation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLayoutEdgeRelation {
    Contains,
    ScrollContainer,
    OffsetParent,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiComponentCluster {
    pub id: String,
    pub fingerprint: String,
    pub signature: String,
    pub representative_node_id: String,
    pub member_node_ids: Vec<String>,
    pub member_count: u64,
    pub confidence: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiStateDiff {
    pub node_id: String,
    #[serde(rename = "from")]
    pub from_state: UiBaselineState,
    #[serde(rename = "to")]
    pub to_state: UiObservedInteractionState,
    pub style_changes: Vec<UiStyleChange>,
    pub accessibility_changes: Vec<UiAccessibilityStateChange>,
    pub confidence: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiBaselineState {
    Default,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiObservedInteractionState {
    Hover,
    Focus,
    FocusVisible,
    Checked,
    Expanded,
    Selected,
    Disabled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiStyleChange {
    pub property: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiAccessibilityStateChange {
    pub state: String,
    pub before: Option<bool>,
    pub after: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiMotionProfile {
    pub prefers_reduced_motion: bool,
    pub transitions: Vec<UiTransitionProfile>,
    pub animations: Vec<UiAnimationProfile>,
    pub keyframe_names: Vec<String>,
    pub sticky_node_ids: Vec<String>,
    pub scroll_container_node_ids: Vec<String>,
    pub canvas_node_ids: Vec<String>,
    pub media_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiTransitionProfile {
    pub node_id: String,
    pub properties: Vec<String>,
    pub durations: Vec<String>,
    pub delays: Vec<String>,
    pub timing_functions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiAnimationProfile {
    pub node_id: String,
    pub names: Vec<String>,
    pub durations: Vec<String>,
    pub delays: Vec<String>,
    pub iteration_counts: Vec<String>,
    pub play_states: Vec<String>,
    pub sources: Vec<UiAnimationSource>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAnimationSource {
    Css,
    WebAnimations,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiUnderstandingValidationError {
    message: &'static str,
}

impl UiUnderstandingValidationError {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for UiUnderstandingValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for UiUnderstandingValidationError {}

impl UiUnderstandingSnapshot {
    pub fn validate(
        &self,
        page_revision: Option<u64>,
        page_viewport: Option<&PageContextViewport>,
    ) -> Result<(), UiUnderstandingValidationError> {
        if self.protocol != UI_UNDERSTANDING_PROTOCOL {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding protocol is unsupported",
            ));
        }
        if page_revision != Some(self.page_revision) {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding is not bound to the page revision",
            ));
        }
        if page_viewport != Some(&self.viewport) || !valid_viewport(&self.viewport) {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding is not bound to the page viewport",
            ));
        }
        if !valid_observation_id(&self.observation_id, self.page_revision) {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding observation id is invalid",
            ));
        }
        validate_scope(&self.scope)?;
        self.validate_budget()?;
        self.validate_collection_bounds()?;
        self.validate_evidence()?;
        self.validate_values()?;

        let encoded = serde_json::to_vec(self).map_err(|_| {
            UiUnderstandingValidationError::new("UI understanding cannot be encoded")
        })?;
        if encoded.len() > self.budget.limits.encoded_bytes as usize
            || self.budget.used.encoded_bytes > self.budget.limits.encoded_bytes
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding exceeds its encoded-size budget",
            ));
        }
        let value = serde_json::to_value(self).map_err(|_| {
            UiUnderstandingValidationError::new("UI understanding cannot be inspected")
        })?;
        if !bounded_json_strings(&value, self.budget.limits.string_bytes as usize, 0) {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding exceeds its string or depth budget",
            ));
        }
        Ok(())
    }

    fn validate_budget(&self) -> Result<(), UiUnderstandingValidationError> {
        let limits = &self.budget.limits;
        if !(1..=MAX_UI_NODES).contains(&limits.nodes)
            || !(1..=MAX_UI_STATE_SAMPLES).contains(&limits.state_samples)
            || !(32..=MAX_UI_STRING_BYTES).contains(&limits.string_bytes)
            || !(8_192..=MAX_UI_ENCODED_BYTES).contains(&limits.encoded_bytes)
            || !(1..=MAX_UI_DURATION_MS).contains(&limits.duration_ms)
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding declares invalid limits",
            ));
        }
        let used = &self.budget.used;
        if used.nodes > limits.nodes
            || used.state_samples > limits.state_samples
            || used.encoded_bytes > limits.encoded_bytes
            || (used.duration_ms > limits.duration_ms
                && !self.budget.reasons.contains(&UiTruncationReason::TimeLimit))
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding exceeds its declared limits",
            ));
        }
        if self.budget.reasons.len() > 4
            || has_duplicates(self.budget.reasons.iter().map(|reason| *reason as u8))
            || self.budget.truncated != !self.budget.reasons.is_empty()
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding truncation metadata is inconsistent",
            ));
        }
        Ok(())
    }

    fn validate_collection_bounds(&self) -> Result<(), UiUnderstandingValidationError> {
        if self.layout.nodes.len() > self.budget.limits.nodes as usize
            || self.layout.edges.len() > self.budget.limits.nodes as usize * 3
            || self.components.len() > 64
            || self.state_diffs.len() > self.budget.limits.state_samples as usize
            || self.style.colors.len() > 64
            || self.style.typography.len() > 32
            || self.style.spacing.len() > 64
            || self.style.radii.len() > 64
            || self.style.shadows.len() > 64
            || self.style.z_indices.len() > 64
            || self.style.custom_properties.len() > 64
            || self.style.responsive_conditions.len() > 64
            || self.motion.transitions.len() > 64
            || self.motion.animations.len() > 64
            || self.motion.keyframe_names.len() > 64
            || self.motion.sticky_node_ids.len() > 64
            || self.motion.scroll_container_node_ids.len() > 64
            || self.motion.canvas_node_ids.len() > 64
            || self.motion.media_node_ids.len() > 64
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding collection bounds are invalid",
            ));
        }
        if self.components.iter().any(|cluster| {
            cluster.member_count < 2
                || cluster.member_node_ids.len() > 32
                || cluster.member_count < cluster.member_node_ids.len() as u64
                || !lower_hex_16(&cluster.fingerprint)
                || cluster.id != format!("cluster-{}", cluster.fingerprint)
                || cluster.confidence != 1.0
        }) || self
            .state_diffs
            .iter()
            .any(|diff| diff.style_changes.len() > 24 || diff.confidence != 1.0)
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding component or state evidence is invalid",
            ));
        }
        Ok(())
    }

    fn validate_evidence(&self) -> Result<(), UiUnderstandingValidationError> {
        if self.evidence.source_kinds.len() > 6
            || self.evidence.sampled_node_ids.len() > 64
            || self.evidence.total_candidate_nodes < self.budget.used.nodes
            || self.evidence.omitted_nodes
                != self
                    .evidence
                    .total_candidate_nodes
                    .saturating_sub(self.budget.used.nodes)
            || self.evidence.sampled_node_ids.len() > self.budget.used.nodes as usize
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding evidence summary is inconsistent",
            ));
        }
        Ok(())
    }

    fn validate_values(&self) -> Result<(), UiUnderstandingValidationError> {
        let observed_tokens = self
            .style
            .colors
            .iter()
            .chain(self.style.spacing.iter())
            .chain(self.style.radii.iter())
            .chain(self.style.shadows.iter())
            .chain(self.style.z_indices.iter());
        if observed_tokens
            .into_iter()
            .any(|token| token.count == 0 || token.node_ids.len() > 8 || token.confidence != 1.0)
            || self.style.typography.iter().any(|token| {
                token.count == 0 || token.node_ids.len() > 8 || token.confidence != 1.0
            })
            || self
                .style
                .custom_properties
                .iter()
                .any(|token| token.confidence != 1.0)
            || self
                .style
                .responsive_conditions
                .iter()
                .any(|condition| condition.confidence != 1.0)
            || self.layout.nodes.iter().any(|node| {
                node.rect.as_ref().is_some_and(|rect| {
                    !rect.x.is_finite()
                        || !rect.y.is_finite()
                        || !rect.width.is_finite()
                        || !rect.height.is_finite()
                        || rect.width < 0.0
                        || rect.height < 0.0
                })
            })
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding contains invalid observed values",
            ));
        }
        Ok(())
    }
}

fn validate_scope(scope: &UiContextScope) -> Result<(), UiUnderstandingValidationError> {
    if let UiContextScope::Region {
        x,
        y,
        width,
        height,
        ..
    } = scope
    {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || *width < 0.0
            || *height < 0.0
        {
            return Err(UiUnderstandingValidationError::new(
                "UI understanding scope is invalid",
            ));
        }
    }
    Ok(())
}

fn valid_viewport(viewport: &PageContextViewport) -> bool {
    viewport.width.is_finite()
        && viewport.width > 0.0
        && viewport.height.is_finite()
        && viewport.height > 0.0
        && viewport.dpr.is_finite()
        && viewport.dpr > 0.0
        && viewport.visual.as_ref().is_none_or(|visual| {
            visual.x.is_finite()
                && visual.y.is_finite()
                && visual.width.is_finite()
                && visual.width >= 0.0
                && visual.height.is_finite()
                && visual.height >= 0.0
                && visual.scale.is_finite()
                && visual.scale > 0.0
        })
}

fn valid_observation_id(value: &str, revision: u64) -> bool {
    value
        .strip_prefix(&format!("ui-{revision}-"))
        .is_some_and(lower_hex_16)
}

fn lower_hex_16(value: &str) -> bool {
    value.len() == 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bounded_json_strings(value: &Value, max_string_bytes: usize, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    match value {
        Value::String(value) => value.len() <= max_string_bytes,
        Value::Array(values) => values
            .iter()
            .all(|value| bounded_json_strings(value, max_string_bytes, depth + 1)),
        Value::Object(values) => values.iter().all(|(key, value)| {
            key.len() <= 256 && bounded_json_strings(value, max_string_bytes, depth + 1)
        }),
        _ => true,
    }
}

fn has_duplicates(values: impl Iterator<Item = u8>) -> bool {
    let mut seen = [false; 256];
    for value in values {
        if seen[value as usize] {
            return true;
        }
        seen[value as usize] = true;
    }
    false
}
