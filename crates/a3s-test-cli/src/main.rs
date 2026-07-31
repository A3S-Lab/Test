use std::process::ExitCode;

use a3s_test_cli::{execute, Cli};
use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    match execute(Cli::parse()).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("a3s-test: {error:#}");
            ExitCode::from(2)
        }
    }
}
