//! Site-scoped preview headers, all-or-nothing cookies, and inert observation.
//!
//! Preview headers are attached only to `open` of the admitted origin. The
//! global `set headers` command is not used. A basic-auth password is read
//! from the named environment variable at execution and is redacted from
//! step data and driver errors. Cookie batches are admitted before any
//! `cookies set` command; a later command failure expires the names already
//! applied.

use std::collections::BTreeSet;
use std::ffi::OsString;

use a3s_test_core::{redact_secret_text, CookieAssignment, DriverError, PreviewHeader, StepOutput};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use url::Url;

use super::driver::admit_returned_page;
use super::AgentBrowserSession;

const INERT_NAMES_SCRIPT: &str = r#"(() => {
  const norm = (value) => String(value ?? "").replace(/\s+/g, " ").trim();
  const inside = new Set();
  const outside = new Set();
  const visit = (element) => {
    if (!(element instanceof Element)) return;
    const bucket = element.closest("[inert]") ? inside : outside;
    const label = norm(element.getAttribute("aria-label"));
    if (label) bucket.add(label);
    if (element.childElementCount === 0) {
      const text = norm(element.innerText || element.textContent || "");
      if (text) bucket.add(text);
    }
    for (const child of element.children) visit(child);
  };
  if (document.documentElement) visit(document.documentElement);
  return { texts: [...inside].filter((name) => name && !outside.has(name)) };
})()"#;

impl AgentBrowserSession {
    pub(super) async fn open_url(&self, url: &str) -> Result<StepOutput, DriverError> {
        let mut args = vec![OsString::from("open"), OsString::from(url)];
        if let Some(headers) = self.headers_for_navigation(url) {
            args.push(OsString::from("--headers"));
            args.push(OsString::from(headers));
        }
        match self.execute_command(args).await {
            Ok(mut data) => {
                self.scrub_preview_secret(&mut data);
                Ok(StepOutput::new("page opened").with_data(data))
            }
            Err(error) => Err(self.scrub_preview_error(error)),
        }
    }

    pub(super) async fn set_cookies(
        &self,
        origin: &str,
        cookies: &[CookieAssignment],
    ) -> Result<StepOutput, DriverError> {
        let origin = admit_cookie_origin(&self.config.network_policy, origin)?;
        admit_cookie_batch(cookies)?;
        let url = format!("{origin}/");
        let mut applied = Vec::new();
        for cookie in cookies {
            let args = vec![
                OsString::from("cookies"),
                OsString::from("set"),
                OsString::from(&cookie.name),
                OsString::from(&cookie.value),
                OsString::from("--url"),
                OsString::from(&url),
            ];
            if let Err(error) = self.execute_command(args).await {
                let rollback = self.rollback_cookies(&url, &applied).await;
                let error = scrub_cookie_values(error, cookies);
                return Err(match rollback {
                    Ok(()) => error,
                    Err(_) => DriverError::new(
                        error.code(),
                        format!("{}; cookie rollback failed", error.message()),
                    ),
                });
            }
            applied.push(cookie.name.clone());
        }
        Ok(StepOutput::new("cookies set").with_data(json!({
            "origin": origin,
            "names": applied,
            "count": applied.len(),
        })))
    }

    pub(super) async fn install_preview_headers(
        &mut self,
        origin: &str,
        headers: &[PreviewHeader],
        username: Option<&str>,
        password_env: Option<&str>,
    ) -> Result<StepOutput, DriverError> {
        let origin =
            admit_cookie_origin(&self.config.network_policy, origin).map_err(retitle_preview)?;
        let (json, secret, authorization) = preview_header_json(headers, username, password_env)?;
        self.preview_origin = Some(origin.clone());
        self.preview_headers = Some(json);
        self.preview_secret = secret;
        self.preview_authorization = authorization;
        let mut names: Vec<&str> = headers.iter().map(|header| header.name.as_str()).collect();
        if username.is_some() {
            names.push("Authorization");
        }
        Ok(
            StepOutput::new("preview headers stored for the target site").with_data(json!({
                "origin": origin,
                "headers": names,
                "password_env": password_env,
            })),
        )
    }

    pub(super) async fn omit_inert_observation(&self, mut data: Value) -> Value {
        let Ok(value) = self
            .execute_command(vec![
                OsString::from("eval"),
                OsString::from(INERT_NAMES_SCRIPT),
            ])
            .await
        else {
            return data;
        };
        let names = inert_names(&value);
        if names.is_empty() {
            return data;
        }
        omit_inert_value(&mut data, &names);
        data
    }

