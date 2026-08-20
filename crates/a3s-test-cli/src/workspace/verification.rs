use std::path::Path;
use std::time::Duration;

use a3s_test_core::{
    RepairCheckResult, RepairCheckStatus, RepairFinding, RepairVerificationScope,
    RepairVerificationSlice, MAX_REPAIR_CHECK_SUMMARY_BYTES,
};
use a3s_test_session::{plan_repair_verification_slice, RepairVerificationCheck};
use anyhow::Result;

use super::config::{self, VerificationCheckProfile, VerificationCheckTier};
use super::process::OwnedCheck;

pub(crate) struct VerificationRun {
    pub(crate) catalog: Vec<RepairVerificationCheck>,
    pub(crate) results: Vec<RepairCheckResult>,
    pub(crate) slice: RepairVerificationSlice,
}

pub(crate) async fn run_configured_checks(
    root: &Path,
    configured: &Path,
    finding: &RepairFinding,
    changed_files: &[String],
    new_console_errors: u32,
    new_page_errors: u32,
    prior_acl_proof_passed: Option<bool>,
) -> Result<VerificationRun> {
    let checks = load_optional_checks(root, configured).await?;
    let catalog = checks
        .iter()
        .map(|check| RepairVerificationCheck {
            id: check.id.clone(),
            file_prefixes: check.file_prefixes.clone(),
            regression: check.tier == VerificationCheckTier::Regression,
        })
        .collect::<Vec<_>>();
    let slice = plan_repair_verification_slice(
        finding,
        changed_files,
        new_console_errors,
        new_page_errors,
        prior_acl_proof_passed,
        &catalog,
    )
    .map_err(anyhow::Error::new)?;
    let mut results = execute_selected_checks(&checks, &slice.selected_checks).await;
    if slice.scope == RepairVerificationScope::Expanded && results.is_empty() {
        results.push(RepairCheckResult {
            command: "project.verification".to_string(),
            status: RepairCheckStatus::Failed,
            summary: "expanded verification requires at least one trusted project check"
                .to_string(),
        });
    }
    Ok(VerificationRun {
        catalog,
        results,
        slice,
    })
}

