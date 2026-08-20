use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use semver::{Version, VersionReq};
use serde::Serialize;

use super::config::{self, ProjectBrowserDriver, ProjectProfile};
use super::discovery::{read_package, testkit_install_command, TESTKIT_PACKAGE};
use super::DoctorArgs;

const TESTKIT_COMPATIBILITY: &str = ">=0.4.0, <0.5.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckStatus {
    Passed,
    Warning,
    Failed,
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    id: &'static str,
    status: CheckStatus,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DoctorStatus {
    Passed,
    Warning,
    Failed,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    protocol: &'static str,
    status: DoctorStatus,
    cli_version: &'static str,
    project: DoctorProject,
    checks: Vec<DoctorCheck>,
    passed: usize,
    warnings: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
struct DoctorProject {
    id: String,
    root: PathBuf,
    config_path: PathBuf,
    url: String,
}

pub(super) async fn execute(args: DoctorArgs) -> Result<ExitCode> {
    let profile = config::load(&args.root, &args.config).await?;
    let report = diagnose(&profile, args.connect).await;
    let failed = report.failed > 0 || (args.strict && report.warnings > 0);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for check in &report.checks {
            let marker = match check.status {
                CheckStatus::Passed => "PASS",
                CheckStatus::Warning => "WARN",
                CheckStatus::Failed => "FAIL",
            };
            println!("[{marker}] {}: {}", check.id, check.summary);
            if let Some(fix) = &check.fix {
                println!("       Fix: {fix}");
            }
        }
        println!(
            "Doctor: {} passed, {} warning(s), {} failed",
            report.passed, report.warnings, report.failed
        );
    }
    Ok(if failed {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}

pub(super) async fn dev_preflight(profile: &ProjectProfile) -> Result<()> {
    let report = diagnose(profile, false).await;
    if report.failed == 0 {
        return Ok(());
    }
    let failures = report
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::Failed)
        .map(|check| match &check.fix {
            Some(fix) => format!("{}: {} (fix: {fix})", check.id, check.summary),
            None => format!("{}: {}", check.id, check.summary),
        })
        .collect::<Vec<_>>()
        .join("; ");
    anyhow::bail!("development preflight failed: {failures}")
}

async fn diagnose(profile: &ProjectProfile, connect: bool) -> DoctorReport {
    let mut checks = vec![passed(
        "profile",
        format!(
            "project profile version {} is valid",
            config::PROJECT_PROFILE_VERSION
        ),
    )];
    let package = match read_package(&profile.root).await {
        Ok(package) => {
            checks.push(passed("package.metadata", "package.json is valid"));
            Some(package)
        }
        Err(error) => {
            checks.push(failed(
                "package.metadata",
                format!("package.json is unavailable: {error:#}"),
                None,
            ));
            None
        }
    };
    check_dev_script(profile, package.as_ref(), &mut checks);
    checks.push(executable_check(
        "dev.executable",
        &profile.dev_server.executable,
        &profile.dev_server.working_directory,
        "install the detected package manager or update project.dev_server.executable",
    ));
    let browser_executable = profile
        .browser
        .executable
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| match profile.browser.driver {
            ProjectBrowserDriver::A3s => "a3s".to_string(),
            ProjectBrowserDriver::Standalone => "agent-browser".to_string(),
        });
    checks.push(executable_check(
        "browser.executable",
        &browser_executable,
        &profile.root,
        "install the configured browser adapter or set project.browser.executable",
    ));
    check_testkit(profile, package.as_ref(), &mut checks).await;
    if connect {
        checks.push(check_url(profile).await);
    } else {
        checks.push(warning(
            "dev.url",
            format!(
                "{} was not probed; pass --connect when the development server is running",
                profile.dev_server.url
            ),
            Some("a3s-test doctor --connect".to_string()),
        ));
    }
    let passed = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Passed)
        .count();
    let warnings = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Warning)
        .count();
    let failed = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Failed)
        .count();
    let status = if failed > 0 {
        DoctorStatus::Failed
    } else if warnings > 0 {
        DoctorStatus::Warning
    } else {
        DoctorStatus::Passed
    };
    DoctorReport {
        protocol: "a3s.test.project-doctor/1",
        status,
        cli_version: env!("CARGO_PKG_VERSION"),
        project: DoctorProject {
            id: profile.id.clone(),
            root: profile.root.clone(),
            config_path: profile.config_path.clone(),
            url: profile.dev_server.url.to_string(),
        },
        checks,
        passed,
        warnings,
        failed,
    }
}

fn check_dev_script(
    profile: &ProjectProfile,
    package: Option<&serde_json::Value>,
    checks: &mut Vec<DoctorCheck>,
) {
    let Some(package) = package else {
        return;
    };
    let script = profile
        .dev_server
        .arguments
        .as_slice()
        .strip_prefix(&["run".to_string()])
        .and_then(|values| values.first());
    let Some(script) = script else {
        checks.push(warning(
            "dev.script",
            "the development command is custom and cannot be matched to package.json",
            None,
        ));
        return;
    };
    if package
        .get("scripts")
        .and_then(|scripts| scripts.get(script))
        .and_then(serde_json::Value::as_str)
        .is_some()
    {
        checks.push(passed(
            "dev.script",
            format!("package.json script '{script}' exists"),
        ));
    } else {
        checks.push(failed(
            "dev.script",
            format!("package.json no longer defines script '{script}'"),
            Some(format!(
                "update project.dev_server.args or restore the '{script}' script"
            )),
        ));
    }
}

