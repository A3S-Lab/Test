use std::path::{Component, Path};

use a3s_test_core::{
    PageContextSnapshot, RepairCheckStatus, RepairEvidenceBundle, RepairFinding,
    RepairVerification, TestSuite,
};

use crate::{RepairVerifyRequest, SessionError};

pub fn validate_repair_verification_request(
    request: &RepairVerifyRequest,
) -> Result<(), SessionError> {
    crate::protocol::validate_session_id(&request.session)?;
    if request.finding_id.trim().is_empty()
        || request.request_id.trim().is_empty()
        || request.summary.trim().is_empty()
        || request.summary.len() > 8_192
    {
        return Err(SessionError::new(
            "test.session.repair_verify_invalid",
            "verification finding id, request id, and summary must be bounded and non-empty",
        ));
    }
    if request.changed_files.len() > 200
        || request.checks.len() > 50
        || request.changed_files.iter().any(|path| {
            path.is_empty()
                || path.len() > 1_024
                || Path::new(path).is_absolute()
                || Path::new(path)
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
        })
        || request.checks.iter().any(|check| {
            check.command.trim().is_empty()
                || check.command.len() > 4_096
                || check.summary.trim().is_empty()
                || check.summary.len() > 8_192
        })
        || request
            .acl_candidate
            .as_ref()
            .is_some_and(|candidate| candidate.len() > 1_048_576)
    {
        return Err(SessionError::new(
            "test.session.repair_verify_invalid",
            "verification files, checks, or ACL candidate exceed bounded protocol limits",
        ));
    }
    Ok(())
}

pub fn build_repair_verification(
    finding: &RepairFinding,
    attempt_id: &str,
    before_evidence: &RepairEvidenceBundle,
    after_evidence: &RepairEvidenceBundle,
    request: &RepairVerifyRequest,
) -> Result<RepairVerification, SessionError> {
    validate_repair_verification_request(request)?;
    let snapshot = &after_evidence.context;
    let console_errors = after_evidence.console_errors;
    let page_errors = after_evidence.page_errors;
    let before_revision = before_evidence.context_revision;
    let after_revision = snapshot.revision.unwrap_or_default();
    if after_revision <= before_revision || !snapshot.page.as_ref().is_some_and(|page| page.ready) {
        return Err(SessionError::new(
            "test.session.repair_verify_not_ready",
            "repair verification requires a newer ready page revision",
        )
        .with_retryable(true));
    }

    let target_found = repair_target_found(finding, snapshot);
    let success_criteria_passed = request
        .success_criteria_passed
        .or_else(|| finding.success_criteria.is_none().then_some(target_found));
    if before_revision < finding.context_revision
        || before_evidence.context.revision != Some(before_revision)
        || !before_evidence
            .context
            .page
            .as_ref()
            .is_some_and(|page| page.ready)
        || after_evidence.context_revision != after_revision
        || console_errors != after_evidence.console_errors
        || page_errors != after_evidence.page_errors
    {
        return Err(SessionError::new(
            "test.session.repair_verify_evidence_invalid",
            "repair verification evidence is not bound to the before and after page revisions",
        ));
    }
    let new_console_errors = console_errors.saturating_sub(before_evidence.console_errors);
    let new_page_errors = page_errors.saturating_sub(before_evidence.page_errors);
    let checks_passed = request
        .checks
        .iter()
        .all(|check| check.status != RepairCheckStatus::Failed);
    let passed = target_found
        && success_criteria_passed == Some(true)
        && new_console_errors == 0
        && new_page_errors == 0
        && checks_passed;
    let acl_candidate = match request.acl_candidate.as_ref() {
        Some(candidate) => {
            TestSuite::from_repair_acl(candidate, &finding.url).map_err(|error| {
                SessionError::new(
                    "test.session.repair_acl_invalid",
                    format!("repair ACL candidate is invalid: {}", error.message()),
                )
            })?;
            Some(candidate.clone())
        }
        None => generate_acl_candidate(finding),
    };

    Ok(RepairVerification {
        finding_id: finding.id.clone(),
        attempt_id: attempt_id.to_string(),
        before_revision,
        after_revision,
        target_found,
        success_criteria_passed,
        new_console_errors,
        new_page_errors,
        changed_files: request.changed_files.clone(),
        checks: request.checks.clone(),
        acl_candidate,
        acl_proof: None,
        before_evidence: Some(before_evidence.clone()),
        after_evidence: Some(after_evidence.clone()),
        passed,
        summary: request.summary.clone(),
    })
}