async fn load_optional_checks(
    root: &Path,
    configured: &Path,
) -> Result<Vec<VerificationCheckProfile>> {
    let path = config::resolve_config_path(root, configured).await?;
    match tokio::fs::symlink_metadata(&path).await {
        Ok(_) => Ok(config::load(root, configured).await?.verification.checks),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

async fn execute_selected_checks(
    checks: &[VerificationCheckProfile],
    selected: &[String],
) -> Vec<RepairCheckResult> {
    let mut results = Vec::with_capacity(selected.len());
    for id in selected {
        let Some(check) = checks.iter().find(|check| check.id == *id) else {
            results.push(RepairCheckResult {
                command: id.clone(),
                status: RepairCheckStatus::Failed,
                summary: "planned verification check disappeared before execution".to_string(),
            });
            continue;
        };
        let command = encoded_command(check);
        let result = match OwnedCheck::spawn(check) {
            Ok(process) => {
                process
                    .complete(
                        Duration::from_millis(check.timeout_ms),
                        Duration::from_millis(check.cleanup_timeout_ms),
                    )
                    .await
            }
            Err(error) => Err(error),
        };
        results.push(match result {
            Ok(status) if status.success() => RepairCheckResult {
                command,
                status: RepairCheckStatus::Passed,
                summary: format!("verification check '{}' passed", check.id),
            },
            Ok(status) => RepairCheckResult {
                command,
                status: RepairCheckStatus::Failed,
                summary: format!(
                    "verification check '{}' exited with {}",
                    check.id,
                    status
                        .code()
                        .map_or_else(|| "no exit code".to_string(), |code| code.to_string())
                ),
            },
            Err(error) => RepairCheckResult {
                command,
                status: RepairCheckStatus::Failed,
                summary: bounded_summary(&format!(
                    "verification check '{}' failed: {error:#}",
                    check.id
                )),
            },
        });
    }
    results
}

fn encoded_command(check: &VerificationCheckProfile) -> String {
    serde_json::to_string(
        &std::iter::once(check.executable.as_str())
            .chain(check.arguments.iter().map(String::as_str))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| format!("[\"{}\"]", check.id))
}

fn bounded_summary(value: &str) -> String {
    let mut end = value.len().min(MAX_REPAIR_CHECK_SUMMARY_BYTES);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use a3s_test_core::{
        RepairVerificationExpansionReason, RepairVerificationScope,
        REPAIR_VERIFICATION_SLICE_PROTOCOL,
    };

    use super::*;

    #[tokio::test]
    async fn executes_only_the_planned_focused_check() {
        let working_directory = std::env::current_dir().expect("current directory");
        let checks = vec![
            check(
                "focused",
                VerificationCheckTier::Focused,
                &["__a3s_verification_noop__", "--exact"],
                &working_directory,
            ),
            check(
                "regression",
                VerificationCheckTier::Regression,
                &["--definitely-invalid"],
                &working_directory,
            ),
        ];
        let results = execute_selected_checks(&checks, &["focused".to_string()]).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, RepairCheckStatus::Passed);
        assert!(results[0].command.contains("__a3s_verification_noop__"));
    }

    #[tokio::test]
    async fn records_a_selected_regression_failure_without_running_unselected_checks() {
        let working_directory = std::env::current_dir().expect("current directory");
        let checks = vec![
            check(
                "focused",
                VerificationCheckTier::Focused,
                &["__a3s_verification_noop__", "--exact"],
                &working_directory,
            ),
            check(
                "regression",
                VerificationCheckTier::Regression,
                &["--definitely-invalid"],
                &working_directory,
            ),
        ];
        let results = execute_selected_checks(&checks, &["regression".to_string()]).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, RepairCheckStatus::Failed);
        assert!(results[0].command.contains("--definitely-invalid"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timed_out_checks_are_cleaned_and_recorded_as_failures() {
        let working_directory = std::env::current_dir().expect("current directory");
        let mut timed_out = check(
            "timeout",
            VerificationCheckTier::Focused,
            &["-c", "sleep 30"],
            &working_directory,
        );
        timed_out.executable = "/bin/sh".to_string();
        timed_out.timeout_ms = 20;
        timed_out.cleanup_timeout_ms = 500;

        let results = execute_selected_checks(&[timed_out], &["timeout".to_string()]).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, RepairCheckStatus::Failed);
        assert!(results[0].summary.contains("execution timeout"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn descendant_processes_make_an_otherwise_successful_check_fail() {
        let working_directory = std::env::current_dir().expect("current directory");
        let mut descendant = check(
            "descendant",
            VerificationCheckTier::Focused,
            &["-c", "sleep 30 &"],
            &working_directory,
        );
        descendant.executable = "/bin/sh".to_string();
        descendant.cleanup_timeout_ms = 100;

        let results = execute_selected_checks(&[descendant], &["descendant".to_string()]).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, RepairCheckStatus::Failed);
        assert!(results[0].summary.contains("descendant"));
    }

    #[test]
    fn serialized_slice_contract_remains_strict_and_versioned() {
        let slice = RepairVerificationSlice {
            protocol: REPAIR_VERIFICATION_SLICE_PROTOCOL.to_string(),
            scope: RepairVerificationScope::Expanded,
            source_files: vec!["src/Checkout.tsx".to_string()],
            stable_locator: true,
            prior_acl_proof_passed: Some(false),
            selected_checks: vec!["workspace".to_string()],
            expansion_reasons: vec![RepairVerificationExpansionReason::PriorProofFailed],
        };
        let value = serde_json::to_value(slice).expect("serialized slice");
        assert_eq!(value["protocol"], REPAIR_VERIFICATION_SLICE_PROTOCOL);
        assert_eq!(value["scope"], "expanded");
        assert_eq!(value["selectedChecks"], serde_json::json!(["workspace"]));
    }

    #[test]
    fn generated_check_summaries_remain_inside_the_wire_byte_limit() {
        let bounded = bounded_summary(&"界".repeat(MAX_REPAIR_CHECK_SUMMARY_BYTES));

        assert!(bounded.len() <= MAX_REPAIR_CHECK_SUMMARY_BYTES);
        assert!(!bounded.is_empty());
    }

    #[tokio::test]
    async fn configured_runner_keeps_source_local_changes_focused_and_expands_cross_source_changes()
    {
        let project = tempfile::tempdir().expect("project");
        std::fs::create_dir(project.path().join(".a3s-test")).expect("profile directory");
        std::fs::write(
            project.path().join(".a3s-test/project.acl"),
            verification_profile(),
        )
        .expect("verification profile");
        let finding = mapped_finding();

        let focused = run_configured_checks(
            project.path(),
            Path::new(".a3s-test/project.acl"),
            &finding,
            &["src/Checkout.tsx".to_string()],
            0,
            0,
            None,
        )
        .await
        .expect("focused run");
        assert_eq!(focused.slice.scope, RepairVerificationScope::Focused);
        assert_eq!(focused.slice.selected_checks, ["component"]);
        assert_eq!(focused.results.len(), 1);
        assert_eq!(focused.results[0].status, RepairCheckStatus::Passed);

        let expanded = run_configured_checks(
            project.path(),
            Path::new(".a3s-test/project.acl"),
            &finding,
            &[
                "src/Checkout.tsx".to_string(),
                "shared/theme.css".to_string(),
            ],
            0,
            0,
            None,
        )
        .await
        .expect("expanded run");
        assert_eq!(expanded.slice.scope, RepairVerificationScope::Expanded);
        assert_eq!(expanded.slice.selected_checks, ["workspace"]);
        assert_eq!(expanded.results.len(), 1);
        assert_eq!(expanded.results[0].status, RepairCheckStatus::Failed);
    }

    #[tokio::test]
    async fn expanded_run_without_a_trusted_catalog_fails_closed() {
        let project = tempfile::tempdir().expect("project");
        std::fs::create_dir(project.path().join(".a3s-test")).expect("profile directory");

        let run = run_configured_checks(
            project.path(),
            Path::new(".a3s-test/project.acl"),
            &mapped_finding(),
            &["shared/theme.css".to_string()],
            0,
            0,
            None,
        )
        .await
        .expect("expanded run without a profile");

        assert_eq!(run.slice.scope, RepairVerificationScope::Expanded);
        assert!(run.slice.selected_checks.is_empty());
        assert_eq!(run.results.len(), 1);
        assert_eq!(run.results[0].command, "project.verification");
        assert_eq!(run.results[0].status, RepairCheckStatus::Failed);
    }

    fn check(
        id: &str,
        tier: VerificationCheckTier,
        arguments: &[&str],
        working_directory: &Path,
    ) -> VerificationCheckProfile {
        VerificationCheckProfile {
            id: id.to_string(),
            tier,
            executable: test_executable(),
            arguments: arguments.iter().map(|value| (*value).to_string()).collect(),
            working_directory: working_directory.to_path_buf(),
            file_prefixes: if tier == VerificationCheckTier::Focused {
                vec!["src".to_string()]
            } else {
                Vec::new()
            },
            timeout_ms: 30_000,
            cleanup_timeout_ms: 10_000,
        }
    }

    fn verification_profile() -> String {
        let executable =
            serde_json::to_string(&test_executable()).expect("encoded test executable");
        format!(
            r#"project "fixture" {{
  version = 1
  root = ".."

  dev_server {{
    executable = {executable}
    args = ["__a3s_verification_noop__", "--exact"]
    working_directory = "."
    url = "http://127.0.0.1:5173/"
  }}

  browser {{
    driver = "a3s"
    session = "dev"
    headed = true
  }}

  verification {{
    check "component" {{
      tier = "focused"
      executable = {executable}
      args = ["__a3s_verification_noop__", "--exact"]
      working_directory = "."
      file_prefixes = ["src"]
    }}

    check "workspace" {{
      tier = "regression"
      executable = {executable}
      args = ["--definitely-invalid"]
      working_directory = "."
      file_prefixes = []
    }}
  }}

  testkit {{
    required = true
  }}
}}
"#
        )
    }

    fn test_executable() -> String {
        // Toolchain shims can create transient descendants on Windows. The
        // current test binary exercises the same owned-process path directly.
        std::env::current_exe()
            .expect("current test executable")
            .into_os_string()
            .into_string()
            .expect("UTF-8 test executable path")
    }

    fn mapped_finding() -> RepairFinding {
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
                        "candidates": [{
                            "span": { "file": "src/Checkout.tsx", "line": 1 },
                            "confidence": 1.0,
                            "origin": "boundary_hint",
                            "relation": "exact",
                            "registrationId": "checkout"
                        }],
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