async fn check_testkit(
    profile: &ProjectProfile,
    package: Option<&serde_json::Value>,
    checks: &mut Vec<DoctorCheck>,
) {
    let declared = package.is_some_and(|package| {
        package
            .get("dependencies")
            .and_then(|dependencies| dependencies.get(TESTKIT_PACKAGE))
            .is_some()
            || package
                .get("devDependencies")
                .and_then(|dependencies| dependencies.get(TESTKIT_PACKAGE))
                .is_some()
    });
    let install = testkit_install_command(&profile.dev_server.executable);
    if declared {
        checks.push(passed(
            "testkit.dependency",
            format!("{TESTKIT_PACKAGE} is declared"),
        ));
    } else if profile.testkit.required {
        checks.push(failed(
            "testkit.dependency",
            format!("{TESTKIT_PACKAGE} is required but not declared"),
            Some(install.clone()),
        ));
    } else {
        checks.push(warning(
            "testkit.dependency",
            format!("{TESTKIT_PACKAGE} is not declared; page review context is optional"),
            Some(install.clone()),
        ));
    }
    if !declared {
        return;
    }
    let installed_path = profile
        .root
        .join("node_modules")
        .join("@a3s-lab")
        .join("testkit")
        .join("package.json");
    match read_installed_version(&installed_path).await {
        Ok(version) => {
            let compatibility = match VersionReq::parse(TESTKIT_COMPATIBILITY) {
                Ok(compatibility) => compatibility,
                Err(error) => {
                    checks.push(failed(
                        "testkit.compatibility",
                        format!("CLI Test Kit compatibility metadata is invalid: {error}"),
                        None,
                    ));
                    return;
                }
            };
            let compatible = compatibility.matches(&version);
            if compatible {
                checks.push(passed(
                    "testkit.installed",
                    format!(
                        "{TESTKIT_PACKAGE} {version} passes the static {TESTKIT_COMPATIBILITY} package range; a3s-test dev verifies the live protocol handshake"
                    ),
                ));
            } else {
                checks.push(failed(
                    "testkit.installed",
                    format!("{TESTKIT_PACKAGE} {version} is outside {TESTKIT_COMPATIBILITY}"),
                    Some(install),
                ));
            }
        }
        Err(error) => {
            let check = if profile.testkit.required {
                failed(
                    "testkit.installed",
                    format!("declared Test Kit is not installed: {error:#}"),
                    Some(install),
                )
            } else {
                warning(
                    "testkit.installed",
                    format!("declared Test Kit is not installed: {error:#}"),
                    Some(install),
                )
            };
            checks.push(check);
        }
    }
}

async fn read_installed_version(path: &Path) -> Result<Version> {
    let metadata = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        anyhow::bail!("installed package metadata is not a bounded regular file");
    }
    let bytes = tokio::fs::read(path).await?;
    let package: serde_json::Value = serde_json::from_slice(&bytes)?;
    let version = package
        .get("version")
        .and_then(serde_json::Value::as_str)
        .context("installed package has no version")?;
    Version::parse(version).context("installed Test Kit version is invalid")
}

async fn check_url(profile: &ProjectProfile) -> DoctorCheck {
    let client = match reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return failed(
                "dev.url",
                format!("failed to prepare URL probe: {error}"),
                None,
            );
        }
    };
    match client.get(profile.dev_server.url.clone()).send().await {
        Ok(response) => passed(
            "dev.url",
            format!(
                "{} responded with HTTP {}",
                profile.dev_server.url,
                response.status().as_u16()
            ),
        ),
        Err(error) => failed(
            "dev.url",
            format!("{} is unreachable: {error}", profile.dev_server.url),
            Some("start the development server or run `a3s-test dev`".to_string()),
        ),
    }
}

fn executable_check(
    id: &'static str,
    executable: &str,
    working_directory: &Path,
    fix: &str,
) -> DoctorCheck {
    match find_executable(executable, working_directory) {
        Some(path) => passed(id, format!("executable resolved to {}", path.display())),
        None => failed(
            id,
            format!("executable '{executable}' was not found"),
            Some(fix.to_string()),
        ),
    }
}

pub(super) fn find_executable(executable: &str, working_directory: &Path) -> Option<PathBuf> {
    let path = Path::new(executable);
    if path.components().count() > 1 || path.is_absolute() {
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            working_directory.join(path)
        };
        return executable_file(&candidate).then_some(candidate);
    }
    let search_path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&search_path) {
        let candidate = directory.join(executable);
        if executable_file(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        for extension in ["exe", "cmd", "bat", "com"] {
            let candidate = directory.join(format!("{executable}.{extension}"));
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn passed(id: &'static str, summary: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id,
        status: CheckStatus::Passed,
        summary: summary.into(),
        fix: None,
    }
}

fn warning(id: &'static str, summary: impl Into<String>, fix: Option<String>) -> DoctorCheck {
    DoctorCheck {
        id,
        status: CheckStatus::Warning,
        summary: summary.into(),
        fix,
    }
}

fn failed(id: &'static str, summary: impl Into<String>, fix: Option<String>) -> DoctorCheck {
    DoctorCheck {
        id,
        status: CheckStatus::Failed,
        summary: summary.into(),
        fix,
    }
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