fn repair_target_found(finding: &RepairFinding, snapshot: &PageContextSnapshot) -> bool {
    let target_ids = finding
        .target
        .node_ids
        .iter()
        .collect::<std::collections::HashSet<_>>();
    if !target_ids.is_empty()
        && snapshot
            .nodes
            .iter()
            .any(|node| target_ids.contains(&node.id))
    {
        return true;
    }
    let context_locators = finding
        .context
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| node.get("locators"))
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .collect::<Vec<_>>();
    snapshot.nodes.iter().any(|node| {
        node.locators.iter().any(|locator| {
            serde_json::to_value(locator)
                .ok()
                .is_some_and(|encoded| context_locators.contains(&&encoded))
        })
    })
}

fn generate_acl_candidate(finding: &RepairFinding) -> Option<String> {
    let locator = finding
        .context
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| node.get("locators"))
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .find_map(acl_locator)?;
    let criterion = finding.success_criteria.as_deref()?.trim();
    let escaped_url = acl_string(&finding.url);
    let escaped_criterion = acl_string(criterion);
    let candidate = format!(
        "suite \"repair-{}\" {{\n    version = 1\n\n    scenario \"regression\" {{\n        name = \"Repair regression\"\n        surface = \"web\"\n        timeout_ms = 30000\n\n        navigate \"open\" {{\n            url = \"{escaped_url}\"\n        }}\n\n        expect \"target\" {{\n            visible = {locator}\n        }}\n\n        expect \"success\" {{\n            text = \"{escaped_criterion}\"\n        }}\n    }}\n}}\n",
        finding
            .id
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
            .take(48)
            .collect::<String>()
    );
    TestSuite::from_repair_acl(&candidate, &finding.url)
        .is_ok()
        .then_some(candidate)
}

