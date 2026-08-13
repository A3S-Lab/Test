use std::process::ExitCode;

use a3s_test_cli::{execute, Cli};
use clap::Parser;

const CLI_WORKER_STACK_SIZE: usize = 8 * 1024 * 1024;

fn main() -> ExitCode {
    match std::thread::Builder::new()
        .name("a3s-test-cli".to_string())
        .stack_size(CLI_WORKER_STACK_SIZE)
        .spawn(run_cli)
    {
        Ok(worker) => match worker.join() {
            Ok(code) => code,
            Err(_) => startup_error("async command worker panicked".to_string()),
        },
        Err(error) => startup_error(format!("could not start command worker: {error}")),
    }
}

fn run_cli() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(CLI_WORKER_STACK_SIZE)
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return startup_error(format!("could not start async runtime: {error}")),
    };

    match runtime.block_on(execute(Cli::parse())) {
        Ok(code) => code,
        Err(error) => command_error(error),
    }
}

fn command_error(error: anyhow::Error) -> ExitCode {
    eprintln!("a3s-test: {error:#}");
    ExitCode::from(2)
}

fn startup_error(message: String) -> ExitCode {
    eprintln!("a3s-test: {message}");
    ExitCode::from(2)
}
