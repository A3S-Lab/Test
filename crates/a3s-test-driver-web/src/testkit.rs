use std::collections::BTreeSet;

use a3s_test_core::DriverError;
use semver::{Version, VersionReq};
use serde::Serialize;
use serde_json::{Map, Value};

pub const TESTKIT_HANDSHAKE_PROTOCOL: &str = "a3s.test.testkit-handshake/1";
pub const TESTKIT_PACKAGE_NAME: &str = "@a3s-lab/testkit";
pub const TESTKIT_SDK_COMPATIBILITY: &str = ">=0.4.0, <0.5.0";

const PAGE_CONTEXT_PROTOCOL: &str = "a3s.test.page-context/1";
const MAX_HANDSHAKE_CAPABILITIES: usize = 64;
const MAX_HANDSHAKE_STRING_BYTES: usize = 128;
const REQUIRED_CAPABILITIES: [&str; 7] = [
    "bounded_snapshot",
    "component_boundaries",
    "design_references",
    "geometry",
    "repair_queue",
    "revision_wait",
    "scoped_inspection",
];

const TESTKIT_HANDSHAKE_FUNCTION: &str = r#"(async ({ requireReviewOverlay, timeoutMs }) => {
  const probe = () => {
    const bridge = window[Symbol.for("a3s.test.page-context")];
    if (bridge === undefined || bridge === null) return { state: "absent" };
    try {
      if (typeof bridge !== "object" && typeof bridge !== "function") {
        return { state: "bridge_invalid" };
      }
      if (typeof bridge.handshake !== "function") {
        return { state: "handshake_missing" };
      }
      const handshake = bridge.handshake();
      const reviewOverlayMounted = Array.from(
        document.querySelectorAll("[data-a3s-testkit-overlay]"),
      ).some((host) => Boolean(host.shadowRoot));
      return {
        state: "present",
        handshake,
        reviewOverlayMounted,
      };
    } catch {
      return { state: "handshake_failed" };
    }
  };
  const deadline = performance.now() + timeoutMs;
  while (true) {
    const result = probe();
    const waitingForBridge = result.state === "absent";
    const waitingForOverlay = result.state === "present" && !result.reviewOverlayMounted;
    if (!requireReviewOverlay || (!waitingForBridge && !waitingForOverlay)) return result;
    if (performance.now() >= deadline) return result;
    await new Promise((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve();
      };
      const timer = setTimeout(finish, 50);
      requestAnimationFrame(finish);
    });
  }
})"#;

