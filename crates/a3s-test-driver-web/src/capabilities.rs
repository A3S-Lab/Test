use std::collections::BTreeSet;
use std::ffi::OsString;

use a3s_test_core::{DriverError, ACTION_PROTOCOL_REVISION};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{AgentBrowserConfig, BrowserCommand, CommandExecutor, CommandInvocation};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserIntegration {
    A3s,
    Standalone,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebCapability {
    Accessibility,
    Console,
    ContextClicks,
    Dialogs,
    DomainContainment,
    Downloads,
    DragAndDrop,
    ElementInteractions,
    FormControls,
    Frames,
    Har,
    MouseWheel,
    NetworkRoutes,
    PageErrors,
    Screenshots,
    Tabs,
    Trace,
    Uploads,
    Video,
    Viewport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BrowserCapabilities {
    pub integration: BrowserIntegration,
    pub version: String,
    pub protocol_revision: u32,
    pub features: BTreeSet<WebCapability>,
}

pub(crate) async fn discover(
    config: &AgentBrowserConfig,
    executor: &dyn CommandExecutor,
) -> Result<BrowserCapabilities, DriverError> {
    let mut args = config.command.prefix();
    args.push(OsString::from("--version"));
    let output = executor
        .run(CommandInvocation {
            program: config.command.program().to_path_buf(),
            args,
            env: Default::default(),
            timeout: config.command_timeout,
        })
        .await
        .map_err(|error| {
            let retryable = error.retryable();
            DriverError::new(
                "test.driver.web.capability_unavailable",
                format!("failed to query browser capabilities: {error}"),
            )
            .with_retryable(retryable)
        })?;
    if output.exit_code != 0 {
        let detail = if output.stderr.trim().is_empty() {
            output.stdout.trim()
        } else {
            output.stderr.trim()
        };
        return Err(DriverError::new(
            "test.driver.web.capability_unavailable",
            format!("browser version probe failed: {detail}"),
        ));
    }

    let version = parse_version(&output.stdout)?;
    let integration = match &config.command {
        BrowserCommand::A3s { .. } => BrowserIntegration::A3s,
        BrowserCommand::Standalone { .. } => BrowserIntegration::Standalone,
    };
    admit_version(integration, &version)?;

    Ok(BrowserCapabilities {
        integration,
        version: version.to_string(),
        protocol_revision: ACTION_PROTOCOL_REVISION,
        features: [
            WebCapability::Accessibility,
            WebCapability::Console,
            WebCapability::ContextClicks,
            WebCapability::Dialogs,
            WebCapability::DomainContainment,
            WebCapability::Downloads,
            WebCapability::DragAndDrop,
            WebCapability::ElementInteractions,
            WebCapability::FormControls,
            WebCapability::Frames,
            WebCapability::Har,
            WebCapability::MouseWheel,
            WebCapability::NetworkRoutes,
            WebCapability::PageErrors,
            WebCapability::Screenshots,
            WebCapability::Tabs,
            WebCapability::Trace,
            WebCapability::Uploads,
            WebCapability::Video,
            WebCapability::Viewport,
        ]
        .into_iter()
        .collect(),
    })
}

fn parse_version(output: &str) -> Result<Version, DriverError> {
    output
        .split_whitespace()
        .rev()
        .find_map(|candidate| Version::parse(candidate).ok())
        .ok_or_else(|| {
            DriverError::new(
                "test.driver.web.version_invalid",
                "browser version output did not contain a semantic version",
            )
        })
}

fn admit_version(integration: BrowserIntegration, version: &Version) -> Result<(), DriverError> {
    let (minimum, maximum) = match integration {
        BrowserIntegration::A3s => (Version::new(0, 1, 1), Version::new(0, 2, 0)),
        BrowserIntegration::Standalone => (Version::new(0, 26, 0), Version::new(0, 27, 0)),
    };
    if version < &minimum || version >= &maximum {
        return Err(DriverError::new(
            "test.driver.web.version_unsupported",
            format!(
                "{integration:?} browser version {version} is unsupported; expected >= {minimum}, < {maximum}"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{admit_version, parse_version, BrowserIntegration};
    use semver::Version;

    #[test]
    fn parses_plain_cli_version_output() {
        assert_eq!(
            parse_version("agent-browser 0.26.0").expect("version"),
            Version::new(0, 26, 0)
        );
    }

    #[test]
    fn rejects_versions_outside_the_verified_protocol_window() {
        let error = admit_version(BrowserIntegration::Standalone, &Version::new(0, 27, 0))
            .expect_err("unverified minor");
        assert_eq!(error.code(), "test.driver.web.version_unsupported");
    }
}