    fn headers_for_navigation(&self, url: &str) -> Option<&str> {
        let headers = self.preview_headers.as_deref()?;
        let origin = self.preview_origin.as_deref()?;
        let parsed = Url::parse(url).ok()?;
        if parsed.origin().ascii_serialization() == origin {
            Some(headers)
        } else {
            None
        }
    }

    fn scrub_preview_secret(&self, value: &mut Value) {
        if let Some(secret) = self.preview_secret.as_deref() {
            scrub_value(value, secret);
        }
        if let Some(authorization) = self.preview_authorization.as_deref() {
            scrub_value(value, authorization);
            if let Some(encoded) = authorization.strip_prefix("Basic ") {
                scrub_value(value, encoded);
            }
        }
    }

    fn scrub_preview_error(&self, error: DriverError) -> DriverError {
        let mut message = error.message().to_string();
        if let Some(secret) = self.preview_secret.as_deref() {
            message = redact_secret_text(&message, secret);
            message = redact_secret_text(&message, &STANDARD.encode(secret));
        }
        if let Some(authorization) = self.preview_authorization.as_deref() {
            message = redact_secret_text(&message, authorization);
            if let Some(encoded) = authorization.strip_prefix("Basic ") {
                message = redact_secret_text(&message, encoded);
            }
        }
        DriverError::new(error.code(), message).with_retryable(error.retryable())
    }

    async fn rollback_cookies(&self, url: &str, names: &[String]) -> Result<(), DriverError> {
        for name in names.iter().rev() {
            self.execute_command(vec![
                OsString::from("cookies"),
                OsString::from("set"),
                OsString::from(name),
                OsString::from("expired"),
                OsString::from("--url"),
                OsString::from(url),
                OsString::from("--expires"),
                OsString::from("1"),
            ])
            .await?;
        }
        Ok(())
    }
}

fn admit_cookie_origin(
    policy: &crate::BrowserNetworkPolicy,
    origin: &str,
) -> Result<String, DriverError> {
    let parsed = Url::parse(origin).map_err(|_| {
        DriverError::new(
            "test.driver.web.cookie_origin_invalid",
            "cookie batch was rejected before any cookie command",
        )
    })?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(DriverError::new(
            "test.driver.web.cookie_origin_invalid",
            "cookie batch was rejected before any cookie command",
        ));
    }
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(DriverError::new(
            "test.driver.web.cookie_origin_invalid",
            "cookie batch was rejected before any cookie command",
        ));
    }
    if parsed.query().is_some()
        || parsed.fragment().is_some()
        || !(parsed.path().is_empty() || parsed.path() == "/")
    {
        return Err(DriverError::new(
            "test.driver.web.cookie_origin_invalid",
            "cookie batch was rejected before any cookie command",
        ));
    }
    let canonical = parsed.origin().ascii_serialization();
    admit_returned_page(policy, &format!("{canonical}/"), "set_cookie")?;
    Ok(canonical)
}

fn admit_cookie_batch(cookies: &[CookieAssignment]) -> Result<(), DriverError> {
    if cookies.is_empty() || cookies.len() > 16 {
        return Err(cookie_rejected());
    }
    let mut seen = BTreeSet::new();
    for cookie in cookies {
        if !cookie_name_ok(&cookie.name)
            || !cookie_value_ok(&cookie.value)
            || !seen.insert(cookie.name.as_str())
        {
            return Err(cookie_rejected());
        }
    }
    Ok(())
}

fn cookie_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-'))
}

fn cookie_value_ok(value: &str) -> bool {
    value.len() <= 4_096
        && value
            .bytes()
            .all(|byte| byte >= 0x20 && byte != 0x7f && byte != b';')
}

fn cookie_rejected() -> DriverError {
    DriverError::new(
        "test.driver.web.cookie_invalid",
        "cookie batch was rejected before any cookie command",
    )
}

