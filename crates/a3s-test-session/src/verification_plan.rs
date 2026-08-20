use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path};

use a3s_test_core::{
    RepairAttempt, RepairFinding, RepairVerificationExpansionReason, RepairVerificationScope,
    RepairVerificationSlice, REPAIR_VERIFICATION_SLICE_PROTOCOL,
};

use crate::SessionError;

const MAX_VERIFICATION_CHECKS: usize = 50;
const MAX_CHECK_PREFIXES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairVerificationCheck {
    pub id: String,
    pub file_prefixes: Vec<String>,
    pub regression: bool,
}

#[must_use]
pub fn latest_prior_acl_proof_passed(
    attempts: &[RepairAttempt],
    active_attempt_id: &str,
) -> Option<bool> {
    attempts.iter().rev().find_map(|attempt| {
        (attempt.id != active_attempt_id)
            .then_some(attempt.verification.as_ref())
            .flatten()
            .and_then(|verification| verification.acl_proof.as_ref())
            .map(|proof| proof.passed)
    })
}

pub fn plan_repair_verification_slice(
    finding: &RepairFinding,
    changed_files: &[String],
    new_console_errors: u32,
    new_page_errors: u32,
    prior_acl_proof_passed: Option<bool>,
    checks: &[RepairVerificationCheck],
) -> Result<RepairVerificationSlice, SessionError> {
    validate_verification_checks(checks)?;
    let source_files = repair_source_files(finding);
    let stable_locator = finding
        .context
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| node.get("locators"))
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .any(|locator| acl_locator(locator).is_some());
    let mut expansion_reasons = Vec::new();
    if source_files.is_empty() {
        expansion_reasons.push(RepairVerificationExpansionReason::SourceMappingUnavailable);
    }
    if !stable_locator {
        expansion_reasons.push(RepairVerificationExpansionReason::StableLocatorUnavailable);
    }
    if changed_files.is_empty() {
        expansion_reasons.push(RepairVerificationExpansionReason::ChangedFilesUnavailable);
    }
    if !source_files.is_empty()
        && changed_files
            .iter()
            .any(|changed| !source_files.iter().any(|source| source == changed))
    {
        expansion_reasons.push(RepairVerificationExpansionReason::ChangedFileOutsideSourceMapping);
    }
    let focused_coverage = focused_check_coverage(changed_files, checks);
    if !checks.is_empty()
        && changed_files
            .iter()
            .enumerate()
            .any(|(index, _)| !focused_coverage.contains(&index))
    {
        expansion_reasons.push(RepairVerificationExpansionReason::ProjectCheckCoverageMissing);
    }
    if new_console_errors > 0 || new_page_errors > 0 {
        expansion_reasons.push(RepairVerificationExpansionReason::NewBrowserErrors);
    }
    if prior_acl_proof_passed == Some(false) {
        expansion_reasons.push(RepairVerificationExpansionReason::PriorProofFailed);
    }
    let scope = if expansion_reasons.is_empty() {
        RepairVerificationScope::Focused
    } else {
        RepairVerificationScope::Expanded
    };
    let selected_checks = match scope {
        RepairVerificationScope::Focused => select_focused_checks(changed_files, checks),
        RepairVerificationScope::Expanded => {
            let regression = checks
                .iter()
                .filter(|check| check.regression)
                .map(|check| check.id.clone())
                .collect::<Vec<_>>();
            if regression.is_empty() {
                checks.iter().map(|check| check.id.clone()).collect()
            } else {
                regression
            }
        }
    };

    Ok(RepairVerificationSlice {
        protocol: REPAIR_VERIFICATION_SLICE_PROTOCOL.to_string(),
        scope,
        source_files,
        stable_locator,
        prior_acl_proof_passed,
        selected_checks,
        expansion_reasons,
    })
}

pub(crate) fn acl_locator(value: &serde_json::Value) -> Option<String> {
    match value.get("type")?.as_str()? {
        "test_id" => value
            .get("value")
            .and_then(serde_json::Value::as_str)
            .map(|value| format!("testid(\"{}\")", acl_string(value))),
        "role" => Some(format!(
            "role(\"{}\", \"{}\")",
            acl_string(value.get("role")?.as_str()?),
            acl_string(value.get("name")?.as_str()?)
        )),
        "label" => value
            .get("value")
            .and_then(serde_json::Value::as_str)
            .map(|value| format!("label(\"{}\")", acl_string(value))),
        "placeholder" => value
            .get("value")
            .and_then(serde_json::Value::as_str)
            .map(|value| format!("placeholder(\"{}\")", acl_string(value))),
        _ => None,
    }
}

