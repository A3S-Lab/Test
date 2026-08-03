use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::DriverError;
use async_trait::async_trait;
use thiserror::Error;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

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
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    closed: bool,
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
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        #[cfg(unix)]
        command.process_group(0);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
        }

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
        let stdin = child.stdin.take().ok_or_else(|| {
            DriverError::new(
                "test.driver.gui.cua_unavailable",
                "CUA MCP proxy stdin is unavailable",
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            DriverError::new(
                "test.driver.gui.cua_unavailable",
                "CUA MCP proxy stdout is unavailable",
            )
        })?;
        let mut stderr = child.stderr.take().ok_or_else(|| {
            DriverError::new(
                "test.driver.gui.cua_unavailable",
                "CUA MCP proxy stderr is unavailable",
            )
        })?;
        tokio::spawn(async move {
            let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
        });

        Ok(Self {
            state: Mutex::new(StdioState {
                child,
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
        ensure_running(&mut state)?;

        let exchange = async {
            write_line(&mut state, &payload).await?;
            let response = read_bounded_line(&mut state.stdout).await?;
            serde_json::from_slice(&response).map_err(|error| {
                CuaTransportError::protocol(format!(
                    "CUA MCP proxy returned invalid JSON-RPC: {error}"
                ))
            })
        };
        match tokio::time::timeout(self.command_timeout, exchange).await {
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
        ensure_running(&mut state)?;
        match tokio::time::timeout(self.command_timeout, write_line(&mut state, &payload)).await {
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
        state.stdin.take();
        match tokio::time::timeout(EMERGENCY_CLOSE_TIMEOUT, state.child.wait()).await {
            Ok(Ok(_)) => {
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
        Ok(Some(status)) => {
            state.closed = true;
            state.stdin.take();
            Err(CuaTransportError::unavailable(format!(
                "CUA MCP proxy exited with {status}"
            )))
        }
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
    let _ = state.child.start_kill();
    let _ = tokio::time::timeout(EMERGENCY_CLOSE_TIMEOUT, state.child.wait()).await;
    state.closed = true;
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
