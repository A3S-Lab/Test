use std::collections::BTreeSet;
use std::ffi::OsString;

use a3s_test_core::{DriverError, ACTION_PROTOCOL_REVISION, PAGE_CONTEXT_PROTOCOL};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{AgentBrowserConfig, BrowserCommand, CommandExecutor, CommandInvocation};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserIntegration {
    A3s,
    Standalone,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum WebCapability {
    Accessibility,
    Console,
    ContextClicks,
    Dialogs,
    DomainContainment,
    ExactOriginContainment,
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

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserCapabilities {
    pub integration: BrowserIntegration,
    #[schemars(length(min = 1, max = 128))]
    pub version: String,
    #[schemars(schema_with = "action_protocol_revision_schema")]
    pub protocol_revision: u32,
    pub features: BTreeSet<WebCapability>,
    /// Runtime page capability discovered independently after navigation.
    /// `None` means no page was probed by this executable-only command.
    #[schemars(schema_with = "page_context_protocol_schema")]
    pub page_context_protocol: Option<String>,
}

impl BrowserCapabilities {
    pub fn validate(&self) -> Result<(), DriverError> {
        if self.version.is_empty() || self.version.len() > 128 {
            return Err(DriverError::new(
                "test.driver.web.version_invalid",
                "browser capability version must be bounded and non-empty",
            ));
        }
        let version = Version::parse(&self.version).map_err(|_| {
            DriverError::new(
                "test.driver.web.version_invalid",
                "browser capability version is not semantic",
            )
        })?;
        admit_version(self.integration, &version)?;
        if self.protocol_revision != ACTION_PROTOCOL_REVISION {
            return Err(DriverError::new(
                "test.driver.web.protocol_unsupported",
                format!(
                    "browser action protocol {} is unsupported; expected {}",
                    self.protocol_revision, ACTION_PROTOCOL_REVISION
                ),
            ));
        }
        if self.features != expected_features(self.integration) {
            return Err(DriverError::new(
                "test.driver.web.capability_invalid",
                "browser feature projection does not match the admitted integration",
            ));
        }
        if self
            .page_context_protocol
            .as_deref()
            .is_some_and(|protocol| protocol != PAGE_CONTEXT_PROTOCOL)
        {
            return Err(DriverError::new(
                "test.driver.web.page_context_protocol_unsupported",
                "browser capability declared an unsupported page-context protocol",
            ));
        }
        Ok(())
    }
}

fn action_protocol_revision_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "integer",
        "const": ACTION_PROTOCOL_REVISION
    })
}

fn page_context_protocol_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "oneOf": [
            {
                "type": "string",
                "const": PAGE_CONTEXT_PROTOCOL
            },
            {
                "type": "null"
            }
        ]
    })
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

    let capabilities = BrowserCapabilities {
        integration,
        version: version.to_string(),
        protocol_revision: ACTION_PROTOCOL_REVISION,
        features: expected_features(integration),
        page_context_protocol: None,
    };
    capabilities.validate()?;
    Ok(capabilities)
}

fn expected_features(integration: BrowserIntegration) -> BTreeSet<WebCapability> {
    let mut features: BTreeSet<_> = [
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
    .collect();
    if integration == BrowserIntegration::A3s {
        features.insert(WebCapability::ExactOriginContainment);
    }
    features
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
        BrowserIntegration::A3s => (Version::new(0, 4, 0), Version::new(0, 5, 0)),
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
    use super::{
        admit_version, expected_features, parse_version, BrowserCapabilities, BrowserIntegration,
        WebCapability,
    };
    use a3s_test_core::ACTION_PROTOCOL_REVISION;
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

    #[test]
    fn admits_the_exact_origin_a3s_browser_window() {
        admit_version(BrowserIntegration::A3s, &Version::new(0, 4, 0))
            .expect("minimum exact-origin Browser version");
        let error = admit_version(BrowserIntegration::A3s, &Version::new(0, 3, 2))
            .expect_err("hostname-only Browser version");
        assert_eq!(error.code(), "test.driver.web.version_unsupported");
    }

    #[test]
    fn capability_wire_shape_is_strict_and_locally_admitted() {
        let capabilities = BrowserCapabilities {
            integration: BrowserIntegration::Standalone,
            version: "0.26.0".to_string(),
            protocol_revision: ACTION_PROTOCOL_REVISION,
            features: expected_features(BrowserIntegration::Standalone),
            page_context_protocol: None,
        };
        capabilities.validate().expect("valid capabilities");

        let schema = serde_json::to_value(schemars::schema_for!(BrowserCapabilities))
            .expect("browser capability schema");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["protocol_revision"]["const"],
            ACTION_PROTOCOL_REVISION
        );

        let mut value = serde_json::to_value(capabilities).expect("browser capability JSON");
        value
            .as_object_mut()
            .expect("capability object")
            .insert("trusted".to_string(), serde_json::Value::Bool(true));
        serde_json::from_value::<BrowserCapabilities>(value).expect_err("unknown field must fail");
    }

    #[test]
    fn capability_admission_rejects_features_not_proven_by_the_integration() {
        let mut features = expected_features(BrowserIntegration::Standalone);
        features.insert(WebCapability::ExactOriginContainment);
        let capabilities = BrowserCapabilities {
            integration: BrowserIntegration::Standalone,
            version: "0.26.0".to_string(),
            protocol_revision: ACTION_PROTOCOL_REVISION,
            features,
            page_context_protocol: None,
        };

        let error = capabilities
            .validate()
            .expect_err("standalone exact-origin overclaim");
        assert_eq!(error.code(), "test.driver.web.capability_invalid");
    }
}