pub(crate) fn acl_string(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' | '\r' => " ".chars().collect(),
            value => vec![value],
        })
        .collect()
}

fn validate_verification_checks(checks: &[RepairVerificationCheck]) -> Result<(), SessionError> {
    let mut ids = HashSet::new();
    let invalid = checks.len() > MAX_VERIFICATION_CHECKS
        || checks.iter().any(|check| {
            check.id.is_empty()
                || check.id.len() > 64
                || !check.id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                })
                || !ids.insert(check.id.as_str())
                || check.file_prefixes.len() > MAX_CHECK_PREFIXES
                || (!check.regression && check.file_prefixes.is_empty())
                || (check.regression && !check.file_prefixes.is_empty())
                || check.file_prefixes.iter().any(|prefix| {
                    prefix.is_empty()
                        || prefix.len() > 1_024
                        || Path::new(prefix).is_absolute()
                        || Path::new(prefix)
                            .components()
                            .any(|component| !matches!(component, Component::Normal(_)))
                })
        });
    if invalid {
        return Err(SessionError::new(
            "test.session.repair_verify_plan_invalid",
            "verification checks require unique bounded identifiers, contained focused prefixes, and unscoped regression entries",
        ));
    }
    Ok(())
}

fn repair_source_files(finding: &RepairFinding) -> Vec<String> {
    let mut files = BTreeSet::new();
    let mut admit = |value: &serde_json::Value| {
        if let Some(file) = value.as_str().filter(|file| contained_relative_path(file)) {
            files.insert(file.to_string());
        }
    };
    for node in finding
        .context
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        for candidate in node
            .get("sourceMapping")
            .and_then(|mapping| mapping.get("candidates"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(file) = candidate.pointer("/span/file") {
                admit(file);
            }
            if let Some(file) = candidate.pointer("/generatedSpan/file") {
                admit(file);
            }
        }
    }
    for component in finding
        .context
        .get("components")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(file) = component.pointer("/source/file") {
            admit(file);
        }
    }
    files.into_iter().collect()
}

fn contained_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1_024
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn focused_check_coverage(
    changed_files: &[String],
    checks: &[RepairVerificationCheck],
) -> BTreeSet<usize> {
    changed_files
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            checks
                .iter()
                .filter(|check| !check.regression)
                .any(|check| check_matches_file(check, file))
                .then_some(index)
        })
        .collect()
}

fn select_focused_checks(
    changed_files: &[String],
    checks: &[RepairVerificationCheck],
) -> Vec<String> {
    let mut uncovered = (0..changed_files.len()).collect::<BTreeSet<_>>();
    let mut selected = Vec::new();
    while !uncovered.is_empty() {
        let best = checks
            .iter()
            .enumerate()
            .filter(|(_, check)| !check.regression && !selected.contains(&check.id))
            .map(|(index, check)| {
                let covered = uncovered
                    .iter()
                    .filter(|file| check_matches_file(check, &changed_files[**file]))
                    .copied()
                    .collect::<Vec<_>>();
                (index, check, covered)
            })
            .filter(|(_, _, covered)| !covered.is_empty())
            .max_by(|left, right| {
                left.2
                    .len()
                    .cmp(&right.2.len())
                    .then_with(|| right.0.cmp(&left.0))
            });
        let Some((_, check, covered)) = best else {
            break;
        };
        selected.push(check.id.clone());
        for file in covered {
            uncovered.remove(&file);
        }
    }
    selected
}

fn check_matches_file(check: &RepairVerificationCheck, file: &str) -> bool {
    check
        .file_prefixes
        .iter()
        .any(|prefix| Path::new(file).starts_with(Path::new(prefix)))
}

#[cfg(test)]
mod tests {
    use a3s_test_core::{RepairVerificationExpansionReason, RepairVerificationScope};

    use super::*;

    #[test]
    fn plans_the_smallest_source_bound_verification_slice() {
        let finding = mapped_finding(&["src/Checkout.tsx", "src/Price.tsx"]);
        let checks = vec![
            check("checkout", &["src/Checkout.tsx"], false),
            check("price", &["src/Price.tsx"], false),
            check("component", &["src"], false),
            check("workspace", &[], true),
        ];

        let slice = plan_repair_verification_slice(
            &finding,
            &["src/Checkout.tsx".to_string(), "src/Price.tsx".to_string()],
            0,
            0,
            None,
            &checks,
        )
        .expect("focused verification slice");

        assert_eq!(slice.scope, RepairVerificationScope::Focused);
        assert_eq!(slice.source_files, ["src/Checkout.tsx", "src/Price.tsx"]);
        assert!(slice.stable_locator);
        assert_eq!(slice.prior_acl_proof_passed, None);
        assert!(slice.expansion_reasons.is_empty());
        assert_eq!(slice.selected_checks, ["component"]);
    }

