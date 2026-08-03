use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::DriverError;

use crate::api::{CuaApi, CuaApp, CuaPermissions, CuaWindow};
use crate::{ApplicationIdentity, CuaEndpoint, GuiAppTarget, LaunchSpec, WindowSelector};

const WINDOW_DISCOVERY_ATTEMPTS: usize = 5;
const WINDOW_DISCOVERY_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub(crate) struct ApplicationBinding {
    pub pid: i32,
    pub identity: ApplicationIdentity,
    pub name: String,
    pub owned: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct WindowBinding {
    pub window_id: u32,
    pub title: String,
}

pub(crate) fn validate_permissions(
    endpoint: &CuaEndpoint,
    permissions: &CuaPermissions,
) -> Result<(), DriverError> {
    let expected_attribution = match endpoint {
        CuaEndpoint::InstalledDaemon { .. } => "driver-daemon",
        CuaEndpoint::EmbeddedSocket { .. } => "host",
    };
    if permissions.source.attribution != expected_attribution {
        return Err(DriverError::new(
            "test.driver.gui.permission_identity_invalid",
            format!(
                "CUA permission status is attributed to '{}', expected '{expected_attribution}'",
                permissions.source.attribution
            ),
        ));
    }
    if !permissions.accessibility || !permissions.screen_recording {
        let mut missing = Vec::new();
        if !permissions.accessibility {
            missing.push("accessibility");
        }
        if !permissions.screen_recording {
            missing.push("screen_recording");
        }
        return Err(DriverError::new(
            "test.driver.gui.permission_missing",
            format!(
                "CUA is missing required permissions: {}",
                missing.join(", ")
            ),
        ));
    }
    Ok(())
}

pub(crate) async fn bind_application(
    api: &CuaApi,
    target: &GuiAppTarget,
) -> Result<ApplicationBinding, DriverError> {
    match target {
        GuiAppTarget::Launch(spec) => launch_application(api, spec).await,
        GuiAppTarget::Attach(spec) => {
            let apps = api.list_apps().await?;
            let matches = matching_running_apps(&apps, &spec.application)?;
            let selected = if let Some(process_id) = spec.process_id {
                let pid = i32::try_from(process_id.get()).map_err(|_| {
                    DriverError::new(
                        "test.driver.gui.app_identity_invalid",
                        "configured process id exceeds the CUA process-id range",
                    )
                })?;
                matches
                    .into_iter()
                    .find(|app| app.pid == pid)
                    .ok_or_else(|| {
                        DriverError::new(
                            "test.driver.gui.app_not_found",
                            "no running application matched the configured identity and process id",
                        )
                    })?
            } else {
                select_exact_app(matches)?
            };
            Ok(ApplicationBinding {
                pid: selected.pid,
                identity: spec.application.clone(),
                name: selected.name.clone(),
                owned: false,
            })
        }
    }
}

async fn launch_application(
    api: &CuaApi,
    spec: &LaunchSpec,
) -> Result<ApplicationBinding, DriverError> {
    let ApplicationIdentity::MacOsBundle { bundle_id } = &spec.application else {
        return Err(platform_unsupported(&spec.application));
    };
    if spec.working_directory.is_some() {
        return Err(DriverError::new(
            "test.driver.gui.launch_option_unsupported",
            "the locked CUA launch_app contract does not support a working directory",
        ));
    }
    let arguments = spec
        .arguments
        .iter()
        .map(|argument| {
            argument.to_str().map(ToOwned::to_owned).ok_or_else(|| {
                DriverError::new(
                    "test.driver.gui.launch_option_invalid",
                    "GUI application arguments must be valid Unicode",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let before = api.list_apps().await?;
    let previous_pids: BTreeSet<i32> = matching_running_apps(&before, &spec.application)?
        .into_iter()
        .map(|app| app.pid)
        .collect();
    let launched = api.launch_macos_app(bundle_id, &arguments).await?;
    if launched.pid <= 0 || launched.bundle_id != *bundle_id {
        return Err(DriverError::new(
            "test.driver.gui.app_identity_invalid",
            "CUA launch_app returned an application identity that did not match the request",
        ));
    }
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
        owned: true,
    })
}

fn matching_running_apps<'a>(
    apps: &'a [CuaApp],
    identity: &ApplicationIdentity,
) -> Result<Vec<&'a CuaApp>, DriverError> {
    match identity {
        ApplicationIdentity::MacOsBundle { bundle_id } => Ok(apps
            .iter()
            .filter(|app| {
                app.running
                    && app
                        .bundle_id
                        .as_deref()
                        .is_some_and(|value| value == bundle_id)
            })
            .collect()),
        _ => Err(platform_unsupported(identity)),
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
        format!("the locked CUA 0.10.0 adapter does not implement {platform} application identity"),
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
            let Some(highest) = windows.iter().map(|window| window.z_index).max() else {
                return Err(window_not_found());
            };
            windows
                .into_iter()
                .filter(|window| window.z_index == highest)
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
    if matching_running_apps(std::slice::from_ref(current), &application.identity)?.len() != 1 {
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
                    ) {
                        Ok(matches) if matches.len() == 1 => {
                            if let Err(error) = api.kill_app(application.pid).await {
                                return Err(retryable_cleanup_error(error));
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

fn retryable_cleanup_error(error: DriverError) -> DriverError {
    let retryable = matches!(
        error.code(),
        "test.driver.gui.cua_rpc_error"
            | "test.driver.gui.cua_tool_failed"
            | "test.driver.gui.cua_output_invalid"
    );
    error.with_retryable(retryable)
}