fn acl_locator(value: &serde_json::Value) -> Option<String> {
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

fn acl_string(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_test_core::{
        Evidence, PageContextPage, PageContextPoint, PageContextSize, PageContextTheme,
        PageContextViewport, RepairIntent, RepairSeverity, RepairStatus, RepairTarget,
        RepairTargetKind,
    };
    use serde_json::json;

    fn finding() -> RepairFinding {
        RepairFinding {
            id: "finding-1".to_string(),
            batch_id: "batch-1".to_string(),
            instruction: "Fix the target".to_string(),
            success_criteria: Some("The target is repaired".to_string()),
            intent: RepairIntent::Fix,
            severity: RepairSeverity::Important,
            relations: Vec::new(),
            target: RepairTarget {
                kind: RepairTargetKind::Node,
                node_ids: vec!["n1".to_string()],
                selected_text: None,
                region: None,
                drawing: None,
            },
            created_at: "2026-08-13T00:00:00Z".to_string(),
            page_id: "page".to_string(),
            url: "http://127.0.0.1/".to_string(),
            context_revision: 1,
            context: json!({ "untrusted": true }),
            status: RepairStatus::Queued,
            submitted_at: "2026-08-13T00:00:01Z".to_string(),
        }
    }

    fn snapshot() -> PageContextSnapshot {
        PageContextSnapshot {
            protocol: Some("a3s.test.page-context/1".to_string()),
            sdk_version: Some("0.1.0".to_string()),
            revision: Some(2),
            page: Some(PageContextPage {
                id: "page".to_string(),
                url: "http://127.0.0.1/".to_string(),
                route: "/".to_string(),
                title: "Page".to_string(),
                ready: true,
                viewport: PageContextViewport {
                    width: 100.0,
                    height: 100.0,
                    dpr: 1.0,
                    visual: None,
                },
                document: PageContextSize {
                    width: 100.0,
                    height: 100.0,
                },
                scroll: PageContextPoint { x: 0.0, y: 0.0 },
                language: "en".to_string(),
                theme: PageContextTheme::Light,
            }),
            components: vec![],
            nodes: vec![],
            facts: Default::default(),
            removed_node_ids: vec![],
            truncated: false,
            next_cursor: None,
        }
    }

    fn request(success_criteria_passed: Option<bool>) -> RepairVerifyRequest {
        RepairVerifyRequest {
            session: "session".to_string(),
            finding_id: "finding-1".to_string(),
            request_id: "verify-1".to_string(),
            success_criteria_passed,
            changed_files: vec!["src/page.tsx".to_string()],
            checks: vec![],
            acl_candidate: None,
            summary: "Verification finished".to_string(),
        }
    }

    fn evidence(revision: u64) -> RepairEvidenceBundle {
        let mut context = snapshot();
        context.revision = Some(revision);
        RepairEvidenceBundle {
            captured_at_ms: revision,
            context_revision: revision,
            context_sha256: "a".repeat(64),
            context,
            console_errors: 0,
            page_errors: 0,
            screenshot: Evidence {
                name: "repair".to_string(),
                path: "repairs/finding-1/attempt-1/evidence.png".to_string(),
                media_type: "image/png".to_string(),
            },
            screenshot_sha256: "b".repeat(64),
        }
    }

    #[test]
    fn explicit_success_criteria_require_an_explicit_positive_result() {
        let mut snapshot = snapshot();
        snapshot.nodes.push(a3s_test_core::PageContextNode {
            id: "n1".to_string(),
            r#ref: None,
            parent_id: None,
            component_id: None,
            tag: "button".to_string(),
            role: None,
            name: None,
            text: None,
            description: None,
            test_id: None,
            geometry: None,
            state: a3s_test_core::PageContextNodeState {
                visible: true,
                disabled: None,
                checked: None,
                selected: None,
                expanded: None,
                focused: None,
                readonly: None,
                required: None,
                invalid: None,
            },
            locators: vec![],
            classes: None,
            attributes: None,
            computed_styles: None,
        });

        let unknown = build_repair_verification(
            &finding(),
            "attempt-1",
            &evidence(1),
            &RepairEvidenceBundle {
                context: snapshot.clone(),
                ..evidence(2)
            },
            &request(None),
        )
        .expect("verification");
        assert!(!unknown.passed);

        let passed = build_repair_verification(
            &finding(),
            "attempt-1",
            &evidence(1),
            &RepairEvidenceBundle {
                context: snapshot,
                ..evidence(2)
            },
            &request(Some(true)),
        )
        .expect("verification");
        assert!(passed.passed);
    }

    #[test]
    fn rejects_non_normal_changed_file_paths() {
        let invalid = RepairVerifyRequest {
            changed_files: vec!["src/../outside.ts".to_string()],
            ..request(Some(true))
        };
        let error = validate_repair_verification_request(&invalid)
            .expect_err("parent traversal must be rejected");
        assert_eq!(error.code(), "test.session.repair_verify_invalid");
    }

    #[test]
    fn rejects_a_snapshot_without_explicit_ready_page_metadata() {
        let mut missing_page = snapshot();
        missing_page.page = None;
        let error = build_repair_verification(
            &finding(),
            "attempt-1",
            &evidence(1),
            &RepairEvidenceBundle {
                context: missing_page,
                ..evidence(2)
            },
            &request(Some(true)),
        )
        .expect_err("missing page readiness must fail closed");
        assert_eq!(error.code(), "test.session.repair_verify_not_ready");
    }
}
