use std::ffi::OsString;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::DriverError;
use async_trait::async_trait;
use thiserror::Error;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::process::{configure_owned_process, terminate_unattached_child, OwnedProcessTree};
use crate::{CuaEndpoint, GuiDriverConfig, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

const MAX_REQUEST_BYTES: usize = 8 * 1_024 * 1_024;
const MAX_RESPONSE_BYTES: usize = 64 * 1_024 * 1_024;
const EMERGENCY_CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CuaTransportErrorKind {
    Unavailable,
    TimedOut,
    Protocol,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct CuaTransportError {
    kind: CuaTransportErrorKind,
    message: String,
}

impl CuaTransportError {
    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: CuaTransportErrorKind::Unavailable,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn timed_out(message: impl Into<String>) -> Self {
        Self {
            kind: CuaTransportErrorKind::TimedOut,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn protocol(message: impl Into<String>) -> Self {
        Self {
            kind: CuaTransportErrorKind::Protocol,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> CuaTransportErrorKind {
        self.kind
    }
}

#[async_trait]
pub trait CuaTransport: Send + Sync {
    async fn request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, CuaTransportError>;

    async fn notify(&self, notification: JsonRpcNotification) -> Result<(), CuaTransportError>;

    async fn close(&self) -> Result<(), CuaTransportError> {
        Ok(())
    }
}

#[async_trait]
pub trait CuaTransportFactory: Send + Sync {
    async fn connect(&self, config: &GuiDriverConfig)
        -> Result<Arc<dyn CuaTransport>, DriverError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StdioCuaTransportFactory;

#[async_trait]
impl CuaTransportFactory for StdioCuaTransportFactory {
    async fn connect(
        &self,
        config: &GuiDriverConfig,
    ) -> Result<Arc<dyn CuaTransport>, DriverError> {
        Ok(Arc::new(StdioCuaTransport::spawn(config).await?))
    }
}

pub struct StdioCuaTransport {
    state: Mutex<StdioState>,
    command_timeout: Duration,
}

struct StdioState {
    child: Child,
    process_tree: Option<OwnedProcessTree>,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    closed: bool,
}

struct InFlightGuard<'a> {
    state: &'a mut StdioState,
    armed: bool,
}

impl<'a> InFlightGuard<'a> {
    fn new(state: &'a mut StdioState) -> Self {
        Self { state, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Deref for InFlightGuard<'_> {
    type Target = StdioState;

    fn deref(&self) -> &Self::Target {
        self.state
    }
}

impl DerefMut for InFlightGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.state
    }
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            abort_now(self.state);
        }
    }
}

impl Drop for StdioState {
    fn drop(&mut self) {
        abort_now(self);
    }
}

impl StdioCuaTransport {
    pub async fn spawn(config: &GuiDriverConfig) -> Result<Self, DriverError> {
        config.validate()?;
        let policy_metadata = tokio::fs::metadata(&config.policy_file)
            .await
            .map_err(|error| {
                DriverError::new(
                    "test.driver.gui.policy_unavailable",
                    format!(
                        "failed to inspect CUA policy file {}: {error}",
                        config.policy_file.display()
                    ),
                )
            })?;
        if !policy_metadata.is_file() {
            return Err(DriverError::new(
                "test.driver.gui.policy_unavailable",
                format!(
                    "CUA policy path {} is not a regular file",
                    config.policy_file.display()
                ),
            ));
        }

        let (program, arguments) = proxy_command(&config.endpoint)?;
        let mut command = Command::new(&program);
        command
            .args(arguments)
            .env("CUA_DRIVER_POLICY_FILE", &config.policy_file)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        configure_owned_process(&mut command);

        let mut child = command.spawn().map_err(|error| {
            DriverError::new(
                "test.driver.gui.cua_unavailable",
                format!(
                    "failed to start CUA MCP proxy {}: {error}",
                    program.display()
                ),
            )
            .with_retryable(true)
        })?;
        let process_tree = match OwnedProcessTree::attach(&child) {
            Ok(process_tree) => process_tree,
            Err(error) => {
                terminate_unattached_child(&mut child, EMERGENCY_CLOSE_TIMEOUT).await;
                return Err(DriverError::new(
                    "test.driver.gui.process_supervision_unavailable",
                    format!(
                        "failed to bind CUA MCP proxy {} to an owned process tree: {error}",
                        program.display()
                    ),
                ));
            }
        };
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(mut stderr)) = pipes else {
            terminate_attached_child(&mut child, process_tree).await;
            return Err(DriverError::new(
                "test.driver.gui.cua_unavailable",
                "CUA MCP proxy stdio pipes are unavailable",
            ));
        };
        tokio::spawn(async move {
            let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
        });

        Ok(Self {
            state: Mutex::new(StdioState {
                child,
                process_tree: Some(process_tree),
                stdin: Some(stdin),
                stdout: BufReader::new(stdout),
                closed: false,
            }),
            command_timeout: config.command_timeout,
        })
    }
}

#[async_trait]
impl CuaTransport for StdioCuaTransport {
    async fn request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, CuaTransportError> {
        let payload = serialize_line(&request)?;
        let mut state = self.state.lock().await;
        if let Err(error) = ensure_running(&mut state) {
            if !state.closed {
                terminate(&mut state).await;
            }
            return Err(error);
        }

        let result = {
            let mut in_flight = InFlightGuard::new(&mut state);
            let exchange = async {
                write_line(&mut in_flight, &payload).await?;
                let response = read_bounded_line(&mut in_flight.stdout).await?;
                serde_json::from_slice(&response).map_err(|error| {
                    CuaTransportError::protocol(format!(
                        "CUA MCP proxy returned invalid JSON-RPC: {error}"
                    ))
                })
            };
            let result = tokio::time::timeout(self.command_timeout, exchange).await;
            in_flight.disarm();
            result
        };
        match result {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => {
                terminate(&mut state).await;
                Err(error)
            }
            Err(_) => {
                terminate(&mut state).await;
                Err(CuaTransportError::timed_out(format!(
                    "CUA MCP request exceeded {} ms",
                    self.command_timeout.as_millis()
                )))
            }
        }
    }

    async fn notify(&self, notification: JsonRpcNotification) -> Result<(), CuaTransportError> {
        let payload = serialize_line(&notification)?;
        let mut state = self.state.lock().await;
        if let Err(error) = ensure_running(&mut state) {
            if !state.closed {
                terminate(&mut state).await;
            }
            return Err(error);
        }
        let result = {
            let mut in_flight = InFlightGuard::new(&mut state);
            let result =
                tokio::time::timeout(self.command_timeout, write_line(&mut in_flight, &payload))
                    .await;
            in_flight.disarm();
            result
        };
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                terminate(&mut state).await;
                Err(error)
            }
            Err(_) => {
                terminate(&mut state).await;
                Err(CuaTransportError::timed_out(format!(
                    "CUA MCP notification exceeded {} ms",
                    self.command_timeout.as_millis()
                )))
            }
        }
    }

    async fn close(&self) -> Result<(), CuaTransportError> {
        let mut state = self.state.lock().await;
        if state.closed {
            return Ok(());
        }
        let result = {
            let mut in_flight = InFlightGuard::new(&mut state);
            in_flight.stdin.take();
            let result =
                tokio::time::timeout(EMERGENCY_CLOSE_TIMEOUT, in_flight.child.wait()).await;
            in_flight.disarm();
            result
        };
        match result {
            Ok(Ok(_)) => {
                if let Err(error) = terminate_process_tree_and_wait(&mut state).await {
                    state.closed = true;
                    return Err(CuaTransportError::protocol(format!(
                        "failed to finish CUA MCP proxy process cleanup: {error}"
                    )));
                }
                state.closed = true;
                Ok(())
            }
            Ok(Err(error)) => {
                terminate(&mut state).await;
                Err(CuaTransportError::protocol(format!(
                    "failed to wait for CUA MCP proxy: {error}"
                )))
            }
            Err(_) => {
                terminate(&mut state).await;
                Err(CuaTransportError::timed_out(
                    "CUA MCP proxy did not exit after stdin closed",
                ))
            }
        }
    }
}

fn proxy_command(endpoint: &CuaEndpoint) -> Result<(PathBuf, Vec<OsString>), DriverError> {
    match endpoint {
        CuaEndpoint::InstalledDaemon { proxy_executable } => {
            Ok((proxy_executable.clone(), vec![OsString::from("mcp")]))
        }
        CuaEndpoint::EmbeddedSocket {
            proxy_executable,
            socket,
        } => Ok((
            proxy_executable.clone(),
            vec![
                OsString::from("mcp"),
                OsString::from("--embedded"),
                OsString::from("--socket"),
                socket.as_os_str().to_owned(),
            ],
        )),
    }
}

fn serialize_line(value: &impl serde::Serialize) -> Result<Vec<u8>, CuaTransportError> {
    let mut payload = serde_json::to_vec(value).map_err(|error| {
        CuaTransportError::protocol(format!("failed to serialize CUA JSON-RPC message: {error}"))
    })?;
    if payload.len() > MAX_REQUEST_BYTES {
        return Err(CuaTransportError::protocol(format!(
            "CUA JSON-RPC request exceeds {MAX_REQUEST_BYTES} bytes"
        )));
    }
    payload.push(b'\n');
    Ok(payload)
}

fn ensure_running(state: &mut StdioState) -> Result<(), CuaTransportError> {
    if state.closed || state.stdin.is_none() {
        return Err(CuaTransportError::unavailable(
            "CUA MCP proxy is already closed",
        ));
    }
    match state.child.try_wait() {
        Ok(None) => Ok(()),
        Ok(Some(status)) => Err(CuaTransportError::unavailable(format!(
            "CUA MCP proxy exited with {status}"
        ))),
        Err(error) => Err(CuaTransportError::protocol(format!(
            "failed to inspect CUA MCP proxy: {error}"
        ))),
    }
}

async fn write_line(state: &mut StdioState, payload: &[u8]) -> Result<(), CuaTransportError> {
    let stdin = state
        .stdin
        .as_mut()
        .ok_or_else(|| CuaTransportError::unavailable("CUA MCP proxy stdin is closed"))?;
    stdin.write_all(payload).await.map_err(|error| {
        CuaTransportError::protocol(format!("failed to write CUA MCP request: {error}"))
    })?;
    stdin.flush().await.map_err(|error| {
        CuaTransportError::protocol(format!("failed to flush CUA MCP request: {error}"))
    })
}

async fn read_bounded_line<R>(reader: &mut R) -> Result<Vec<u8>, CuaTransportError>
where
    R: AsyncBufRead + Unpin,
{
    let mut response = Vec::new();
    let mut limited = reader.take((MAX_RESPONSE_BYTES + 1) as u64);
    let count = limited
        .read_until(b'\n', &mut response)
        .await
        .map_err(|error| {
            CuaTransportError::protocol(format!("failed to read CUA MCP response: {error}"))
        })?;
    if count == 0 {
        return Err(CuaTransportError::unavailable(
            "CUA MCP proxy closed stdout before responding",
        ));
    }
    if response.len() > MAX_RESPONSE_BYTES || response.last() != Some(&b'\n') {
        return Err(CuaTransportError::protocol(format!(
            "CUA MCP response exceeds {MAX_RESPONSE_BYTES} bytes or is not line-delimited"
        )));
    }
    Ok(response)
}

async fn terminate(state: &mut StdioState) {
    state.stdin.take();
    let process_tree = state.process_tree.take();
    if let Some(process_tree) = &process_tree {
        process_tree.terminate_now();
    }
    let _ = state.child.start_kill();
    let _ = tokio::time::timeout(EMERGENCY_CLOSE_TIMEOUT, state.child.wait()).await;
    if let Some(process_tree) = process_tree {
        let _ = wait_for_process_tree(process_tree).await;
    }
    state.closed = true;
}

async fn terminate_process_tree_and_wait(state: &mut StdioState) -> std::io::Result<()> {
    match state.process_tree.take() {
        Some(process_tree) => wait_for_process_tree(process_tree).await,
        None => Ok(()),
    }
}

async fn wait_for_process_tree(process_tree: OwnedProcessTree) -> std::io::Result<()> {
    tokio::task::spawn_blocking(move || process_tree.terminate_and_wait(EMERGENCY_CLOSE_TIMEOUT))
        .await
        .map_err(|error| std::io::Error::other(format!("process cleanup task failed: {error}")))?
}

async fn terminate_attached_child(child: &mut Child, process_tree: OwnedProcessTree) {
    process_tree.terminate_now();
    let _ = child.start_kill();
    let _ = tokio::time::timeout(EMERGENCY_CLOSE_TIMEOUT, child.wait()).await;
    let _ = wait_for_process_tree(process_tree).await;
}

fn abort_now(state: &mut StdioState) {
    state.stdin.take();
    let process_tree = state.process_tree.take();
    if let Some(process_tree) = &process_tree {
        process_tree.terminate_now();
    }
    let _ = state.child.start_kill();
    reap_child(&mut state.child, EMERGENCY_CLOSE_TIMEOUT);
    if let Some(process_tree) = process_tree {
        let _ = process_tree.terminate_and_wait(EMERGENCY_CLOSE_TIMEOUT);
    }
    state.closed = true;
}

fn reap_child(child: &mut Child, timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn builds_only_supported_proxy_commands() {
        let (program, arguments) = proxy_command(&CuaEndpoint::InstalledDaemon {
            proxy_executable: PathBuf::from("cua-driver"),
        })
        .expect("installed endpoint");
        assert_eq!(program, PathBuf::from("cua-driver"));
        assert_eq!(arguments, [OsString::from("mcp")]);

        let (_, embedded) = proxy_command(&CuaEndpoint::EmbeddedSocket {
            proxy_executable: PathBuf::from("cua-driver"),
            socket: PathBuf::from("private.sock"),
        })
        .expect("embedded endpoint");
        assert_eq!(
            embedded,
            [
                OsString::from("mcp"),
                OsString::from("--embedded"),
                OsString::from("--socket"),
                OsString::from("private.sock"),
            ]
        );
    }

    #[tokio::test]
    async fn reads_one_line_without_consuming_the_next_response() {
        let (client, mut server) = tokio::io::duplex(128);
        tokio::spawn(async move {
            server
                .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1}\nsecond\n")
                .await
                .expect("write fixture");
        });
        let mut reader = BufReader::new(client);

        let first = read_bounded_line(&mut reader).await.expect("first line");
        let second = read_bounded_line(&mut reader).await.expect("second line");

        assert_eq!(first, b"{\"jsonrpc\":\"2.0\",\"id\":1}\n");
        assert_eq!(second, b"second\n");
    }

    #[tokio::test]
    async fn cancelling_an_in_flight_request_terminates_the_proxy_immediately() {
        let (transport, process_id) = hanging_transport().await;
        let request = tokio::spawn({
            let transport = Arc::clone(&transport);
            async move {
                transport
                    .request(JsonRpcRequest::new(1, "fixture/hang", None))
                    .await
            }
        });
        wait_until_request_holds_transport(&transport).await;

        request.abort();
        assert!(request
            .await
            .expect_err("request cancellation")
            .is_cancelled());
        let terminated = wait_until_child_exits(&transport, Duration::from_secs(1)).await;
        if !terminated {
            let _ = transport.close().await;
        }

        assert!(
            terminated,
            "cancelled request left CUA proxy {process_id} running"
        );
    }

    async fn hanging_transport() -> (Arc<StdioCuaTransport>, u32) {
        let mut command = hanging_command();
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        configure_owned_process(&mut command);
        let mut child = command.spawn().expect("spawn hanging CUA proxy");
        let process_id = child.id().expect("hanging proxy PID");
        let process_tree = OwnedProcessTree::attach(&child).expect("contain hanging CUA proxy");
        let stdin = child.stdin.take().expect("hanging proxy stdin");
        let stdout = child.stdout.take().expect("hanging proxy stdout");
        (
            Arc::new(StdioCuaTransport {
                state: Mutex::new(StdioState {
                    child,
                    process_tree: Some(process_tree),
                    stdin: Some(stdin),
                    stdout: BufReader::new(stdout),
                    closed: false,
                }),
                command_timeout: Duration::from_secs(30),
            }),
            process_id,
        )
    }

    #[cfg(unix)]
    fn hanging_command() -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30"]);
        command
    }

    #[cfg(windows)]
    fn hanging_command() -> Command {
        let system_root = PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"));
        let mut command = Command::new(system_root.join("System32").join("cmd.exe"));
        command.args(["/D", "/S", "/C", "ping -n 30 127.0.0.1 >NUL"]);
        command
    }

    #[cfg(not(any(unix, windows)))]
    fn hanging_command() -> Command {
        let mut command = Command::new("sleep");
        command.arg("30");
        command
    }

    async fn wait_until_request_holds_transport(transport: &StdioCuaTransport) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if transport.state.try_lock().is_err() {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "request never acquired the CUA transport"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn wait_until_child_exits(transport: &StdioCuaTransport, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let exited = transport
                .state
                .lock()
                .await
                .child
                .try_wait()
                .is_ok_and(|status| status.is_some());
            if exited {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
