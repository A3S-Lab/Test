use std::collections::BTreeMap;

use a3s_test_core::{DriverError, Target};
use serde_json::{json, Value};

use crate::api::{CuaElement, CuaFrame, CuaWindowState};
use crate::lifecycle::{ApplicationBinding, WindowBinding};

#[derive(Default)]
pub(crate) struct SemanticState {
    generation: u64,
    current: Option<SemanticSnapshot>,
    last_source_snapshot: Option<String>,
}

impl SemanticState {
    pub(crate) fn has_snapshot(&self) -> bool {
        self.current.is_some()
    }

    pub(crate) fn install(
        &mut self,
        state: CuaWindowState,
        visual_digest: Option<String>,
    ) -> Result<(), DriverError> {
        if self.last_source_snapshot.as_deref() == Some(state.snapshot_id.as_str()) {
            return Err(DriverError::new(
                "test.driver.gui.stale_observation",
                "CUA repeated a snapshot identifier instead of producing fresh state",
            ));
        }
        self.generation = self.generation.checked_add(1).ok_or_else(|| {
            DriverError::new(
                "test.driver.gui.snapshot_limit_reached",
                "GUI semantic snapshot generation overflowed",
            )
        })?;
        self.last_source_snapshot = Some(state.snapshot_id.clone());
        self.current = Some(SemanticSnapshot::from_cua(
            self.generation,
            state,
            visual_digest,
        )?);
        Ok(())
    }

    pub(crate) fn invalidate(&mut self) {
        self.current = None;
    }

    pub(crate) fn data(
        &self,
        application: &ApplicationBinding,
        window: &WindowBinding,
    ) -> Result<Value, DriverError> {
        self.current
            .as_ref()
            .map(|snapshot| snapshot.data(application, window))
            .ok_or_else(snapshot_required)
    }

    pub(crate) fn resolve(&self, target: &Target) -> Result<ElementAddress, DriverError> {
        let Some(snapshot) = &self.current else {
            if let Target::Ref { value } = target {
                if reference_generation(value).is_some() {
                    return Err(stale_reference());
                }
            }
            return Err(snapshot_required());
        };
        snapshot.resolve(target)
    }

    pub(crate) fn text_visible(&self, text: &str) -> Result<bool, DriverError> {
        if text.trim().is_empty() {
            return Err(DriverError::new(
                "test.driver.gui.assertion_invalid",
                "GUI text assertion must not be empty",
            ));
        }
        let snapshot = self.current.as_ref().ok_or_else(snapshot_required)?;
        Ok(snapshot.elements.iter().any(|element| {
            element
                .name
                .iter()
                .chain(element.value.iter())
                .any(|candidate| candidate.contains(text))
        }))
    }

    pub(crate) fn resolve_visual(
        &self,
        snapshot_reference: &str,
        x: u32,
        y: u32,
    ) -> Result<VisualAddress, DriverError> {
        let visual = self
            .current
            .as_ref()
            .and_then(|snapshot| snapshot.visual.as_ref())
            .ok_or_else(stale_image)?;
        if visual.reference != snapshot_reference {
            return Err(stale_image());
        }
        if x >= visual.width || y >= visual.height {
            return Err(DriverError::new(
                "test.driver.gui.visual_point_out_of_bounds",
                format!(
                    "visual point ({x}, {y}) is outside the {}x{} grounding image",
                    visual.width, visual.height
                ),
            ));
        }
        Ok(VisualAddress {
            reference: visual.reference.clone(),
            x,
            y,
            evidence_path: visual.evidence_path.clone(),
            digest: visual.digest.clone(),
        })
    }
}

struct SemanticSnapshot {
    generation: u64,
    degraded: bool,
    visual: Option<VisualSnapshot>,
    elements: Vec<SemanticElement>,
}