fn preview_header_json(
    headers: &[PreviewHeader],
    username: Option<&str>,
    password_env: Option<&str>,
) -> Result<(String, Option<String>, Option<String>), DriverError> {
    if headers.len() > 16 {
        return Err(preview_invalid("preview header batch is too large"));
    }
    let mut map = serde_json::Map::new();
    for header in headers {
        if !header_name_ok(&header.name) || !header_value_ok(&header.value) {
            return Err(preview_invalid(
                "preview header was rejected before any browser command",
            ));
        }
        if map
            .insert(header.name.clone(), Value::String(header.value.clone()))
            .is_some()
        {
            return Err(preview_invalid(
                "preview header was rejected before any browser command",
            ));
        }
    }
    let (secret, authorization) = match (username, password_env) {
        (Some(username), Some(name)) => {
            if !username_ok(username) {
                return Err(preview_invalid(
                    "preview header was rejected before any browser command",
                ));
            }
            let password = std::env::var(name).unwrap_or_default();
            if password.is_empty() {
                return Err(DriverError::new(
                    "test.driver.web.secret_unavailable",
                    format!("environment variable {name} is unset or empty"),
                ));
            }
            if map.contains_key("Authorization") {
                return Err(preview_invalid(
                    "preview header was rejected before any browser command",
                ));
            }
            let token = STANDARD.encode(format!("{username}:{password}"));
            let authorization = format!("Basic {token}");
            map.insert(
                "Authorization".to_string(),
                Value::String(authorization.clone()),
            );
            (Some(password), Some(authorization))
        }
        (None, None) => (None, None),
        _ => {
            return Err(preview_invalid(
                "basic auth requires both a username and a password environment name",
            ));
        }
    };
    if map.is_empty() {
        return Err(preview_invalid(
            "preview header was rejected before any browser command",
        ));
    }
    Ok((Value::Object(map).to_string(), secret, authorization))
}

fn header_name_ok(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    !matches!(
        lowered.as_str(),
        "authorization"
            | "cookie"
            | "host"
            | "connection"
            | "content-length"
            | "transfer-encoding"
            | "proxy-authorization"
    ) && !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-'))
}

fn header_value_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1_024
        && value.bytes().all(|byte| byte >= 0x20 && byte != 0x7f)
}

fn username_ok(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 128
        && !username.contains(':')
        && username.bytes().all(|byte| byte >= 0x20 && byte != 0x7f)
}

fn preview_invalid(message: &str) -> DriverError {
    DriverError::new("test.driver.web.preview_headers_invalid", message)
}

fn retitle_preview(error: DriverError) -> DriverError {
    if error.code() == "test.driver.web.cookie_origin_invalid" {
        DriverError::new(
            "test.driver.web.preview_origin_invalid",
            "preview headers were rejected before any browser command",
        )
        .with_retryable(error.retryable())
    } else if error.code() == "test.driver.web.navigation_origin_denied" {
        DriverError::new(error.code(), error.message()).with_retryable(error.retryable())
    } else {
        error
    }
}

fn scrub_cookie_values(error: DriverError, cookies: &[CookieAssignment]) -> DriverError {
    let mut message = error.message().to_string();
    for cookie in cookies {
        if !cookie.value.is_empty() {
            message = redact_secret_text(&message, &cookie.value);
        }
    }
    DriverError::new(error.code(), message).with_retryable(error.retryable())
}

fn scrub_value(value: &mut Value, secret: &str) {
    let token = STANDARD.encode(secret);
    match value {
        Value::String(text) => {
            *text = redact_secret_text(text, secret);
            *text = redact_secret_text(text, &token);
        }
        Value::Array(items) => {
            for item in items {
                scrub_value(item, secret);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                scrub_value(item, secret);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn inert_names(value: &Value) -> Vec<String> {
    let result = super::browser_result(value.clone());
    result
        .get("texts")
        .and_then(Value::as_array)
        .map(|texts| {
            texts
                .iter()
                .filter_map(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn omit_inert_value(value: &mut Value, names: &[String]) {
    match value {
        Value::String(text) => {
            if names.iter().any(|name| text.contains(name)) {
                let filtered = text
                    .lines()
                    .filter(|line| !line_mentions_inert(line, names))
                    .collect::<Vec<_>>()
                    .join("\n");
                *text = filtered;
            }
        }
        Value::Array(items) => {
            items.retain(|item| !named_inert(item, names));
            for item in items {
                omit_inert_value(item, names);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                omit_inert_value(item, names);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn named_inert(value: &Value, names: &[String]) -> bool {
    value
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| names.iter().any(|inert| inert == name))
}

fn line_mentions_inert(line: &str, names: &[String]) -> bool {
    names
        .iter()
        .any(|name| line.contains(&format!("\"{name}\"")) || line.contains(&format!("'{name}'")))
}
