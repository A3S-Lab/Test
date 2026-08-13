use std::collections::HashSet;

use a3s_acl::{Block, Value};

use super::{
    AdmittedProvenance, ContractContext, ContractElement, ContractMode, ContractProvenanceKind,
    ContractProvenanceStatus, ContractSeverity, ContractVariant, SurfaceContractDraft,
};
use crate::{PageContextTheme, SpecError};

impl SurfaceContractDraft {
    pub fn from_acl(source: &str) -> Result<Self, SpecError> {
        let document = a3s_acl::parse(source).map_err(|error| {
            SpecError::new(
                "test.contract.syntax",
                "document",
                format!("invalid ACL document: {error}"),
            )
        })?;
        if document.blocks.len() != 1 || document.blocks[0].name != "surface_contract" {
            return Err(SpecError::new(
                "test.contract.root_required",
                "document",
                "the document must contain exactly one surface_contract block",
            ));
        }
        parse_contract(&document.blocks[0])
    }
}

fn parse_contract(block: &Block) -> Result<SurfaceContractDraft, SpecError> {
    let name = one_label(block, "surface_contract")?.to_string();
    validate_identifier(&name, "surface_contract")?;
    let path = format!("surface_contract.{name}");
    ensure_attributes(block, &["version"], &path)?;
    let version = optional_u32(block, "version", 1, &path)?;

    let mut context = None;
    let mut provenance = Vec::new();
    let mut provenance_ids = HashSet::new();
    let mut variants = Vec::new();
    let mut variant_ids = HashSet::new();
    for child in &block.blocks {
        match child.name.as_str() {
            "context" if context.is_none() => context = Some(parse_context(child, &path)?),
            "context" => {
                return Err(SpecError::new(
                    "test.contract.context_duplicate",
                    format!("{path}.context"),
                    "a surface contract must contain exactly one context block",
                ));
            }
            "provenance" => {
                let item = parse_provenance(child, &path)?;
                if !provenance_ids.insert(item.id.clone()) {
                    return Err(SpecError::new(
                        "test.contract.provenance_duplicate",
                        format!("{path}.provenance.{}", item.id),
                        "provenance identifiers must be unique",
                    ));
                }
                provenance.push(item);
            }
            "variant" => {
                let variant = parse_variant(child, &path)?;
                if !variant_ids.insert(variant.id.clone()) {
                    return Err(SpecError::new(
                        "test.contract.variant_duplicate",
                        format!("{path}.variant.{}", variant.id),
                        "variant identifiers must be unique",
                    ));
                }
                variants.push(variant);
            }
            _ => {
                return Err(SpecError::new(
                    "test.contract.block_unknown",
                    format!("{path}.{}", child.name),
                    "only context, provenance, and variant blocks are allowed",
                ));
            }
        }
    }

    Ok(SurfaceContractDraft {
        name,
        version,
        context: context.ok_or_else(|| {
            SpecError::new(
                "test.contract.context_required",
                &path,
                "a surface contract requires one context block",
            )
        })?,
        provenance,
        variants,
    })
}

fn parse_context(block: &Block, parent: &str) -> Result<ContractContext, SpecError> {
    let path = format!("{parent}.context");
    no_labels_or_blocks(block, &path)?;
    ensure_attributes(block, &["mode", "audience", "primary_outcome"], &path)?;
    let mode = match required_string(block, "mode", &path)? {
        "persuade" => ContractMode::Persuade,
        "operate" => ContractMode::Operate,
        "read" => ContractMode::Read,
        "experience" => ContractMode::Experience,
        _ => {
            return Err(SpecError::new(
                "test.contract.mode_unknown",
                format!("{path}.mode"),
                "mode must be persuade, operate, read, or experience",
            ));
        }
    };
    let audience = required_string_list(block, "audience", &path)?;
    if audience.is_empty() {
        return Err(SpecError::new(
            "test.contract.audience_required",
            format!("{path}.audience"),
            "context audience must contain at least one entry",
        ));
    }
    Ok(ContractContext {
        mode,
        audience,
        primary_outcome: required_nonempty_string(block, "primary_outcome", &path)?,
    })
}

