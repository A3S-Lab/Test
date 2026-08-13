use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    ContractElement, ContractSeverity, PageContextNode, PageContextNodeState, PageContextTheme,
    SpecError, SurfaceContract, SurfaceObservation,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractOutcome {
    Passed,
    Failed,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractMatchStrategy {
    TestId,
    Component,
    RoleAndName,
    Role,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractMatch {
    pub element_id: String,
    pub node_id: String,
    pub strategy: ContractMatchStrategy,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ContractFinding {
    pub id: String,
    pub dimension: String,
    pub rule_id: String,
    pub severity: ContractSeverity,
    pub message: String,
    pub expected: Value,
    pub actual: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_node_id: Option<String>,
    pub confidence: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ContractReport {
    pub contract: String,
    pub variant: String,
    pub state: String,
    pub outcome: ContractOutcome,
    pub observation_revision: Option<u64>,
    pub matches: Vec<ContractMatch>,
    pub findings: Vec<ContractFinding>,
}

pub(crate) fn reconcile(
    contract: &SurfaceContract,
    variant_id: &str,
    state: &str,
    observation: &SurfaceObservation,
) -> Result<ContractReport, SpecError> {
    let variant = contract.variant(variant_id).ok_or_else(|| {
        SpecError::new(
            "test.contract.variant_unknown",
            format!("surface_contract.{}.variant.{variant_id}", contract.name),
            "the requested contract variant does not exist",
        )
    })?;
    if variant.state != state {
        return Err(SpecError::new(
            "test.contract.state_mismatch",
            format!(
                "surface_contract.{}.variant.{}.state",
                contract.name, variant.id
            ),
            format!(
                "the requested state '{state}' does not match variant state '{}'",
                variant.state
            ),
        ));
    }
    let revision = observation
        .page_context
        .as_ref()
        .and_then(|context| context.revision);
    let Some(context) = observation
        .page_context
        .as_ref()
        .filter(|context| context.present)
    else {
        return Ok(inconclusive_report(
            contract,
            variant_id,
            state,
            revision,
            "contract.observation.page_context_required",
            "a compatible Test Kit page-context observation is required for contract reconciliation",
        ));
    };
    let Some(snapshot) = context.snapshot.as_ref() else {
        return Ok(inconclusive_report(
            contract,
            variant_id,
            state,
            revision,
            "contract.observation.snapshot_required",
            "the page-context observation did not include a typed snapshot",
        ));
    };
    if snapshot.truncated {
        return Ok(inconclusive_report(
            contract,
            variant_id,
            state,
            revision,
            "contract.observation.truncated",
            "the page-context snapshot was truncated and cannot prove the complete contract",
        ));
    }
    let Some(page) = snapshot.page.as_ref() else {
        return Ok(inconclusive_report(
            contract,
            variant_id,
            state,
            revision,
            "contract.observation.page_required",
            "the page-context snapshot did not include page identity",
        ));
    };

    let mut findings = Vec::new();
    if variant
        .min_width
        .is_some_and(|minimum| page.viewport.width < f64::from(minimum))
        || variant
            .max_width
            .is_some_and(|maximum| page.viewport.width > f64::from(maximum))
    {
        findings.push(finding(
            "contract.variant.viewport_width",
            ContractSeverity::Important,
            "the observed viewport is outside the selected contract variant",
            json!({ "min": variant.min_width, "max": variant.max_width }),
            json!(page.viewport.width),
            None,
            None,
            100,
        ));
    }
    if let Some(theme) = variant.theme {
        if theme != page.theme {
            findings.push(finding(
                "contract.variant.theme",
                ContractSeverity::Important,
                "the observed theme does not match the selected contract variant",
                json!(theme_name(theme)),
                json!(theme_name(page.theme)),
                None,
                None,
                100,
            ));
        }
    }
    if let Some(language) = &variant.language {
        if language != &page.language {
            findings.push(finding(
                "contract.variant.language",
                ContractSeverity::Important,
                "the observed language does not match the selected contract variant",
                json!(language),
                json!(page.language),
                None,
                None,
                100,
            ));
        }
    }

    let nodes_by_id = snapshot
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let mut matches = Vec::new();
    let mut matched_nodes = BTreeMap::new();
    for element in &variant.elements {
        match match_element(element, &snapshot.nodes) {
            ElementResolution::Matched(node, strategy) => {
                matches.push(ContractMatch {
                    element_id: element.id.clone(),
                    node_id: node.id.clone(),
                    strategy,
                });
                matched_nodes.insert(element.id.as_str(), node);
                compare_element(element, node, &mut findings);
            }
            ElementResolution::Missing if element.required => findings.push(finding(
                "contract.element.required",
                element.severity,
                "a required contract element was not observed",
                json!(true),
                json!(false),
                Some(&element.id),
                None,
                100,
            )),
            ElementResolution::Missing => {}
            ElementResolution::Ambiguous(count) => findings.push(finding(
                "contract.element.ambiguous",
                element.severity,
                "multiple observed nodes matched one contract element",
                json!(1),
                json!(count),
                Some(&element.id),
                None,
                100,
            )),
        }
    }
    for element in &variant.elements {
        let (Some(parent_id), Some(child)) = (
            element.parent.as_deref(),
            matched_nodes.get(element.id.as_str()),
        ) else {
            continue;
        };
        let Some(parent) = matched_nodes.get(parent_id) else {
            continue;
        };
        if child.parent_id.as_deref() != Some(parent.id.as_str()) {
            let actual = child
                .parent_id
                .as_deref()
                .and_then(|id| nodes_by_id.get(id).copied())
                .map(|node| node.id.clone());
            findings.push(finding(
                "contract.element.parent",
                element.severity,
                "the observed element is not inside its contracted parent",
                json!(parent.id),
                json!(actual),
                Some(&element.id),
                Some(&child.id),
                100,
            ));
        }
    }

    matches.sort_by(|left, right| left.element_id.cmp(&right.element_id));
    findings.sort_by(|left, right| {
        left.element_id
            .cmp(&right.element_id)
            .then_with(|| left.rule_id.cmp(&right.rule_id))
            .then_with(|| left.observed_node_id.cmp(&right.observed_node_id))
    });
    for finding in &mut findings {
        finding.id = stable_finding_id(
            &contract.name,
            &variant.id,
            state,
            &finding.rule_id,
            finding.element_id.as_deref(),
        );
    }
    Ok(ContractReport {
        contract: contract.name.clone(),
        variant: variant.id.clone(),
        state: state.to_string(),
        outcome: if findings
            .iter()
            .any(|finding| finding.severity == ContractSeverity::Blocking)
        {
            ContractOutcome::Failed
        } else {
            ContractOutcome::Passed
        },
        observation_revision: revision,
        matches,
        findings,
    })
}

enum ElementResolution<'a> {
    Matched(&'a PageContextNode, ContractMatchStrategy),
    Missing,
    Ambiguous(usize),
}

fn match_element<'a>(
    element: &ContractElement,
    nodes: &'a [PageContextNode],
) -> ElementResolution<'a> {
    if let Some(test_id) = &element.test_id {
        return resolve_candidates(
            nodes
                .iter()
                .filter(|node| node.test_id.as_ref() == Some(test_id)),
            ContractMatchStrategy::TestId,
        );
    }
    if let Some(component_id) = &element.component_id {
        return resolve_candidates(
            nodes
                .iter()
                .filter(|node| node.component_id.as_ref() == Some(component_id))
                .filter(|node| optional_equals(node.role.as_ref(), element.role.as_ref()))
                .filter(|node| optional_equals(node.name.as_ref(), element.name.as_ref())),
            ContractMatchStrategy::Component,
        );
    }
    if let (Some(role), Some(name)) = (&element.role, &element.name) {
        return resolve_candidates(
            nodes.iter().filter(|node| {
                node.role.as_ref() == Some(role) && node.name.as_ref() == Some(name)
            }),
            ContractMatchStrategy::RoleAndName,
        );
    }
    resolve_candidates(
        nodes
            .iter()
            .filter(|node| node.role.as_ref() == element.role.as_ref()),
        ContractMatchStrategy::Role,
    )
}

fn resolve_candidates<'a>(
    candidates: impl Iterator<Item = &'a PageContextNode>,
    strategy: ContractMatchStrategy,
) -> ElementResolution<'a> {
    let matches = candidates.collect::<Vec<_>>();
    match matches.as_slice() {
        [] => ElementResolution::Missing,
        [node] => ElementResolution::Matched(node, strategy),
        values => ElementResolution::Ambiguous(values.len()),
    }
}