impl SemanticSnapshot {
    fn from_cua(
        generation: u64,
        state: CuaWindowState,
        visual_digest: Option<String>,
    ) -> Result<Self, DriverError> {
        let visual = match (
            state.screenshot_width,
            state.screenshot_height,
            state.screenshot_mime_type.as_deref(),
            state.screenshot_file_path.as_deref(),
        ) {
            (Some(width), Some(height), Some("image/png"), Some(path)) => {
                let digest = visual_digest.ok_or_else(|| {
                    DriverError::new(
                        "test.driver.gui.screenshot_invalid",
                        "visual snapshot is missing its verified SHA-256 digest",
                    )
                })?;
                Some(VisualSnapshot {
                    reference: format!("@v{generation}"),
                    width,
                    height,
                    evidence_path: path.to_string(),
                    digest,
                })
            }
            (None, None, None, None) if visual_digest.is_none() => None,
            _ => {
                return Err(DriverError::new(
                    "test.driver.gui.cua_output_invalid",
                    "CUA returned a partial or unsupported visual snapshot",
                ));
            }
        };
        let references = state
            .elements
            .iter()
            .enumerate()
            .map(|(position, element)| {
                (
                    element.element_index,
                    format!("@g{generation}.{}", position + 1),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let elements = state
            .elements
            .into_iter()
            .map(|element| SemanticElement::from_cua(element, &references))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            generation,
            degraded: state.degraded,
            visual,
            elements,
        })
    }

    fn data(&self, application: &ApplicationBinding, window: &WindowBinding) -> Value {
        let elements = self
            .elements
            .iter()
            .map(SemanticElement::data)
            .collect::<Vec<_>>();
        let mut data = json!({
            "schema_version": 1,
            "surface": "gui",
            "application": {
                "pid": application.pid,
                "name": application.name,
                "owned": application.owned,
            },
            "window": {
                "id": window.window_id,
                "title": window.title,
            },
            "snapshot": {
                "generation": self.generation,
                "degraded": self.degraded,
            },
            "elements": elements,
        });
        if let Some(visual) = &self.visual {
            data["visual"] = json!({
                "ref": visual.reference,
                "width": visual.width,
                "height": visual.height,
                "media_type": "image/png",
                "sha256": visual.digest,
            });
        }
        data
    }

    fn resolve(&self, target: &Target) -> Result<ElementAddress, DriverError> {
        if let Target::Ref { value } = target {
            if let Some(generation) = reference_generation(value) {
                if generation != self.generation {
                    return Err(stale_reference());
                }
            }
            return self
                .elements
                .iter()
                .find(|element| element.reference == *value)
                .map(SemanticElement::address)
                .ok_or_else(target_not_found);
        }

        validate_target_text(target)?;
        let matches = self
            .elements
            .iter()
            .filter(|element| element.matches(target))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(target_not_found()),
            [element] => Ok(element.address()),
            _ => Err(DriverError::new(
                "test.driver.gui.target_ambiguous",
                "multiple GUI elements matched the semantic target; use a current ref",
            )),
        }
    }
}

struct SemanticElement {
    reference: String,
    token: String,
    role: String,
    name: Option<String>,
    value: Option<String>,
    automation_id: Option<String>,
    frame: Option<CuaFrame>,
    parent_reference: Option<String>,
    depth: u64,
}

impl SemanticElement {
    fn from_cua(
        element: CuaElement,
        references: &BTreeMap<u64, String>,
    ) -> Result<Self, DriverError> {
        let reference = references
            .get(&element.element_index)
            .cloned()
            .ok_or_else(|| {
                DriverError::new(
                    "test.driver.gui.cua_output_invalid",
                    "CUA element index was absent from the snapshot reference map",
                )
            })?;
        Ok(Self {
            reference,
            token: element.element_token,
            role: element.role,
            name: element.label,
            value: element.value,
            automation_id: element.automation_id,
            frame: element.frame,
            parent_reference: element
                .parent_index
                .and_then(|parent| references.get(&parent).cloned()),
            depth: element.depth,
        })
    }

