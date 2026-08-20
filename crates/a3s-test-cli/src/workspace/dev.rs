use std::future::pending;
use std::process::{ExitCode, ExitStatus};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::config::{ProjectBrowserDriver, ProjectProfile};
use super::discovery::testkit_install_command;
use super::process::OwnedServer;
use super::repair_bridge::{LocalRepairBridge, RepairBridgeBatch, RepairBridgeEvent};
use super::{config, doctor, DevArgs};
use crate::agent_session::{abort_dev_session, start_dev_session, DevSession, DevSessionRequest};
use crate::BrowserDriverKind;

const URL_PROBE_TIMEOUT: Duration = Duration::from_millis(500);
const URL_PROBE_INTERVAL: Duration = Duration::from_millis(100);

pub(super) async fn execute(args: DevArgs) -> Result<ExitCode> {
    let profile = config::load(&args.root, &args.config).await?;
    doctor::dev_preflight(&profile).await?;
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(URL_PROBE_TIMEOUT)
        .build()
        .context("failed to prepare the development URL probe")?;
    let cancellation = CancellationToken::new();
    let signal_task = crate::install_interrupt_handler(cancellation.clone());
    let result = run(profile, args.json, client, cancellation).await;
    signal_task.abort();
    let _ = signal_task.await;
    result
}

async fn run(
    profile: ProjectProfile,
    json_output: bool,
    client: Client,
    cancellation: CancellationToken,
) -> Result<ExitCode> {
    let mut owned_server = None;
    let server_kind = if probe(&client, &profile).await {
        "existing"
    } else {
        let mut server = OwnedServer::spawn(&profile.dev_server)?;
        match wait_for_ready(&client, &profile, &mut server, &cancellation).await? {
            Startup::Ready => {
                owned_server = Some(server);
                "started"
            }
            Startup::Interrupted => {
                let cleanup = server
                    .shutdown(Duration::from_millis(profile.dev_server.cleanup_timeout_ms))
                    .await;
                emit(
                    json_output,
                    json!({
                        "protocol": "a3s.test.dev/1",
                        "event": "stopped",
                        "project": profile.id,
                        "url": profile.dev_server.url.to_string(),
                        "server": "started",
                        "reason": "interrupt",
                        "cleanup": if cleanup.is_ok() { "complete" } else { "failed" },
                    }),
                    "Development review stopped before the browser session started",
                )?;
                cleanup.context(
                    "development review was interrupted and owned server cleanup failed",
                )?;
                return Ok(ExitCode::from(130));
            }
            Startup::Exited(status) => {
                let cleanup = server
                    .shutdown(Duration::from_millis(profile.dev_server.cleanup_timeout_ms))
                    .await;
                emit_server_exit(json_output, &profile, None, status, cleanup.is_ok())?;
                cleanup.context(
                    "development server exited during startup and process-tree cleanup failed",
                )?;
                return Ok(ExitCode::from(1));
            }
            Startup::TimedOut => {
                let cleanup = server
                    .shutdown(Duration::from_millis(profile.dev_server.cleanup_timeout_ms))
                    .await;
                if let Err(cleanup) = cleanup {
                    anyhow::bail!(
                        "development server did not make {} reachable within {} ms; process-tree cleanup also failed: {cleanup:#}",
                        profile.dev_server.url,
                        profile.dev_server.startup_timeout_ms
                    );
                }
                anyhow::bail!(
                    "development server did not make {} reachable within {} ms",
                    profile.dev_server.url,
                    profile.dev_server.startup_timeout_ms
                );
            }
        }
    };

    let session = match start_browser_session(&profile).await {
        Ok(session) => session,
        Err(error) => {
            if let Some(server) = owned_server.take() {
                if let Err(cleanup) = server
                    .shutdown(Duration::from_millis(profile.dev_server.cleanup_timeout_ms))
                    .await
                {
                    anyhow::bail!(
                        "development review browser failed to start: {error:#}; development server cleanup also failed: {cleanup:#}"
                    );
                }
            }
            return Err(error.context("failed to start the development review browser"));
        }
    };
    let repair_bridge = session
        .testkit
        .as_ref()
        .map(|_| LocalRepairBridge::new(profile.root.clone(), session.session.clone()));
    let repair_bridge_metadata = repair_bridge.as_ref().map(LocalRepairBridge::metadata);
    let stop = if cancellation.is_cancelled() {
        Stop::Interrupted
    } else {
        emit(
            json_output,
            json!({
                "protocol": "a3s.test.dev/1",
                "event": "ready",
                "project": profile.id,
                "url": profile.dev_server.url.to_string(),
                "server": server_kind,
                "session": session.session,
                "artifacts_dir": session.artifacts_dir,
                "testkit": session.testkit,
                "repair_bridge": repair_bridge_metadata,
            }),
            &format!(
                "A3S Test review is ready at {} (session '{}')",
                profile.dev_server.url, session.session
            ),
        )?;
        monitor_review(
            &profile,
            json_output,
            &cancellation,
            &mut owned_server,
            repair_bridge,
        )
        .await
    };

    let browser_cleanup = abort_dev_session(&profile.root, &session.session).await;
    let server_cleanup = match owned_server.take() {
        Some(server) => {
            server
                .shutdown(Duration::from_millis(profile.dev_server.cleanup_timeout_ms))
                .await
        }
        None => Ok(()),
    };
    let cleanup_complete = browser_cleanup.is_ok() && server_cleanup.is_ok();
    match &stop {
        Stop::Interrupted => emit(
            json_output,
            json!({
                "protocol": "a3s.test.dev/1",
                "event": "stopped",
                "project": profile.id,
                "url": profile.dev_server.url.to_string(),
                "server": server_kind,
                "session": session.session,
                "reason": "interrupt",
                "cleanup": if cleanup_complete { "complete" } else { "failed" },
            }),
            "Development review stopped",
        )?,
        Stop::ServerExited(status) => {
            emit_server_exit(
                json_output,
                &profile,
                Some(&session),
                *status,
                cleanup_complete,
            )?;
        }
        Stop::RepairBridgeFailed(_) => emit_monitor_failure(
            json_output,
            &profile,
            &session,
            server_kind,
            "repair_bridge_error",
            cleanup_complete,
            "Local repair bridge stopped unexpectedly",
        )?,
        Stop::ServerWaitFailed(_) => emit_monitor_failure(
            json_output,
            &profile,
            &session,
            server_kind,
            "server_wait_error",
            cleanup_complete,
            "Development server monitoring stopped unexpectedly",
        )?,
    }
    let cleanup = finish_cleanup(browser_cleanup, server_cleanup);
    match (stop, cleanup) {
        (Stop::Interrupted, Ok(())) => Ok(ExitCode::from(130)),
        (Stop::ServerExited(_), Ok(())) => Ok(ExitCode::from(1)),
        (Stop::RepairBridgeFailed(error), Ok(())) => {
            Err(error.context("local repair bridge failed"))
        }
        (Stop::ServerWaitFailed(error), Ok(())) => {
            Err(error.context("development server monitoring failed"))
        }
        (Stop::RepairBridgeFailed(error), Err(cleanup)) => anyhow::bail!(
            "local repair bridge failed ({error:#}); development review cleanup also failed: {cleanup:#}"
        ),
        (Stop::ServerWaitFailed(error), Err(cleanup)) => anyhow::bail!(
            "development server monitoring failed ({error:#}); development review cleanup also failed: {cleanup:#}"
        ),
        (_, Err(cleanup)) => Err(cleanup),
    }
}