    #[test]
    fn expands_verification_only_for_observed_impact_evidence() {
        let finding = mapped_finding(&["src/Checkout.tsx"]);
        let checks = vec![
            check("checkout", &["src/Checkout.tsx"], false),
            check("workspace", &[], true),
        ];

        let slice = plan_repair_verification_slice(
            &finding,
            &[
                "src/Checkout.tsx".to_string(),
                "shared/theme.css".to_string(),
            ],
            1,
            0,
            Some(false),
            &checks,
        )
        .expect("expanded verification slice");

        assert_eq!(slice.scope, RepairVerificationScope::Expanded);
        assert_eq!(slice.selected_checks, ["workspace"]);
        assert_eq!(slice.prior_acl_proof_passed, Some(false));
        assert_eq!(
            slice.expansion_reasons,
            [
                RepairVerificationExpansionReason::ChangedFileOutsideSourceMapping,
                RepairVerificationExpansionReason::ProjectCheckCoverageMissing,
                RepairVerificationExpansionReason::NewBrowserErrors,
                RepairVerificationExpansionReason::PriorProofFailed,
            ]
        );
    }

    #[test]
    fn rejects_ambiguous_or_unsafe_verification_check_catalogs() {
        let finding = mapped_finding(&["src/Checkout.tsx"]);
        let duplicate = vec![check("same", &["src"], false), check("same", &[], true)];
        let error = plan_repair_verification_slice(
            &finding,
            &["src/Checkout.tsx".to_string()],
            0,
            0,
            None,
            &duplicate,
        )
        .expect_err("duplicate check identifiers must fail");
        assert_eq!(error.code(), "test.session.repair_verify_plan_invalid");

        let escaping = [check("escape", &["../outside"], false)];
        let error = plan_repair_verification_slice(
            &finding,
            &["src/Checkout.tsx".to_string()],
            0,
            0,
            None,
            &escaping,
        )
        .expect_err("escaping file prefixes must fail");
        assert_eq!(error.code(), "test.session.repair_verify_plan_invalid");

        let scoped_regression = [check("regression", &["src"], true)];
        let error = plan_repair_verification_slice(
            &finding,
            &["src/Checkout.tsx".to_string()],
            0,
            0,
            None,
            &scoped_regression,
        )
        .expect_err("regression checks must remain unscoped");
        assert_eq!(error.code(), "test.session.repair_verify_plan_invalid");
    }

    fn check(id: &str, prefixes: &[&str], regression: bool) -> RepairVerificationCheck {
        RepairVerificationCheck {
            id: id.to_string(),
            file_prefixes: prefixes
                .iter()
                .map(|prefix| (*prefix).to_string())
                .collect(),
            regression,
        }
    }

    fn mapped_finding(files: &[&str]) -> RepairFinding {
        let candidates = files
            .iter()
            .enumerate()
            .map(|(index, file)| {
                serde_json::json!({
                    "span": { "file": file, "line": index + 1 },
                    "confidence": 1.0 - (index as f64 * 0.1),
                    "origin": "boundary_hint",
                    "relation": "exact",
                    "registrationId": format!("source-{index}"),
                })
            })
            .collect::<Vec<_>>();
        serde_json::from_value(serde_json::json!({
            "id": "finding-1",
            "batchId": "batch-1",
            "instruction": "Repair checkout",
            "successCriteria": "Checkout is repaired",
            "intent": "fix",
            "severity": "important",
            "target": {
                "kind": "node",
                "nodeIds": ["n1"],
                "selectedText": null,
                "region": null,
                "drawing": null
            },
            "createdAt": "2026-08-20T00:00:00Z",
            "pageId": "checkout",
            "url": "http://127.0.0.1:5173/checkout",
            "contextRevision": 1,
            "context": {
                "nodes": [{
                    "id": "n1",
                    "locators": [{ "type": "test_id", "value": "checkout" }],
                    "sourceMapping": {
                        "protocol": "a3s.test.source-mapping/1",
                        "candidates": candidates,
                        "truncated": false
                    }
                }]
            },
            "status": "queued",
            "submittedAt": "2026-08-20T00:00:01Z"
        }))
        .expect("mapped finding")
    }
}
