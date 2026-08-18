use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, Evidence, Expectation, ScenarioContext, StepOutput,
    Surface, SurfaceDriver, SurfaceObservation, TestStep, WaitCondition,
};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::Notify;

use crate::artifact::{prepare_root, write_recording};
use crate::config::validate_terminal_budget;
use crate::input::{key_bytes, paste_bytes};
use crate::process::{
    register_process, ProcessRegistration, ProcessStatus, PtyProcess, SharedProcess,
};
use crate::terminal::{ProcessExit, TerminalState};
use crate::{process, TuiDriverConfig, TuiSize};

const READ_CHUNK_BYTES: usize = 16 * 1024;
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_WAIT_PATTERN_BYTES: usize = 4_096;

pub struct TuiDriver {
    config: TuiDriverConfig,
}

impl TuiDriver {
    #[must_use]
    pub fn new(config: TuiDriverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SurfaceDriver for TuiDriver {
    fn surface(&self) -> Surface {
        Surface::Tui
    }

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError> {
        self.config.validate()?;
        validate_context_component(&context.run_id, "run id")?;
        validate_context_component(&context.scenario_id, "scenario id")?;
        let artifacts_dir = prepare_root(&context.artifacts_dir).await?;
        let spawned = process::spawn(self.config.command.clone(), self.config.initial_size)
            .await
            .map_err(|error| {
                DriverError::new(
                    "test.driver.tui.spawn_failed",
                    format!("failed to create owned PTY process: {error}"),
                )
            })?;
        let process_id = spawned.process.process_id();
        let process = Arc::new(Mutex::new(spawned.process));
        let registration = register_process(process_id, &process).map_err(|error| {
            DriverError::new(
                "test.driver.tui.process_containment_failed",
                format!("failed to register owned TUI process: {error}"),
            )
        })?;
        let state = Arc::new(Mutex::new(TerminalState::new(
            self.config.initial_size,
            self.config.scrollback_rows,
            self.config.max_output_bytes,
        )));
        let output_notify = Arc::new(Notify::new());
        let reader_task = spawn_reader(
            spawned.reader,
            Arc::clone(&state),
            Arc::clone(&output_notify),
        );

        Ok(Box::new(TuiSession {
            process: Some(process),
            process_id,
            registration: Some(registration),
            writer: Some(Arc::new(Mutex::new(spawned.writer))),
            state,
            output_notify,
            reader_task: Some(reader_task),
            artifacts_dir,
            scrollback_rows: self.config.scrollback_rows,
            command_timeout: self.config.command_timeout,
            cleanup_timeout: self.config.cleanup_timeout,
            closed: false,
        }))
    }
}

pub struct TuiSession {
    process: Option<SharedProcess>,
    process_id: u32,
    registration: Option<ProcessRegistration>,
    writer: Option<Arc<Mutex<Box<dyn std::io::Write + Send>>>>,
    state: Arc<Mutex<TerminalState>>,
    output_notify: Arc<Notify>,
    reader_task: Option<tokio::task::JoinHandle<Result<(), DriverError>>>,
    artifacts_dir: PathBuf,
    scrollback_rows: usize,
    command_timeout: Duration,
    cleanup_timeout: Duration,
    closed: bool,
}

#[async_trait]
impl DriverSession for TuiSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        self.ensure_open()?;
        self.refresh_process_status().await?;
        self.observation("terminal viewport captured")
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.ensure_open()?;
        self.refresh_process_status().await?;
        match &step.action {
            Action::Snapshot { .. } => self.step_observation("terminal viewport captured"),
            Action::Press { key } => {
                let application_cursor = self.with_state(|state| state.application_cursor())?;
                let bytes = key_bytes(key, application_cursor)?;
                self.write_input(bytes).await?;
                Ok(StepOutput::new("terminal key pressed")
                    .with_data(json!({ "surface": "tui", "key": key })))
            }
            Action::TerminalPaste { text } => {
                let bracketed = self.with_state(|state| state.bracketed_paste())?;
                let bytes = paste_bytes(text, bracketed)?;
                self.write_input(bytes).await?;
                Ok(StepOutput::new("text pasted into terminal").with_data(json!({
                    "surface": "tui",
                    "bytes": text.len(),
                    "bracketed": bracketed,
                })))
            }
            Action::Type {
                target: _,
                value: _,
            } => Err(unsupported(
                "TUI text input uses terminal_paste so browser-style targets cannot be confused with terminal focus",
            )),
            Action::TerminalResize { columns, rows } => {
                self.resize(TuiSize::new(*columns, *rows)?).await
            }
            Action::Viewport { width, height, scale } => {
                if scale.is_some() {
                    return Err(unsupported("terminal resize does not admit a display scale"));
                }
                let columns = u16::try_from(*width).map_err(|_| {
                    DriverError::new(
                        "test.driver.tui.resize_invalid",
                        "terminal columns exceed the supported range",
                    )
                })?;
                let rows = u16::try_from(*height).map_err(|_| {
                    DriverError::new(
                        "test.driver.tui.resize_invalid",
                        "terminal rows exceed the supported range",
                    )
                })?;
                self.resize(TuiSize::new(columns, rows)?).await
            }
            Action::Wait { condition } => self.wait(condition).await,
            Action::Assert { expectation } => self.assert(expectation),
            Action::TerminalRecording { path } => self.recording(path).await,
            Action::VerifyContract { .. } => Err(DriverError::new(
                "test.driver.tui.runner_action_unsupported",
                "verify_contract is executed by the A3S Test runner and must not reach a surface driver",
            )),
            _ => Err(unsupported(
                "this action is not implemented by the terminal surface",
            )),
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        if self.closed {
            return Ok(());
        }
        self.writer.take();
        let timeout = self.cleanup_timeout;
        let cleanup = if let Some(process) = self.process.as_ref().cloned() {
            let cleanup_task = tokio::task::spawn_blocking(move || {
                let mut process = process
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                terminate_and_wait(process.as_mut(), timeout)
            });
            match tokio::time::timeout(timeout, cleanup_task).await {
                Ok(Ok(Ok(_))) => Ok(()),
                Ok(Ok(Err(error))) => Err(DriverError::new(
                    "test.driver.tui.cleanup_failed",
                    format!("failed to terminate TUI process tree: {error}"),
                )),
                Ok(Err(error)) => Err(DriverError::new(
                    "test.driver.tui.cleanup_failed",
                    format!("failed to join TUI cleanup: {error}"),
                )),
                Err(_) => {
                    process::emergency_terminate_process(self.process_id);
                    Err(DriverError::new(
                        "test.driver.tui.cleanup_failed",
                        "terminal cleanup exceeded its deadline",
                    ))
                }
            }
        } else {
            Ok(())
        };
        self.process.take();
        self.registration.take();
        let output = if let Some(mut reader_task) = self.reader_task.take() {
            match tokio::time::timeout(timeout, &mut reader_task).await {
                Ok(Ok(Ok(()))) => {}
                Ok(Ok(Err(error))) => return self.finish_close(cleanup, Err(error)),
                Ok(Err(error)) => {
                    return self.finish_close(
                        cleanup,
                        Err(DriverError::new(
                            "test.driver.tui.output_failed",
                            format!("terminal output task failed: {error}"),
                        )),
                    );
                }
                Err(_) => {
                    reader_task.abort();
                    return self.finish_close(
                        cleanup,
                        Err(DriverError::new(
                            "test.driver.tui.cleanup_failed",
                            "terminal output task did not stop before the cleanup deadline",
                        )),
                    );
                }
            }
            Ok(())
        } else {
            Ok(())
        };
        self.finish_close(cleanup, output)
    }
}

impl TuiSession {
    fn ensure_open(&self) -> Result<(), DriverError> {
        if self.closed {
            Err(DriverError::new(
                "test.driver.tui.session_closed",
                "terminal session is already closed",
            ))
        } else {
            Ok(())
        }
    }

