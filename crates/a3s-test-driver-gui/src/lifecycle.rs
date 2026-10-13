use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::DriverError;

use crate::api::{CuaApi, CuaApp, CuaWindow};
use crate::{ApplicationIdentity, GuiAppTarget, LaunchSpec, WindowSelector};

const WINDOW_DISCOVERY_ATTEMPTS: usize = 5;
const WINDOW_DISCOVERY_INTERVAL: Duration = Duration::from_millis(100);
const APPLICATION_EXIT_ATTEMPTS: usize = 50;
const APPLICATION_EXIT_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub(crate) struct ApplicationBinding {
    pub pid: i32,
    pub identity: ApplicationIdentity,
    pub name: String,
    pub process_name: Option<String>,
    pub owned: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct WindowBinding {
    pub window_id: u32,
    pub title: String,
}

pub(crate) async fn bind_application(
    api: &CuaApi,
    target: &GuiAppTarget,
) -> Result<ApplicationBinding, DriverError> {
    refuse_before_application_tools(target)?;
    match target {
        GuiAppTarget::Launch(spec) => launch_application(api, spec).await,
        GuiAppTarget::Attach(spec) => {
            let apps = api.list_apps().await?;
            let selected = if let Some(process_id) = spec.process_id {
                let pid = i32::try_from(process_id.get()).map_err(|_| {
                    DriverError::new(
                        "test.driver.gui.app_identity_invalid",
                        "configured process id exceeds the CUA process-id range",
                    )
                })?;
                let app = apps
                    .iter()
                    .find(|app| app.running && app.pid == pid)
                    .ok_or_else(|| {
                        DriverError::new(
                            "test.driver.gui.app_not_found",
                            "no running application matched the configured process id",
                        )
                    })?;
                ensure_app_matches_attach_identity(
                    app,
                    &spec.application,
                    spec.process_name.as_deref(),
                )?;
                app
            } else {
                select_exact_app(matching_running_apps(
                    &apps,
                    &spec.application,
                    spec.process_name.as_deref(),
                )?)?
            };
            Ok(ApplicationBinding {
                pid: selected.pid,
                identity: spec.application.clone(),
                name: selected.name.clone(),
                process_name: spec.process_name.clone(),
                owned: false,
            })
        }
    }
}

async fn launch_application(
    api: &CuaApi,
    spec: &LaunchSpec,
) -> Result<ApplicationBinding, DriverError> {
    if spec.working_directory.is_some() {
        return Err(DriverError::new(
            "test.driver.gui.launch_option_unsupported",
            "the locked CUA launch_app contract does not support a working directory",
        ));
    }
    let arguments = unicode_arguments(&spec.arguments)?;
    let before = api.list_apps().await?;
    let previous_pids: BTreeSet<i32> = matching_running_apps(&before, &spec.application, None)?
        .into_iter()
        .map(|app| app.pid)
        .collect();
    let launched = match &spec.application {
        ApplicationIdentity::MacOsBundle { bundle_id } => {
            let launched = api.launch_macos_app(bundle_id, &arguments).await?;
            if launched.pid <= 0 || launched.bundle_id.as_deref() != Some(bundle_id.as_str()) {
                return Err(launch_identity_invalid());
            }
            launched
        }
        ApplicationIdentity::WindowsExecutable {
            path,
            expected_publisher,
        } => {
            reject_unproven_publisher(expected_publisher)?;
            let path = windows_launch_path(path)?;
            let launched = api.launch_windows_app(&path, &arguments).await?;
            if launched.pid <= 0
                || launched.bundle_id.is_some()
                || !windows_name_matches(&launched.name, Path::new(&path))
            {
                return Err(launch_identity_invalid());
            }
            launched
        }
        ApplicationIdentity::LinuxDesktop { .. } => {
            return Err(platform_unsupported(&spec.application));
        }
    };
    if previous_pids.contains(&launched.pid) {
        return Err(DriverError::new(
            "test.driver.gui.app_ownership_unproven",
            "CUA returned a process that was already running before this GUI session",
        ));
    }
    Ok(ApplicationBinding {
        pid: launched.pid,
        identity: spec.application.clone(),
        name: launched.name,
        process_name: None,
        owned: true,
    })
}

fn unicode_arguments(arguments: &[std::ffi::OsString]) -> Result<Vec<String>, DriverError> {
    arguments
        .iter()
        .map(|argument| {
            argument.to_str().map(ToOwned::to_owned).ok_or_else(|| {
                DriverError::new(
                    "test.driver.gui.launch_option_invalid",
                    "GUI application arguments must be valid Unicode",
                )
            })
        })
        .collect()
}

fn launch_identity_invalid() -> DriverError {
    DriverError::new(
        "test.driver.gui.app_identity_invalid",
        "CUA launch_app returned an application identity that did not match the request",
    )
}

fn reject_unproven_publisher(publisher: &Option<String>) -> Result<(), DriverError> {
    crate::config::reject_unproven_publisher(publisher)
}

fn refuse_before_application_tools(target: &GuiAppTarget) -> Result<(), DriverError> {
    let (identity, launching) = match target {
        GuiAppTarget::Launch(spec) => (&spec.application, true),
        GuiAppTarget::Attach(spec) => (&spec.application, false),
    };
    match identity {
        ApplicationIdentity::WindowsExecutable {
            path,
            expected_publisher,
        } => {
            reject_unproven_publisher(expected_publisher)?;
            if launching {
                windows_launch_path(path)?;
            }
            Ok(())
        }
        ApplicationIdentity::LinuxDesktop { .. } => Err(platform_unsupported(identity)),
        ApplicationIdentity::MacOsBundle { .. } => Ok(()),
    }
}

fn windows_launch_path(path: &Path) -> Result<String, DriverError> {
    let text = path.to_str().ok_or_else(|| {
        DriverError::new(
            "test.driver.gui.launch_option_invalid",
            "Windows application path must be valid Unicode",
        )
    })?;
    if text.trim().is_empty() || !path.is_absolute() {
        return Err(DriverError::new(
            "test.driver.gui.launch_option_invalid",
            "Windows application path must be an absolute Unicode path",
        ));
    }
    Ok(text.to_string())
}

/// `list_apps` reports the executable file name. `launch_app` for a path
/// reports that path. Both identify the configured executable.
pub(crate) fn windows_name_matches(reported: &str, path: &Path) -> bool {
    let reported = reported.trim();
    if reported.is_empty() {
        return false;
    }
    if let Some(path_text) = path.to_str() {
        if reported.eq_ignore_ascii_case(path_text) {
            return true;
        }
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| reported.eq_ignore_ascii_case(name))
}

fn reported_bundle_id(app: &CuaApp) -> Option<&str> {
    app.bundle_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn ensure_app_matches_attach_identity(
    app: &CuaApp,
    identity: &ApplicationIdentity,
    process_name: Option<&str>,
) -> Result<(), DriverError> {
    match identity {
        ApplicationIdentity::MacOsBundle { bundle_id } => match reported_bundle_id(app) {
            Some(id) if id == bundle_id => {
                if let Some(expected_name) = process_name {
                    if app.name != expected_name {
                        return Err(DriverError::new(
                            "test.driver.gui.app_identity_invalid",
                            "running application process name did not match the configured process name",
                        ));
                    }
                }
                Ok(())
            }
            Some(_) => Err(DriverError::new(
                "test.driver.gui.app_identity_invalid",
                "running application bundle identifier did not match the configured identity",
            )),
            None => {
                let Some(expected_name) = process_name else {
                    return Err(DriverError::new(
                        "test.driver.gui.app_identity_incomplete",
                        "running application has no bundle identifier; configure an exact process name to attach unpackaged macOS binaries",
                    ));
                };
                if app.name == expected_name {
                    Ok(())
                } else {
                    Err(DriverError::new(
                        "test.driver.gui.app_identity_invalid",
                        "unpackaged application process name did not match the configured process name",
                    ))
                }
            }
        },
        ApplicationIdentity::WindowsExecutable {
            path,
            expected_publisher,
        } => {
            reject_unproven_publisher(expected_publisher)?;
            if !windows_name_matches(&app.name, path) {
                return Err(DriverError::new(
                    "test.driver.gui.app_identity_invalid",
                    "running application did not match the configured Windows executable",
                ));
            }
            if let Some(expected_name) = process_name {
                if app.name != expected_name {
                    return Err(DriverError::new(
                        "test.driver.gui.app_identity_invalid",
                        "running application process name did not match the configured process name",
                    ));
                }
            }
            Ok(())
        }
        ApplicationIdentity::LinuxDesktop { .. } => Err(platform_unsupported(identity)),
    }
}

fn matching_running_apps<'a>(
    apps: &'a [CuaApp],
    identity: &ApplicationIdentity,
    process_name: Option<&str>,
) -> Result<Vec<&'a CuaApp>, DriverError> {
    match identity {
        ApplicationIdentity::MacOsBundle { bundle_id } => Ok(apps
            .iter()
            .filter(|app| {
                if !app.running {
                    return false;
                }
                match reported_bundle_id(app) {
                    Some(id) => {
                        id == bundle_id && process_name.is_none_or(|expected| app.name == expected)
                    }
                    None => process_name.is_some_and(|expected| app.name == expected),
                }
            })
            .collect()),
        ApplicationIdentity::WindowsExecutable {
            path,
            expected_publisher,
        } => {
            reject_unproven_publisher(expected_publisher)?;
            Ok(apps
                .iter()
                .filter(|app| {
                    app.running
                        && windows_name_matches(&app.name, path)
                        && process_name.is_none_or(|expected| app.name == expected)
                })
                .collect())
        }
        ApplicationIdentity::LinuxDesktop { .. } => Err(platform_unsupported(identity)),
    }
}