async fn monitor_review(
    profile: &ProjectProfile,
    json_output: bool,
    cancellation: &CancellationToken,
    owned_server: &mut Option<OwnedServer>,
    mut repair_bridge: Option<LocalRepairBridge>,
) -> Stop {
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => return Stop::Interrupted,
            status = wait_for_owned_server(owned_server) => {
                return match status {
                    Ok(status) => Stop::ServerExited(status),
                    Err(error) => Stop::ServerWaitFailed(error),
                };
            }
            batch = next_repair_batch(&mut repair_bridge) => {
                match batch {
                    Ok(Some(batch)) => {
                        if let Err(error) = emit_repair_batch(json_output, &profile.id, batch) {
                            return Stop::RepairBridgeFailed(error);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => return Stop::RepairBridgeFailed(error),
                }
            }
        }
    }
}

async fn wait_for_owned_server(server: &mut Option<OwnedServer>) -> Result<ExitStatus> {
    match server.as_mut() {
        Some(server) => server.wait().await.map_err(anyhow::Error::new),
        None => pending().await,
    }
}

async fn next_repair_batch(
    bridge: &mut Option<LocalRepairBridge>,
) -> Result<Option<RepairBridgeBatch>> {
    match bridge.as_mut() {
        Some(bridge) => bridge.next().await,
        None => pending().await,
    }
}

