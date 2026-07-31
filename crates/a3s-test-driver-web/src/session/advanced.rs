use std::collections::BTreeSet;

use a3s_test_core::{DriverError, ModifierKey, StepOutput, Target};
use serde_json::{json, Value};

use super::AgentBrowserSession;
use crate::actions::{drag_args, modifier_name};
use crate::protocol::direct_selector;

impl AgentBrowserSession {
    pub(super) async fn context_click(&self, target: &Target) -> Result<StepOutput, DriverError> {
        let (x, y, box_data) = self.target_center(target).await?;
        let move_data = self
            .execute_command(vec![
                "mouse".into(),
                "move".into(),
                x.to_string().into(),
                y.to_string().into(),
            ])
            .await?;
        let event_data = self
            .execute_command(vec![
                "eval".into(),
                targeted_context_menu_script(x, y).into(),
            ])
            .await;
        let event_data = match event_data {
            Ok(data) => data,
            Err(error) => return Err(error),
        };

        Ok(StepOutput::new("target context-clicked").with_data(json!({
            "box": box_data,
            "move": move_data,
            "event": event_data,
        })))
    }

    pub(super) async fn drag(
        &self,
        source: &Target,
        target: &Target,
    ) -> Result<StepOutput, DriverError> {
        let source_selector = direct_selector(source)?;
        let target_selector = direct_selector(target)?;
        self.execute_command(vec!["scrollintoview".into(), source_selector.into()])
            .await?;
        self.execute_command(vec!["scrollintoview".into(), target_selector.into()])
            .await?;
        match self.execute_command(drag_args(source, target)?).await {
            Ok(data) => Ok(StepOutput::new("target dragged").with_data(data)),
            Err(error) => {
                let _ = self
                    .execute_command(vec!["mouse".into(), "up".into(), "left".into()])
                    .await;
                Err(error)
            }
        }
    }

    pub(super) async fn wheel(
        &self,
        target: Option<&Target>,
        delta_x: i32,
        delta_y: i32,
        modifiers: &[ModifierKey],
    ) -> Result<StepOutput, DriverError> {
        if delta_x == 0 && delta_y == 0 {
            return Err(DriverError::new(
                "test.driver.web.wheel_delta_required",
                "wheel requires a non-zero delta_x or delta_y",
            ));
        }
        let mut unique = BTreeSet::new();
        if modifiers.iter().any(|modifier| !unique.insert(*modifier)) {
            return Err(DriverError::new(
                "test.driver.web.modifier_duplicate",
                "wheel modifiers cannot contain duplicates",
            ));
        }

        let target_center = match target {
            Some(target) => Some(self.target_center(target).await?),
            None => None,
        };
        let mut pressed = Vec::new();
        for modifier in modifiers {
            let result = self
                .execute_command(vec!["keydown".into(), modifier_name(*modifier).into()])
                .await;
            if let Err(error) = result {
                self.release_modifiers(&pressed).await;
                return Err(error);
            }
            pressed.push(*modifier);
        }

        let action_result = match target_center {
            Some((x, y, _)) => {
                let script = targeted_wheel_script(x, y, delta_x, delta_y, modifiers);
                self.execute_command(vec!["eval".into(), script.into()])
                    .await
            }
            None => {
                self.execute_command(vec![
                    "mouse".into(),
                    "wheel".into(),
                    delta_y.to_string().into(),
                    delta_x.to_string().into(),
                ])
                .await
            }
        };
        let cleanup_error = self.release_modifiers(&pressed).await;
        match (action_result, cleanup_error) {
            (Ok(data), None) => Ok(StepOutput::new("mouse wheel dispatched").with_data(data)),
            (Ok(_), Some(error)) | (Err(error), _) => Err(error),
        }
    }

    async fn release_modifiers(&self, pressed: &[ModifierKey]) -> Option<DriverError> {
        let mut first_error = None;
        for modifier in pressed.iter().rev() {
            if let Err(error) = self
                .execute_command(vec!["keyup".into(), modifier_name(*modifier).into()])
                .await
            {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        first_error
    }

    async fn target_center(&self, target: &Target) -> Result<(i32, i32, Value), DriverError> {
        let selector = direct_selector(target)?;
        self.execute_command(vec!["scrollintoview".into(), selector.into()])
            .await?;
        let data = self
            .execute_command(vec!["get".into(), "box".into(), selector.into()])
            .await?;
        let x = box_number(&data, "x")?;
        let y = box_number(&data, "y")?;
        let width = box_number(&data, "width")?;
        let height = box_number(&data, "height")?;
        if width <= 0.0 || height <= 0.0 {
            return Err(DriverError::new(
                "test.driver.web.box_invalid",
                "target bounding box must have positive width and height",
            ));
        }
        let center_x = checked_coordinate(x + width / 2.0)?;
        let center_y = checked_coordinate(y + height / 2.0)?;
        Ok((center_x, center_y, data))
    }
}

fn box_number(value: &Value, name: &str) -> Result<f64, DriverError> {
    value
        .get(name)
        .and_then(Value::as_f64)
        .or_else(|| value.get("data")?.get(name)?.as_f64())
        .or_else(|| value.pointer(&format!("/data/value/{name}"))?.as_f64())
        .filter(|number| number.is_finite())
        .ok_or_else(|| {
            DriverError::new(
                "test.driver.web.box_invalid",
                format!("browser bounding box response is missing finite '{name}'"),
            )
        })
}

fn checked_coordinate(value: f64) -> Result<i32, DriverError> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(DriverError::new(
            "test.driver.web.box_invalid",
            "target bounding box center is outside the supported coordinate range",
        ));
    }
    Ok(value.round() as i32)
}

fn targeted_wheel_script(
    x: i32,
    y: i32,
    delta_x: i32,
    delta_y: i32,
    modifiers: &[ModifierKey],
) -> String {
    let alt = modifiers.contains(&ModifierKey::Alt);
    let control = modifiers.contains(&ModifierKey::Control);
    let meta = modifiers.contains(&ModifierKey::Meta);
    let shift = modifiers.contains(&ModifierKey::Shift);
    format!(
        "(() => {{ const target = document.elementFromPoint({x}, {y}); \
         if (!target) throw new Error('A3S Test wheel target is not visible'); \
         const accepted = target.dispatchEvent(new WheelEvent('wheel', {{ \
         bubbles: true, cancelable: true, composed: true, deltaX: {delta_x}, \
         deltaY: {delta_y}, altKey: {alt}, ctrlKey: {control}, metaKey: {meta}, \
         shiftKey: {shift}, view: window }})); \
         return {{ accepted, tagName: target.tagName }}; }})()"
    )
}

fn targeted_context_menu_script(x: i32, y: i32) -> String {
    format!(
        "(() => {{ const target = document.elementFromPoint({x}, {y}); \
         if (!target) throw new Error('A3S Test context-click target is not visible'); \
         const event = new MouseEvent('contextmenu', {{ bubbles: true, cancelable: true, \
         composed: true, clientX: {x}, clientY: {y}, button: 2, buttons: 2, view: window }}); \
         const accepted = target.dispatchEvent(event); \
         return {{ accepted, defaultPrevented: event.defaultPrevented, \
         tagName: target.tagName }}; }})()"
    )
}