fn select_exact_app(matches: Vec<&CuaApp>) -> Result<&CuaApp, DriverError> {
    match matches.as_slice() {
        [] => Err(DriverError::new(
            "test.driver.gui.app_not_found",
            "no running application matched the configured identity",
        )),
        [app] => Ok(*app),
        _ => Err(DriverError::new(
            "test.driver.gui.app_ambiguous",
            "multiple running applications matched; configure an explicit process id",
        )),
    }
}

fn platform_unsupported(identity: &ApplicationIdentity) -> DriverError {
    let platform = match identity {
        ApplicationIdentity::MacOsBundle { .. } => "macOS",
        ApplicationIdentity::WindowsExecutable { .. } => "Windows",
        ApplicationIdentity::LinuxDesktop { .. } => "Linux",
    };
    DriverError::new(
        "test.driver.gui.platform_unsupported",
        format!("the locked CUA adapter does not implement {platform} application identity"),
    )
}

pub(crate) async fn bind_window(
    api: &CuaApi,
    application: &ApplicationBinding,
    selector: &WindowSelector,
) -> Result<WindowBinding, DriverError> {
    for attempt in 0..WINDOW_DISCOVERY_ATTEMPTS {
        let windows = api.list_windows(application.pid).await?;
        match select_window(windows, selector) {
            Ok(window) => {
                return Ok(WindowBinding {
                    window_id: window.window_id,
                    title: window.title,
                });
            }
            Err(error)
                if error.code() == "test.driver.gui.window_not_found"
                    && attempt + 1 < WINDOW_DISCOVERY_ATTEMPTS =>
            {
                tokio::time::sleep(WINDOW_DISCOVERY_INTERVAL).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(DriverError::new(
        "test.driver.gui.window_not_found",
        "application did not expose a matching top-level window",
    ))
}

fn select_window(
    windows: Vec<CuaWindow>,
    selector: &WindowSelector,
) -> Result<CuaWindow, DriverError> {
    let matches = match selector {
        WindowSelector::Primary => {
            let Some(highest) = windows.iter().map(primary_window_score).max() else {
                return Err(window_not_found());
            };
            windows
                .into_iter()
                .filter(|window| primary_window_score(window) == highest)
                .collect::<Vec<_>>()
        }
        WindowSelector::ExactTitle(title) => windows
            .into_iter()
            .filter(|window| window.title == *title)
            .collect(),
        WindowSelector::AutomationId(automation_id) => windows
            .into_iter()
            .filter(|window| window.automation_id.as_deref() == Some(automation_id.as_str()))
            .collect(),
    };
    match matches.as_slice() {
        [] => Err(window_not_found()),
        [window] => Ok(window.clone()),
        _ => Err(DriverError::new(
            "test.driver.gui.window_ambiguous",
            "multiple top-level windows matched the configured selector",
        )),
    }
}

/// Prefer a visible window on the active Space over an off-screen auxiliary
/// window, even when that auxiliary window has a higher WindowServer z-index.
/// Newer CUA versions expose every layer-0 window and explicitly document that
/// z-index alone is not a primary-window selector.
fn primary_window_score(window: &CuaWindow) -> (u8, u8, i64) {
    (
        u8::from(window.is_on_screen),
        u8::from(window.on_current_space == Some(true)),
        window.z_index.unwrap_or(i64::MIN),
    )
}

fn window_not_found() -> DriverError {
    DriverError::new(
        "test.driver.gui.window_not_found",
        "application did not expose a matching top-level window",
    )
}

pub(crate) async fn validate_runtime_binding(
    api: &CuaApi,
    application: &ApplicationBinding,
    window: &WindowBinding,
) -> Result<(), DriverError> {
    let apps = api.list_apps().await?;
    let Some(current) = apps
        .iter()
        .find(|candidate| candidate.running && candidate.pid == application.pid)
    else {
        return Err(application_binding_lost(application));
    };
    if matching_running_apps(
        std::slice::from_ref(current),
        &application.identity,
        application.process_name.as_deref(),
    )?
    .len()
        != 1
    {
        return Err(application_binding_lost(application));
    }

    let windows = api.list_windows(application.pid).await?;
    if !windows
        .iter()
        .any(|candidate| candidate.window_id == window.window_id)
    {
        return Err(DriverError::new(
            "test.driver.gui.window_binding_lost",
            format!(
                "bound window {} is no longer present for process {}; refusing GUI observation or input",
                window.window_id, application.pid
            ),
        ));
    }
    Ok(())
}

fn application_binding_lost(application: &ApplicationBinding) -> DriverError {
    DriverError::new(
        "test.driver.gui.application_binding_lost",
        format!(
            "process {} is no longer running with the configured application identity; refusing GUI observation or input",
            application.pid
        ),
    )
}

pub(crate) async fn cleanup_resources(
    api: Arc<CuaApi>,
    application: Option<ApplicationBinding>,
) -> Result<(), DriverError> {
    let mut first_error = None;
    if let Some(application) = application.filter(|application| application.owned) {
        match api.list_apps().await {
            Ok(apps) => {
                if let Some(current) = apps
                    .iter()
                    .find(|app| app.running && app.pid == application.pid)
                {
                    match matching_running_apps(
                        std::slice::from_ref(current),
                        &application.identity,
                        application.process_name.as_deref(),
                    ) {
                        Ok(matches) if matches.len() == 1 => {
                            if let Err(error) = api.kill_app(application.pid).await {
                                return Err(retryable_cleanup_error(error));
                            }
                            match wait_for_application_exit(&api, &application).await {
                                Ok(()) => {}
                                Err(error) if error.retryable() => return Err(error),
                                Err(error) => first_error = Some(error),
                            }
                        }
                        _ => {
                            first_error = Some(DriverError::new(
                                "test.driver.gui.app_ownership_lost",
                                "owned process id now belongs to a different application; refusing to terminate it",
                            ));
                        }
                    }
                }
            }
            Err(error) => return Err(retryable_cleanup_error(error)),
        }
    }
    if first_error.is_none() {
        if let Err(error) = api.end_session().await {
            return Err(retryable_cleanup_error(error));
        }
    } else if let Err(error) = api.end_session().await {
        first_error.get_or_insert(error);
    }
    if let Err(error) = api.close().await {
        first_error.get_or_insert(error.with_retryable(false));
    }
    first_error.map_or(Ok(()), Err)
}

async fn wait_for_application_exit(
    api: &CuaApi,
    application: &ApplicationBinding,
) -> Result<(), DriverError> {
    for attempt in 0..APPLICATION_EXIT_ATTEMPTS {
        let apps = api.list_apps().await.map_err(retryable_cleanup_error)?;
        let Some(current) = apps
            .iter()
            .find(|app| app.running && app.pid == application.pid)
        else {
            return Ok(());
        };
        if matching_running_apps(
            std::slice::from_ref(current),
            &application.identity,
            application.process_name.as_deref(),
        )?
        .len()
            != 1
        {
            return Err(DriverError::new(
                "test.driver.gui.app_ownership_lost",
                "owned process id was reused before application termination could be confirmed",
            ));
        }
        if attempt + 1 < APPLICATION_EXIT_ATTEMPTS {
            tokio::time::sleep(APPLICATION_EXIT_INTERVAL).await;
        }
    }
    Err(DriverError::new(
        "test.driver.gui.app_cleanup_incomplete",
        format!(
            "CUA acknowledged termination of owned process {} but the application is still running",
            application.pid
        ),
    )
    .with_retryable(true))
}

fn retryable_cleanup_error(error: DriverError) -> DriverError {
    let retryable = matches!(
        error.code(),
        "test.driver.gui.cua_rpc_error"
            | "test.driver.gui.cua_tool_failed"
            | "test.driver.gui.cua_output_invalid"
    );
    error.with_retryable(retryable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::CuaApp;
    use serde_json::json;

    fn window(
        window_id: u32,
        z_index: Option<i64>,
        is_on_screen: bool,
        on_current_space: Option<bool>,
    ) -> CuaWindow {
        CuaWindow {
            window_id,
            pid: 1,
            title: String::new(),
            z_index,
            is_on_screen,
            on_current_space,
            automation_id: None,
        }
    }

    #[test]
    fn primary_prefers_visible_active_space_over_higher_z_index_auxiliary_window() {
        let selected = select_window(
            vec![
                window(10, Some(106), false, None),
                window(11, Some(105), true, Some(true)),
            ],
            &WindowSelector::Primary,
        )
        .expect("primary window");

        assert_eq!(selected.window_id, 11);
    }

    #[test]
    fn primary_falls_back_to_z_index_when_visibility_is_unknown() {
        let selected = select_window(
            vec![
                window(10, Some(106), false, None),
                window(11, Some(105), false, None),
            ],
            &WindowSelector::Primary,
        )
        .expect("primary window");

        assert_eq!(selected.window_id, 10);
    }

    #[test]
    fn cua_window_accepts_nullable_z_index_and_missing_visibility_metadata() {
        let parsed: CuaWindow = serde_json::from_value(json!({
            "window_id": 10,
            "pid": 1,
            "title": "Auxiliary",
            "z_index": null,
        }))
        .expect("CUA window with nullable z-index");

        assert_eq!(parsed.z_index, None);
        assert!(!parsed.is_on_screen);
        assert_eq!(parsed.on_current_space, None);
    }

    #[test]
    fn attach_accepts_unpackaged_macos_app_when_process_name_matches() {
        let app = CuaApp {
            pid: 42,
            name: "A3S".to_string(),
            bundle_id: None,
            running: true,
        };
        ensure_app_matches_attach_identity(
            &app,
            &ApplicationIdentity::MacOsBundle {
                bundle_id: "dev.a3s.desktop".to_string(),
            },
            Some("A3S"),
        )
        .expect("unpackaged attach");

        let matches = matching_running_apps(
            std::slice::from_ref(&app),
            &ApplicationIdentity::MacOsBundle {
                bundle_id: "dev.a3s.desktop".to_string(),
            },
            Some("A3S"),
        )
        .expect("match unpackaged");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn attach_refuses_unpackaged_macos_app_without_process_name() {
        let app = CuaApp {
            pid: 42,
            name: "A3S".to_string(),
            bundle_id: None,
            running: true,
        };
        let error = ensure_app_matches_attach_identity(
            &app,
            &ApplicationIdentity::MacOsBundle {
                bundle_id: "dev.a3s.desktop".to_string(),
            },
            None,
        )
        .expect_err("missing process name");
        assert_eq!(error.code(), "test.driver.gui.app_identity_incomplete");
    }

    #[test]
    fn attach_refuses_bundle_mismatch_even_when_process_name_matches() {
        let app = CuaApp {
            pid: 42,
            name: "A3S".to_string(),
            bundle_id: Some("com.other.app".to_string()),
            running: true,
        };
        let error = ensure_app_matches_attach_identity(
            &app,
            &ApplicationIdentity::MacOsBundle {
                bundle_id: "dev.a3s.desktop".to_string(),
            },
            Some("A3S"),
        )
        .expect_err("bundle mismatch");
        assert_eq!(error.code(), "test.driver.gui.app_identity_invalid");
    }

    #[test]
    fn windows_identity_matches_launch_path_and_process_file_name() {
        let path = Path::new(r"C:\Windows\System32\notepad.exe");
        assert!(windows_name_matches(
            r"C:\Windows\System32\notepad.exe",
            path
        ));
        assert!(windows_name_matches("notepad.exe", path));
        assert!(!windows_name_matches("notepad", path));
        #[cfg(windows)]
        {
            assert_eq!(
                windows_launch_path(path).expect("absolute Windows launch path"),
                path.to_str().expect("unicode path")
            );
            let error = windows_launch_path(Path::new("notepad.exe"))
                .expect_err("relative Windows launch path");
            assert_eq!(error.code(), "test.driver.gui.launch_option_invalid");
        }

        let app = CuaApp {
            pid: 7,
            name: "notepad.exe".to_string(),
            bundle_id: None,
            running: true,
        };
        ensure_app_matches_attach_identity(
            &app,
            &ApplicationIdentity::WindowsExecutable {
                path: path.to_path_buf(),
                expected_publisher: None,
            },
            None,
        )
        .expect("windows attach");
        ensure_app_matches_attach_identity(
            &app,
            &ApplicationIdentity::WindowsExecutable {
                path: path.to_path_buf(),
                expected_publisher: None,
            },
            Some("notepad.exe"),
        )
        .expect("exact list_apps file name");
        let error = ensure_app_matches_attach_identity(
            &app,
            &ApplicationIdentity::WindowsExecutable {
                path: path.to_path_buf(),
                expected_publisher: None,
            },
            Some("notepad"),
        )
        .expect_err("file name must match list_apps exactly");
        assert_eq!(error.code(), "test.driver.gui.app_identity_invalid");
        let apps = [app.clone()];
        let identity = ApplicationIdentity::WindowsExecutable {
            path: path.to_path_buf(),
            expected_publisher: None,
        };
        assert_eq!(
            matching_running_apps(&apps, &identity, Some("notepad.exe"))
                .expect("exact file name filter")
                .len(),
            1
        );
        assert!(matching_running_apps(&apps, &identity, Some("notepad"))
            .expect("inexact file name filter")
            .is_empty());

        let error = ensure_app_matches_attach_identity(
            &app,
            &ApplicationIdentity::WindowsExecutable {
                path: path.to_path_buf(),
                expected_publisher: Some("Microsoft".to_string()),
            },
            None,
        )
        .expect_err("publisher cannot be proven");
        assert_eq!(error.code(), "test.driver.gui.publisher_unproven");
    }
}