async fn start_browser_session(profile: &ProjectProfile) -> Result<DevSession> {
    start_dev_session(DevSessionRequest {
        workspace: profile.root.clone(),
        url: profile.dev_server.url.to_string(),
        session_prefix: profile.browser.session.clone(),
        browser_driver: match profile.browser.driver {
            ProjectBrowserDriver::A3s => BrowserDriverKind::A3s,
            ProjectBrowserDriver::Standalone => BrowserDriverKind::Standalone,
        },
        browser_executable: profile.browser.executable.clone(),
        headed: profile.browser.headed,
        command_timeout_ms: profile.browser.command_timeout_ms,
        idle_timeout_ms: profile.browser.idle_timeout_ms,
        testkit_required: profile.testkit.required,
        testkit_install_command: testkit_install_command(&profile.dev_server.executable),
    })
    .await
}

async fn wait_for_ready(
    client: &Client,
    profile: &ProjectProfile,
    server: &mut OwnedServer,
    cancellation: &CancellationToken,
) -> Result<Startup> {
    let deadline = Instant::now() + Duration::from_millis(profile.dev_server.startup_timeout_ms);
    loop {
        if let Some(status) = server.try_wait()? {
            return Ok(Startup::Exited(status));
        }
        if cancellation.is_cancelled() {
            return Ok(Startup::Interrupted);
        }
        if probe(client, profile).await {
            return Ok(Startup::Ready);
        }
        if Instant::now() >= deadline {
            return Ok(Startup::TimedOut);
        }
        tokio::select! {
            _ = cancellation.cancelled() => return Ok(Startup::Interrupted),
            _ = tokio::time::sleep(URL_PROBE_INTERVAL) => {}
        }
    }
}

async fn probe(client: &Client, profile: &ProjectProfile) -> bool {
    client
        .get(profile.dev_server.url.clone())
        .send()
        .await
        .is_ok()
}

fn finish_cleanup(browser: Result<()>, server: Result<()>) -> Result<()> {
    match (browser, server) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error.context("failed to close the development browser")),
        (Ok(()), Err(error)) => Err(error.context("failed to close the development server")),
        (Err(browser), Err(server)) => anyhow::bail!(
            "development cleanup failed for both browser ({browser:#}) and server ({server:#})"
        ),
    }
}

fn emit_server_exit(
    json_output: bool,
    profile: &ProjectProfile,
    session: Option<&DevSession>,
    status: ExitStatus,
    cleanup_complete: bool,
) -> Result<()> {
    let mut event = json!({
        "protocol": "a3s.test.dev/1",
        "event": "stopped",
        "project": profile.id,
        "url": profile.dev_server.url.to_string(),
        "server": "started",
        "reason": "server_exit",
        "server_exit_code": status.code(),
        "cleanup": if cleanup_complete { "complete" } else { "failed" },
    });
    if let Some(session) = session {
        event["session"] = Value::String(session.session.clone());
    }
    emit(
        json_output,
        event,
        &format!("Development server exited unexpectedly ({status})"),
    )
}

fn emit_monitor_failure(
    json_output: bool,
    profile: &ProjectProfile,
    session: &DevSession,
    server_kind: &str,
    reason: &str,
    cleanup_complete: bool,
    human: &str,
) -> Result<()> {
    emit(
        json_output,
        json!({
            "protocol": "a3s.test.dev/1",
            "event": "stopped",
            "project": profile.id,
            "url": profile.dev_server.url.to_string(),
            "server": server_kind,
            "session": session.session,
            "reason": reason,
            "cleanup": if cleanup_complete { "complete" } else { "failed" },
        }),
        human,
    )
}

fn emit_repair_batch(json_output: bool, project: &str, batch: RepairBridgeBatch) -> Result<()> {
    let count = batch.repairs.len();
    let session = batch.session.clone();
    emit(
        json_output,
        RepairBridgeEvent::new(project, &batch),
        &format!("Received {count} submitted review finding(s) in session '{session}'"),
    )
}

fn emit<T: Serialize>(json_output: bool, event: T, human: &str) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string(&event)?);
    } else {
        println!("{human}");
    }
    Ok(())
}

enum Startup {
    Ready,
    Interrupted,
    Exited(ExitStatus),
    TimedOut,
}

enum Stop {
    Interrupted,
    ServerExited(ExitStatus),
    RepairBridgeFailed(anyhow::Error),
    ServerWaitFailed(anyhow::Error),
}