fn optional_equals<T: PartialEq>(actual: Option<&T>, expected: Option<&T>) -> bool {
    expected.is_none_or(|expected| actual == Some(expected))
}

fn compare_element(
    element: &ContractElement,
    node: &PageContextNode,
    findings: &mut Vec<ContractFinding>,
) {
    compare_optional(
        findings,
        element,
        node,
        "role",
        element.role.as_ref(),
        node.role.as_ref(),
    );
    compare_optional(
        findings,
        element,
        node,
        "name",
        element.name.as_ref(),
        node.name.as_ref(),
    );
    compare_optional(
        findings,
        element,
        node,
        "description",
        element.description.as_ref(),
        node.description.as_ref(),
    );
    compare_bool(
        findings,
        element,
        node,
        "visible",
        element.visible,
        Some(node.state.visible),
    );
    compare_bool(
        findings,
        element,
        node,
        "enabled",
        element.enabled,
        Some(!node.state.disabled.unwrap_or(false)),
    );
    compare_state(
        findings,
        element,
        node,
        "checked",
        element.checked,
        &node.state,
    );
    compare_state(
        findings,
        element,
        node,
        "selected",
        element.selected,
        &node.state,
    );
    compare_state(
        findings,
        element,
        node,
        "expanded",
        element.expanded,
        &node.state,
    );
    compare_state(
        findings,
        element,
        node,
        "readonly",
        element.readonly,
        &node.state,
    );
    compare_state(
        findings,
        element,
        node,
        "form_required",
        element.form_required,
        &node.state,
    );
    compare_state(
        findings,
        element,
        node,
        "invalid",
        element.invalid,
        &node.state,
    );
}