fn parse_provenance(block: &Block, parent: &str) -> Result<AdmittedProvenance, SpecError> {
    let id = one_label(block, "provenance")?.to_string();
    validate_identifier(&id, parent)?;
    let path = format!("{parent}.provenance.{id}");
    ensure_no_blocks(block, &path)?;
    ensure_attributes(
        block,
        &["kind", "uri", "digest", "status", "confidence"],
        &path,
    )?;
    let kind = match required_string(block, "kind", &path)? {
        "prd" => ContractProvenanceKind::Prd,
        "design" => ContractProvenanceKind::Design,
        "manual" => ContractProvenanceKind::Manual,
        "official_docs" => ContractProvenanceKind::OfficialDocs,
        _ => {
            return Err(SpecError::new(
                "test.contract.provenance_kind_unknown",
                format!("{path}.kind"),
                "provenance kind must be prd, design, manual, or official_docs",
            ));
        }
    };
    let status = match required_string(block, "status", &path)? {
        "draft" => ContractProvenanceStatus::Draft,
        "reviewed" => ContractProvenanceStatus::Reviewed,
        _ => {
            return Err(SpecError::new(
                "test.contract.provenance_status_unknown",
                format!("{path}.status"),
                "provenance status must be draft or reviewed",
            ));
        }
    };
    let confidence = required_u32(block, "confidence", &path)?;
    if confidence > 100 {
        return Err(SpecError::new(
            "test.contract.confidence_range",
            format!("{path}.confidence"),
            "provenance confidence must be between 0 and 100",
        ));
    }
    Ok(AdmittedProvenance {
        id,
        kind,
        uri: required_nonempty_string(block, "uri", &path)?,
        digest: required_nonempty_string(block, "digest", &path)?,
        status,
        confidence: confidence as u8,
    })
}

fn parse_variant(block: &Block, parent: &str) -> Result<ContractVariant, SpecError> {
    let id = one_label(block, "variant")?.to_string();
    validate_identifier(&id, parent)?;
    let path = format!("{parent}.variant.{id}");
    ensure_attributes(
        block,
        &["state", "min_width", "max_width", "theme", "language"],
        &path,
    )?;
    let theme = optional_string(block, "theme", &path)?
        .map(|theme| match theme.as_str() {
            "light" => Ok(PageContextTheme::Light),
            "dark" => Ok(PageContextTheme::Dark),
            "unknown" => Ok(PageContextTheme::Unknown),
            _ => Err(SpecError::new(
                "test.contract.theme_unknown",
                format!("{path}.theme"),
                "theme must be light, dark, or unknown",
            )),
        })
        .transpose()?;
    let mut elements = Vec::new();
    let mut element_ids = HashSet::new();
    for child in &block.blocks {
        if child.name != "element" {
            return Err(SpecError::new(
                "test.contract.block_unknown",
                format!("{path}.{}", child.name),
                "only element blocks are allowed inside a variant",
            ));
        }
        let element = parse_element(child, &path)?;
        if !element_ids.insert(element.id.clone()) {
            return Err(SpecError::new(
                "test.contract.element_duplicate",
                format!("{path}.element.{}", element.id),
                "element identifiers must be unique inside a variant",
            ));
        }
        elements.push(element);
    }
    Ok(ContractVariant {
        id,
        state: required_nonempty_string(block, "state", &path)?,
        min_width: optional_u32_attribute(block, "min_width", &path)?,
        max_width: optional_u32_attribute(block, "max_width", &path)?,
        theme,
        language: optional_string(block, "language", &path)?,
        elements,
    })
}

fn parse_element(block: &Block, parent: &str) -> Result<ContractElement, SpecError> {
    let id = one_label(block, "element")?.to_string();
    validate_identifier(&id, parent)?;
    let path = format!("{parent}.element.{id}");
    ensure_no_blocks(block, &path)?;
    ensure_attributes(
        block,
        &[
            "test_id",
            "component_id",
            "role",
            "name",
            "description",
            "required",
            "visible",
            "enabled",
            "checked",
            "selected",
            "expanded",
            "readonly",
            "form_required",
            "invalid",
            "parent",
            "severity",
        ],
        &path,
    )?;
    let severity = match optional_string(block, "severity", &path)?.as_deref() {
        None | Some("important") => ContractSeverity::Important,
        Some("blocking") => ContractSeverity::Blocking,
        Some("suggestion") => ContractSeverity::Suggestion,
        Some(_) => {
            return Err(SpecError::new(
                "test.contract.severity_unknown",
                format!("{path}.severity"),
                "severity must be blocking, important, or suggestion",
            ));
        }
    };
    Ok(ContractElement {
        id,
        test_id: optional_nonempty_string(block, "test_id", &path)?,
        component_id: optional_nonempty_string(block, "component_id", &path)?,
        role: optional_nonempty_string(block, "role", &path)?,
        name: optional_nonempty_string(block, "name", &path)?,
        description: optional_nonempty_string(block, "description", &path)?,
        required: optional_bool(block, "required", true, &path)?,
        visible: optional_bool_attribute(block, "visible", &path)?,
        enabled: optional_bool_attribute(block, "enabled", &path)?,
        checked: optional_bool_attribute(block, "checked", &path)?,
        selected: optional_bool_attribute(block, "selected", &path)?,
        expanded: optional_bool_attribute(block, "expanded", &path)?,
        readonly: optional_bool_attribute(block, "readonly", &path)?,
        form_required: optional_bool_attribute(block, "form_required", &path)?,
        invalid: optional_bool_attribute(block, "invalid", &path)?,
        parent: optional_nonempty_string(block, "parent", &path)?,
        severity,
    })
}

fn one_label<'a>(block: &'a Block, path: &str) -> Result<&'a str, SpecError> {
    match block.labels.as_slice() {
        [label] if !label.trim().is_empty() => Ok(label),
        _ => Err(SpecError::new(
            "test.contract.label_required",
            path,
            "block requires exactly one non-empty label",
        )),
    }
}