    fn data(&self) -> Value {
        let mut value = json!({
            "ref": self.reference,
            "role": self.role,
            "depth": self.depth,
        });
        if let Some(name) = &self.name {
            value["name"] = Value::String(name.clone());
        }
        if let Some(current) = &self.value {
            value["value"] = Value::String(current.clone());
        }
        if let Some(automation_id) = &self.automation_id {
            value["automation_id"] = Value::String(automation_id.clone());
        }
        if let Some(parent) = &self.parent_reference {
            value["parent_ref"] = Value::String(parent.clone());
        }
        if let Some(frame) = &self.frame {
            value["frame"] = json!({
                "x": frame.x,
                "y": frame.y,
                "width": frame.w,
                "height": frame.h,
            });
        }
        value
    }

    fn address(&self) -> ElementAddress {
        ElementAddress {
            reference: self.reference.clone(),
            token: self.token.clone(),
        }
    }

    fn matches(&self, target: &Target) -> bool {
        match target {
            Target::Role { role, name } => {
                self.role == *role && self.name.as_deref() == Some(name.as_str())
            }
            Target::Text { value, exact } => {
                self.name.iter().chain(self.value.iter()).any(|candidate| {
                    if *exact {
                        candidate == value
                    } else {
                        candidate.contains(value)
                    }
                })
            }
            Target::AutomationId { value } => self.automation_id.as_deref() == Some(value.as_str()),
            Target::Label { value } => self.name.as_deref() == Some(value.as_str()),
            Target::Ref { .. }
            | Target::Css { .. }
            | Target::VisualPoint { .. }
            | Target::TestId { .. }
            | Target::Placeholder { .. } => false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ElementAddress {
    pub reference: String,
    pub token: String,
}

struct VisualSnapshot {
    reference: String,
    width: u32,
    height: u32,
    evidence_path: String,
    digest: String,
}

#[derive(Clone, Debug)]
pub(crate) struct VisualAddress {
    pub reference: String,
    pub x: u32,
    pub y: u32,
    pub evidence_path: String,
    pub digest: String,
}

fn validate_target_text(target: &Target) -> Result<(), DriverError> {
    let values: &[&str] = match target {
        Target::Role { role, name } => &[role, name],
        Target::Text { value, .. } | Target::AutomationId { value } | Target::Label { value } => {
            &[value]
        }
        Target::Css { .. }
        | Target::VisualPoint { .. }
        | Target::TestId { .. }
        | Target::Placeholder { .. } => {
            return Err(DriverError::new(
                "test.driver.gui.target_unsupported",
                "GUI semantic actions support ref, role, text, label, and automation_id targets",
            ));
        }
        Target::Ref { .. } => return Ok(()),
    };
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(DriverError::new(
            "test.driver.gui.target_invalid",
            "GUI semantic target values must not be empty",
        ));
    }
    Ok(())
}

fn reference_generation(reference: &str) -> Option<u64> {
    let value = reference.strip_prefix("@g")?;
    let (generation, position) = value.split_once('.')?;
    if generation.is_empty()
        || position.is_empty()
        || !position.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    generation.parse().ok()
}

fn snapshot_required() -> DriverError {
    DriverError::new(
        "test.driver.gui.snapshot_required",
        "capture a fresh GUI observation before using an element reference",
    )
}

fn stale_reference() -> DriverError {
    DriverError::new(
        "test.driver.gui.stale_reference",
        "GUI element reference belongs to an expired semantic snapshot",
    )
}

fn stale_image() -> DriverError {
    DriverError::new(
        "test.driver.gui.stale_image",
        "visual point does not belong to the current grounding image",
    )
}

fn target_not_found() -> DriverError {
    DriverError::new(
        "test.driver.gui.target_not_found",
        "no GUI element matched the semantic target",
    )
}