    fn observation(&self, summary: &str) -> Result<SurfaceObservation, DriverError> {
        let data = self.with_state_mut(TerminalState::data_with_history)?;
        Ok(SurfaceObservation::new(summary).with_data(data))
    }

    fn step_observation(&self, summary: &str) -> Result<StepOutput, DriverError> {
        let data = self.with_state_mut(TerminalState::data_with_history)?;
        Ok(StepOutput::new(summary).with_data(data))
    }

    fn with_state<T>(&self, operation: impl FnOnce(&TerminalState) -> T) -> Result<T, DriverError> {
        self.state
            .lock()
            .map(|state| operation(&state))
            .map_err(|_| {
                DriverError::new(
                    "test.driver.tui.state_unavailable",
                    "terminal state is unavailable",
                )
            })
    }

    fn with_state_mut<T>(
        &self,
        operation: impl FnOnce(&mut TerminalState) -> T,
    ) -> Result<T, DriverError> {
        self.state
            .lock()
            .map(|mut state| operation(&mut state))
            .map_err(|_| {
                DriverError::new(
                    "test.driver.tui.state_unavailable",
                    "terminal state is unavailable",
                )
            })
    }

    async fn write_input(&self, bytes: Vec<u8>) -> Result<(), DriverError> {
        let writer = self.writer.as_ref().cloned().ok_or_else(|| {
            DriverError::new(
                "test.driver.tui.input_closed",
                "terminal input is already closed",
            )
        })?;
        let timeout = self.command_timeout;
        tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                let mut writer = writer
                    .lock()
                    .map_err(|_| std::io::Error::other("terminal writer is unavailable"))?;
                writer.write_all(&bytes)?;
                writer.flush()
            }),
        )
        .await
        .map_err(|_| {
            DriverError::new(
                "test.driver.tui.input_timeout",
                "terminal input exceeded its deadline",
            )
        })?
        .map_err(|error| {
            DriverError::new(
                "test.driver.tui.input_failed",
                format!("terminal input task failed: {error}"),
            )
        })?
        .map_err(|error| {
            DriverError::new(
                "test.driver.tui.input_failed",
                format!("failed to write terminal input: {error}"),
            )
        })
    }

    async fn resize(&mut self, size: TuiSize) -> Result<StepOutput, DriverError> {
        validate_terminal_budget(size, self.scrollback_rows)?;
        let Some(process) = self.process.as_ref().cloned() else {
            return Err(DriverError::new(
                "test.driver.tui.process_exited",
                "terminal process is no longer available",
            ));
        };
        let state = Arc::clone(&self.state);
        let timeout = self.command_timeout;
        tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                process
                    .lock()
                    .map_err(|_| std::io::Error::other("terminal process is unavailable"))?
                    .resize(size)?;
                state
                    .lock()
                    .map_err(|_| std::io::Error::other("terminal state is unavailable"))?
                    .resize(size);
                Ok::<(), std::io::Error>(())
            }),
        )
        .await
        .map_err(|_| {
            DriverError::new(
                "test.driver.tui.resize_timeout",
                "terminal resize exceeded its deadline",
            )
        })?
        .map_err(|error| {
            DriverError::new(
                "test.driver.tui.resize_failed",
                format!("terminal resize task failed: {error}"),
            )
        })?
        .map_err(|error| {
            DriverError::new(
                "test.driver.tui.resize_failed",
                format!("failed to resize terminal: {error}"),
            )
        })?;
        Ok(StepOutput::new("terminal resized").with_data(json!({
            "surface": "tui",
            "columns": size.columns,
            "rows": size.rows,
        })))
    }

    async fn wait(&mut self, condition: &WaitCondition) -> Result<StepOutput, DriverError> {
        let deadline = tokio::time::Instant::now() + self.command_timeout;
        loop {
            let output_notify = Arc::clone(&self.output_notify);
            let notified = output_notify.notified();
            self.refresh_process_status().await?;
            if self.wait_condition_matches(condition)? {
                return Ok(StepOutput::new("terminal wait condition matched")
                    .with_data(self.with_state_mut(TerminalState::data_with_history)?));
            }
            if self.process_has_exited()? && self.reader_task_finished() {
                self.join_reader_task().await?;
                if self.wait_condition_matches(condition)? {
                    return Ok(StepOutput::new("terminal wait condition matched")
                        .with_data(self.with_state_mut(TerminalState::data_with_history)?));
                }
                return Err(DriverError::new(
                    "test.driver.tui.wait_process_exited",
                    "terminal process exited before the wait condition matched",
                ));
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return Err(DriverError::new(
                    "test.driver.tui.wait_timeout",
                    "terminal wait condition exceeded its deadline",
                ));
            }
            tokio::time::sleep(WAIT_POLL_INTERVAL).await;
        }
    }

    fn assert(&mut self, expectation: &Expectation) -> Result<StepOutput, DriverError> {
        match expectation {
            Expectation::TextVisible(text) => {
                validate_wait_pattern(text)?;
                if !self.with_state_mut(|state| state.contains_text(text))? {
                    return Err(DriverError::new(
                        "test.assert.text_visible",
                        format!("expected terminal text {text:?} is not visible"),
                    ));
                }
                Ok(StepOutput::new("terminal text is visible")
                    .with_data(json!({ "text": text, "visible": true })))
            }
            Expectation::Url(_)
            | Expectation::Visible(_)
            | Expectation::InViewport(_)
            | Expectation::ViewportCoverage { .. }
            | Expectation::PointerReachable(_)
            | Expectation::RenderedText { .. }
            | Expectation::RenderedTexts { .. }
            | Expectation::VisibleCount { .. }
            | Expectation::State { .. }
            | Expectation::Value { .. }
            | Expectation::SelectedValues { .. }
            | Expectation::Layout { .. } => {
                Err(unsupported("terminal assertions support visible text only"))
            }
        }
    }

    async fn recording(&self, requested: &str) -> Result<StepOutput, DriverError> {
        let bytes = self.with_state(TerminalState::recording)??;
        let path = write_recording(&self.artifacts_dir, requested, &bytes).await?;
        Ok(StepOutput::new("terminal recording captured")
            .with_data(json!({ "bytes": bytes.len() }))
            .with_evidence(Evidence {
                name: requested.to_string(),
                path: path.display().to_string(),
                media_type: "application/vnd.a3s.terminal-vt".to_string(),
            }))
    }

    async fn refresh_process_status(&mut self) -> Result<(), DriverError> {
        let Some(process) = self.process.as_ref().cloned() else {
            return Ok(());
        };
        let result = tokio::time::timeout(
            self.command_timeout,
            tokio::task::spawn_blocking(move || {
                process
                    .lock()
                    .map_err(|_| std::io::Error::other("terminal process is unavailable"))?
                    .try_wait()
            }),
        )
        .await
        .map_err(|_| {
            DriverError::new(
                "test.driver.tui.wait_timeout",
                "terminal status probe exceeded its deadline",
            )
        })?
        .map_err(|error| {
            DriverError::new(
                "test.driver.tui.wait_failed",
                format!("failed to join terminal status probe: {error}"),
            )
        })?;
        if let Some(status) = result.map_err(|error| {
            DriverError::new(
                "test.driver.tui.wait_failed",
                format!("failed to inspect terminal process: {error}"),
            )
        })? {
            self.install_exit(status)?;
        }
        Ok(())
    }

    fn install_exit(&self, status: ProcessStatus) -> Result<(), DriverError> {
        self.state
            .lock()
            .map_err(|_| {
                DriverError::new(
                    "test.driver.tui.state_unavailable",
                    "terminal state is unavailable",
                )
            })?
            .set_exit(ProcessExit {
                code: status.code,
                signal: status.signal,
            });
        self.output_notify.notify_one();
        Ok(())
    }

    fn process_has_exited(&self) -> Result<bool, DriverError> {
        let data = self.with_state(TerminalState::data)?;
        Ok(data["process"]["running"] == false)
    }

    fn wait_condition_matches(&self, condition: &WaitCondition) -> Result<bool, DriverError> {
        match condition {
            WaitCondition::Text(text) => {
                validate_wait_pattern(text)?;
                self.with_state_mut(|state| state.contains_text(text))
            }
            WaitCondition::Regex(pattern) => {
                validate_wait_pattern(pattern)?;
                let regex = regex::Regex::new(pattern).map_err(|error| {
                    DriverError::new(
                        "test.driver.tui.regex_invalid",
                        format!("terminal wait regex is invalid: {error}"),
                    )
                })?;
                let contents = self.with_state_mut(TerminalState::contents)?;
                Ok(regex.is_match(&contents))
            }
            WaitCondition::Load(_) | WaitCondition::Url(_) | WaitCondition::Visible(_) => Err(
                unsupported("terminal waits support only exact text or regular expressions"),
            ),
        }
    }

    fn reader_task_finished(&self) -> bool {
        self.reader_task
            .as_ref()
            .is_none_or(tokio::task::JoinHandle::is_finished)
    }

    async fn join_reader_task(&mut self) -> Result<(), DriverError> {
        let Some(reader_task) = self.reader_task.take() else {
            return Ok(());
        };
        reader_task.await.map_err(|error| {
            DriverError::new(
                "test.driver.tui.output_failed",
                format!("terminal output task failed: {error}"),
            )
        })?
    }

    fn finish_close(
        &mut self,
        cleanup: Result<(), DriverError>,
        output: Result<(), DriverError>,
    ) -> Result<(), DriverError> {
        self.closed = true;
        cleanup.and(output)
    }
}