fn compare_optional(
    findings: &mut Vec<ContractFinding>,
    element: &ContractElement,
    node: &PageContextNode,
    property: &str,
    expected: Option<&String>,
    actual: Option<&String>,
) {
    if let Some(expected) = expected {
        if actual != Some(expected) {
            findings.push(finding(
                &format!("contract.element.{property}"),
                element.severity,
                &format!("the observed element {property} does not match the contract"),
                json!(expected),
                json!(actual),
                Some(&element.id),
                Some(&node.id),
                100,
            ));
        }
    }
}

fn compare_bool(
    findings: &mut Vec<ContractFinding>,
    element: &ContractElement,
    node: &PageContextNode,
    property: &str,
    expected: Option<bool>,
    actual: Option<bool>,
) {
    if expected.is_some() && expected != actual {
        findings.push(finding(
            &format!("contract.element.{property}"),
            element.severity,
            &format!("the observed element {property} state does not match the contract"),
            json!(expected),
            json!(actual),
            Some(&element.id),
            Some(&node.id),
            100,
        ));
    }
}

fn compare_state(
    findings: &mut Vec<ContractFinding>,
    element: &ContractElement,
    node: &PageContextNode,
    property: &str,
    expected: Option<bool>,
    state: &PageContextNodeState,
) {
    let actual = match property {
        "checked" => state.checked,
        "selected" => state.selected,
        "expanded" => state.expanded,
        "readonly" => state.readonly,
        "form_required" => state.required,
        "invalid" => state.invalid,
        _ => None,
    };
    compare_bool(findings, element, node, property, expected, actual);
}

#[allow(clippy::too_many_arguments)]
fn finding(
    rule_id: &str,
    severity: ContractSeverity,
    message: &str,
    expected: Value,
    actual: Value,
    element_id: Option<&str>,
    observed_node_id: Option<&str>,
    confidence: u8,
) -> ContractFinding {
    ContractFinding {
        id: String::new(),
        dimension: "design_conformance".to_string(),
        rule_id: rule_id.to_string(),
        severity,
        message: message.to_string(),
        expected,
        actual,
        element_id: element_id.map(str::to_string),
        observed_node_id: observed_node_id.map(str::to_string),
        confidence,
    }
}

fn inconclusive_report(
    contract: &SurfaceContract,
    variant: &str,
    state: &str,
    revision: Option<u64>,
    rule_id: &str,
    message: &str,
) -> ContractReport {
    let mut finding = finding(
        rule_id,
        ContractSeverity::Important,
        message,
        json!("complete typed observation"),
        Value::Null,
        None,
        None,
        100,
    );
    finding.id = stable_finding_id(&contract.name, variant, state, rule_id, None);
    ContractReport {
        contract: contract.name.clone(),
        variant: variant.to_string(),
        state: state.to_string(),
        outcome: ContractOutcome::Inconclusive,
        observation_revision: revision,
        matches: Vec::new(),
        findings: vec![finding],
    }
}

fn stable_finding_id(
    contract: &str,
    variant: &str,
    state: &str,
    rule_id: &str,
    element_id: Option<&str>,
) -> String {
    let mut digest = Sha256::new();
    for part in [contract, variant, state, rule_id, element_id.unwrap_or("")] {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    format!("finding:{:x}", digest.finalize())
}

fn theme_name(theme: PageContextTheme) -> &'static str {
    match theme {
        PageContextTheme::Light => "light",
        PageContextTheme::Dark => "dark",
        PageContextTheme::Unknown => "unknown",
    }
}