fn no_labels_or_blocks(block: &Block, path: &str) -> Result<(), SpecError> {
    if !block.labels.is_empty() {
        return Err(SpecError::new(
            "test.contract.label_unexpected",
            path,
            "this block does not accept labels",
        ));
    }
    ensure_no_blocks(block, path)
}

fn ensure_no_blocks(block: &Block, path: &str) -> Result<(), SpecError> {
    if block.blocks.is_empty() {
        Ok(())
    } else {
        Err(SpecError::new(
            "test.contract.nested_block_unknown",
            path,
            "this block does not accept nested blocks",
        ))
    }
}

fn ensure_attributes(block: &Block, allowed: &[&str], path: &str) -> Result<(), SpecError> {
    if let Some(name) = block
        .attributes
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        return Err(SpecError::new(
            "test.contract.attribute_unknown",
            format!("{path}.{name}"),
            "unsupported contract attribute",
        ));
    }
    Ok(())
}

fn required_string<'a>(block: &'a Block, name: &str, path: &str) -> Result<&'a str, SpecError> {
    block
        .attributes
        .get(name)
        .ok_or_else(|| required_attribute(path, name))?
        .as_str()
        .ok_or_else(|| type_error(path, name, "attribute must be a string"))
}

fn required_nonempty_string(block: &Block, name: &str, path: &str) -> Result<String, SpecError> {
    let value = required_string(block, name, path)?.trim();
    if value.is_empty() {
        return Err(SpecError::new(
            "test.contract.string_empty",
            format!("{path}.{name}"),
            "attribute must not be empty",
        ));
    }
    Ok(value.to_string())
}

fn optional_string(block: &Block, name: &str, path: &str) -> Result<Option<String>, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| type_error(path, name, "attribute must be a string"))
        })
        .transpose()
}

fn optional_nonempty_string(
    block: &Block,
    name: &str,
    path: &str,
) -> Result<Option<String>, SpecError> {
    optional_string(block, name, path)?
        .map(|value| {
            if value.trim().is_empty() {
                Err(SpecError::new(
                    "test.contract.string_empty",
                    format!("{path}.{name}"),
                    "attribute must not be empty",
                ))
            } else {
                Ok(value)
            }
        })
        .transpose()
}

fn required_string_list(block: &Block, name: &str, path: &str) -> Result<Vec<String>, SpecError> {
    let value = block
        .attributes
        .get(name)
        .ok_or_else(|| required_attribute(path, name))?;
    let Value::List(values) = value else {
        return Err(type_error(path, name, "attribute must be a string list"));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| type_error(path, name, "attribute must contain non-empty strings"))
        })
        .collect()
}

fn required_u32(block: &Block, name: &str, path: &str) -> Result<u32, SpecError> {
    let value = block
        .attributes
        .get(name)
        .ok_or_else(|| required_attribute(path, name))?;
    u32_value(value, path, name)
}

fn optional_u32(block: &Block, name: &str, default: u32, path: &str) -> Result<u32, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| u32_value(value, path, name))
        .unwrap_or(Ok(default))
}

fn optional_u32_attribute(block: &Block, name: &str, path: &str) -> Result<Option<u32>, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| u32_value(value, path, name))
        .transpose()
}

fn u32_value(value: &Value, path: &str, name: &str) -> Result<u32, SpecError> {
    let number = value
        .as_number()
        .ok_or_else(|| type_error(path, name, "attribute must be an integer"))?;
    if !number.is_finite()
        || number.fract() != 0.0
        || !(0.0..=f64::from(u32::MAX)).contains(&number)
    {
        return Err(SpecError::new(
            "test.contract.number_range",
            format!("{path}.{name}"),
            "integer is outside the supported range",
        ));
    }
    Ok(number as u32)
}

fn optional_bool(block: &Block, name: &str, default: bool, path: &str) -> Result<bool, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| type_error(path, name, "attribute must be a boolean"))
        })
        .unwrap_or(Ok(default))
}

fn optional_bool_attribute(
    block: &Block,
    name: &str,
    path: &str,
) -> Result<Option<bool>, SpecError> {
    block
        .attributes
        .get(name)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| type_error(path, name, "attribute must be a boolean"))
        })
        .transpose()
}

fn required_attribute(path: &str, name: &str) -> SpecError {
    SpecError::new(
        "test.contract.attribute_required",
        format!("{path}.{name}"),
        "required contract attribute is missing",
    )
}

fn type_error(path: &str, name: &str, message: &str) -> SpecError {
    SpecError::new("test.contract.type", format!("{path}.{name}"), message)
}

fn validate_identifier(value: &str, path: &str) -> Result<(), SpecError> {
    if !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Ok(());
    }
    Err(SpecError::new(
        "test.contract.identifier_invalid",
        format!("{path}.{value}"),
        "identifier must contain only ASCII letters, digits, hyphens, or underscores",
    ))
}