impl Drop for TuiSession {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.writer.take();
        if let Some(process) = self.process.take() {
            match process.try_lock() {
                Ok(mut process) => {
                    if process.terminate().is_err() {
                        process::emergency_terminate_process(self.process_id);
                    }
                }
                Err(TryLockError::Poisoned(error)) => {
                    if error.into_inner().terminate().is_err() {
                        process::emergency_terminate_process(self.process_id);
                    }
                }
                Err(TryLockError::WouldBlock) => {
                    process::emergency_terminate_process(self.process_id);
                }
            }
        }
        self.registration.take();
        if let Some(reader_task) = self.reader_task.take() {
            reader_task.abort();
        }
    }
}

fn spawn_reader(
    mut reader: Box<dyn std::io::Read + Send>,
    state: Arc<Mutex<TerminalState>>,
    notify: Arc<Notify>,
) -> tokio::task::JoinHandle<Result<(), DriverError>> {
    tokio::task::spawn_blocking(move || {
        let mut buffer = vec![0_u8; READ_CHUNK_BYTES];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    notify.notify_one();
                    return Ok(());
                }
                Ok(read) => {
                    state
                        .lock()
                        .map_err(|_| {
                            DriverError::new(
                                "test.driver.tui.state_unavailable",
                                "terminal state is unavailable",
                            )
                        })?
                        .process(&buffer[..read]);
                    notify.notify_one();
                }
                #[cfg(unix)]
                Err(error) if error.raw_os_error() == Some(libc_eio()) => {
                    notify.notify_one();
                    return Ok(());
                }
                Err(error) => {
                    return Err(DriverError::new(
                        "test.driver.tui.output_failed",
                        format!("failed to read terminal output: {error}"),
                    ));
                }
            }
        }
    })
}

fn terminate_and_wait(
    process: &mut dyn PtyProcess,
    timeout: Duration,
) -> std::io::Result<ProcessStatus> {
    let _ = process.try_wait()?;
    process.terminate()?;
    process.wait_for_exit(timeout)
}

fn validate_wait_pattern(value: &str) -> Result<(), DriverError> {
    if value.is_empty() || value.len() > MAX_WAIT_PATTERN_BYTES {
        return Err(DriverError::new(
            "test.driver.tui.wait_invalid",
            "terminal wait pattern must contain 1 to 4096 bytes",
        ));
    }
    Ok(())
}

fn validate_context_component(value: &str, field: &str) -> Result<(), DriverError> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(DriverError::new(
            "test.driver.tui.session_name_invalid",
            format!("{field} must contain only ASCII letters, digits, '-' or '_'"),
        ));
    }
    Ok(())
}

fn unsupported(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.tui.action_unsupported", message)
}

#[cfg(unix)]
const fn libc_eio() -> i32 {
    5
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