pub(crate) fn testkit_handshake_script(require_review_overlay: bool, timeout_ms: u64) -> String {
    format!(
        "{TESTKIT_HANDSHAKE_FUNCTION}({{requireReviewOverlay:{require_review_overlay},timeoutMs:{timeout_ms}}})"
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TestKitHandshake {
    pub protocol: String,
    pub package_name: String,
    pub sdk_version: String,
    pub page_context_protocol: String,
    pub capabilities: Vec<String>,
    pub review_overlay_mounted: bool,
}

pub(crate) fn parse_testkit_handshake(
    value: Value,
    require_review_overlay: bool,
) -> Result<Option<TestKitHandshake>, DriverError> {
    let object = value.as_object().ok_or_else(|| {
        invalid_handshake("live Test Kit handshake probe did not return an object")
    })?;
    let state = bounded_string(object, "state", 32)?;
    match state {
        "absent" => {
            exact_fields(object, &["state"])?;
            Ok(None)
        }
        "bridge_invalid" => {
            exact_fields(object, &["state"])?;
            Err(DriverError::new(
                "test.driver.web.testkit_bridge_invalid",
                "the page context symbol is present but is not a valid Test Kit bridge; update @a3s-lab/testkit and mount <A3STestKit>",
            ))
        }
        "handshake_missing" => {
            exact_fields(object, &["state"])?;
            Err(DriverError::new(
                "test.driver.web.testkit_handshake_missing",
                "the mounted Test Kit does not expose the live handshake protocol; update @a3s-lab/testkit within >=0.4.0, <0.5.0",
            ))
        }
        "handshake_failed" => {
            exact_fields(object, &["state"])?;
            Err(DriverError::new(
                "test.driver.web.testkit_handshake_failed",
                "the mounted Test Kit handshake threw an error",
            ))
        }
        "present" => parse_present(object, require_review_overlay).map(Some),
        _ => Err(invalid_handshake(
            "live Test Kit handshake probe returned an unsupported state",
        )),
    }
}

fn parse_present(
    object: &Map<String, Value>,
    require_review_overlay: bool,
) -> Result<TestKitHandshake, DriverError> {
    exact_fields(object, &["state", "handshake", "reviewOverlayMounted"])?;
    let handshake = object
        .get("handshake")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_handshake("Test Kit handshake payload is not an object"))?;
    exact_fields(
        handshake,
        &[
            "protocol",
            "packageName",
            "sdkVersion",
            "pageContextProtocol",
            "capabilities",
        ],
    )?;

    let protocol = bounded_string(handshake, "protocol", MAX_HANDSHAKE_STRING_BYTES)?;
    if protocol != TESTKIT_HANDSHAKE_PROTOCOL {
        return Err(DriverError::new(
            "test.driver.web.testkit_handshake_protocol_unsupported",
            format!(
                "Test Kit handshake protocol is unsupported; expected {TESTKIT_HANDSHAKE_PROTOCOL}"
            ),
        ));
    }

    let package_name = bounded_string(handshake, "packageName", MAX_HANDSHAKE_STRING_BYTES)?;
    if package_name != TESTKIT_PACKAGE_NAME {
        return Err(DriverError::new(
            "test.driver.web.testkit_package_unsupported",
            format!("Test Kit handshake reported an unsupported package; expected {TESTKIT_PACKAGE_NAME}"),
        ));
    }

    let sdk_version = bounded_string(handshake, "sdkVersion", 64)?;
    let parsed_version = Version::parse(sdk_version).map_err(|_| {
        DriverError::new(
            "test.driver.web.testkit_sdk_version_invalid",
            "Test Kit handshake reported an invalid SDK version",
        )
    })?;
    let compatibility = VersionReq::parse(TESTKIT_SDK_COMPATIBILITY).map_err(|error| {
        DriverError::new(
            "test.driver.web.testkit_compatibility_invalid",
            format!("Web adapter Test Kit compatibility metadata is invalid: {error}"),
        )
    })?;
    if !compatibility.matches(&parsed_version) {
        return Err(DriverError::new(
            "test.driver.web.testkit_sdk_version_unsupported",
            format!(
                "Test Kit SDK {parsed_version} is outside the supported range {TESTKIT_SDK_COMPATIBILITY}"
            ),
        ));
    }

    let page_context_protocol =
        bounded_string(handshake, "pageContextProtocol", MAX_HANDSHAKE_STRING_BYTES)?;
    if page_context_protocol != PAGE_CONTEXT_PROTOCOL {
        return Err(DriverError::new(
            "test.driver.web.testkit_page_context_protocol_unsupported",
            format!("Test Kit uses an unsupported Page Context protocol; expected {PAGE_CONTEXT_PROTOCOL}"),
        ));
    }

    let capabilities = parse_capabilities(handshake.get("capabilities"))?;
    let available = capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let missing = REQUIRED_CAPABILITIES
        .iter()
        .copied()
        .filter(|capability| !available.contains(capability))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(DriverError::new(
            "test.driver.web.testkit_capability_missing",
            format!(
                "Test Kit handshake is missing required capabilities: {}; update @a3s-lab/testkit within {TESTKIT_SDK_COMPATIBILITY}",
                missing.join(", ")
            ),
        ));
    }

    let review_overlay_mounted = object
        .get("reviewOverlayMounted")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            invalid_handshake("live Test Kit handshake omitted the Review Overlay mount state")
        })?;
    if require_review_overlay && !review_overlay_mounted {
        return Err(DriverError::new(
            "test.driver.web.testkit_review_overlay_missing",
            "the Test Kit handshake is compatible, but the Review Overlay is not mounted; render <A3SReviewOverlay /> inside <A3STestKit>",
        ));
    }

    Ok(TestKitHandshake {
        protocol: protocol.to_string(),
        package_name: package_name.to_string(),
        sdk_version: parsed_version.to_string(),
        page_context_protocol: page_context_protocol.to_string(),
        capabilities,
        review_overlay_mounted,
    })
}

fn parse_capabilities(value: Option<&Value>) -> Result<Vec<String>, DriverError> {
    let values = value.and_then(Value::as_array).ok_or_else(|| {
        DriverError::new(
            "test.driver.web.testkit_capabilities_invalid",
            "Test Kit handshake capabilities must be an array",
        )
    })?;
    if values.is_empty() || values.len() > MAX_HANDSHAKE_CAPABILITIES {
        return Err(DriverError::new(
            "test.driver.web.testkit_capabilities_invalid",
            format!(
                "Test Kit handshake must report between 1 and {MAX_HANDSHAKE_CAPABILITIES} capabilities"
            ),
        ));
    }
    let mut capabilities = Vec::with_capacity(values.len());
    for value in values {
        let Some(capability) = value.as_str() else {
            return Err(DriverError::new(
                "test.driver.web.testkit_capabilities_invalid",
                "Test Kit handshake capabilities must contain only strings",
            ));
        };
        if capability.is_empty()
            || capability.len() > 64
            || !capability
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(DriverError::new(
                "test.driver.web.testkit_capabilities_invalid",
                "Test Kit handshake contains an invalid capability",
            ));
        }
        capabilities.push(capability.to_string());
    }
    if capabilities.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DriverError::new(
            "test.driver.web.testkit_capabilities_invalid",
            "Test Kit handshake capabilities must be sorted and unique",
        ));
    }
    Ok(capabilities)
}

fn bounded_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    max_bytes: usize,
) -> Result<&'a str, DriverError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= max_bytes)
        .ok_or_else(|| {
            invalid_handshake(format!(
                "live Test Kit handshake field '{field}' must be a bounded non-empty string"
            ))
        })
}

fn exact_fields(object: &Map<String, Value>, expected: &[&str]) -> Result<(), DriverError> {
    if object.len() == expected.len() && expected.iter().all(|field| object.contains_key(*field)) {
        return Ok(());
    }
    Err(invalid_handshake(
        "live Test Kit handshake contains missing or unsupported fields",
    ))
}

fn invalid_handshake(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.web.testkit_handshake_invalid", message)
}
