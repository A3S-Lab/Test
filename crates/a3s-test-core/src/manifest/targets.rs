use a3s_acl::{Block, Value};

use crate::{SpecError, Target};

use super::type_error;

pub(super) fn required_target(block: &Block, name: &str, path: &str) -> Result<Target, SpecError> {
    let value = block.attributes.get(name).ok_or_else(|| {
        SpecError::new(
            "test.spec.attribute_required",
            format!("{path}.{name}"),
            "required target is missing",
        )
    })?;
    parse_target(value, &format!("{path}.{name}"))
}

pub(super) fn optional_target(
    block: &Block,
    name: &str,
    path: &str,
) -> Result<Option<Target>, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| parse_target(value, &format!("{path}.{name}")))
        .transpose()
}

pub(super) fn parse_target(value: &Value, path: &str) -> Result<Target, SpecError> {
    let Value::Call(name, arguments) = value else {
        return Err(type_error(path, "target must use a typed locator function"));
    };
    match (name.as_str(), arguments.as_slice()) {
        ("ref", [value]) => {
            let value = target_argument(value, path)?;
            if crate::page_context::is_ui_evidence_ref(&value) {
                return Err(SpecError::new(
                    "test.spec.target_observation_only",
                    path,
                    "UI evidence refs are observation-only and cannot be action targets",
                ));
            }
            Ok(Target::Ref { value })
        }
        ("css", [value]) => Ok(Target::Css {
            selector: target_argument(value, path)?,
        }),
        ("role", [role, name]) => Ok(Target::Role {
            role: target_argument(role, path)?,
            name: target_argument(name, path)?,
        }),
        ("text", [value]) => Ok(Target::Text {
            value: target_argument(value, path)?,
            exact: false,
        }),
        ("text", [value, Value::Bool(exact)]) => Ok(Target::Text {
            value: target_argument(value, path)?,
            exact: *exact,
        }),
        ("automation_id", [value]) => Ok(Target::AutomationId {
            value: target_argument(value, path)?,
        }),
        ("visual_point", [snapshot, x, y]) => Ok(Target::VisualPoint {
            snapshot: target_argument(snapshot, path)?,
            x: target_coordinate(x, path)?,
            y: target_coordinate(y, path)?,
        }),
        ("testid", [value]) => Ok(Target::TestId {
            value: target_argument(value, path)?,
        }),
        ("label", [value]) => Ok(Target::Label {
            value: target_argument(value, path)?,
        }),
        ("placeholder", [value]) => Ok(Target::Placeholder {
            value: target_argument(value, path)?,
        }),
        _ => Err(SpecError::new(
            "test.spec.target_invalid",
            path,
            "unsupported locator function or argument count",
        )),
    }
}

fn target_coordinate(value: &Value, path: &str) -> Result<u32, SpecError> {
    let Some(number) = value.as_number() else {
        return Err(type_error(
            path,
            "visual point coordinates must be integers",
        ));
    };
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > f64::from(u32::MAX)
    {
        return Err(SpecError::new(
            "test.spec.target_invalid",
            path,
            "visual point coordinates must be unsigned 32-bit integers",
        ));
    }
    Ok(number as u32)
}

pub(super) fn target_argument(value: &Value, path: &str) -> Result<String, SpecError> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| type_error(path, "locator arguments must be strings"))
}
