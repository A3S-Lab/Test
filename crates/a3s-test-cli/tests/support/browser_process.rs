use std::fs::File;
use std::io::{Read as _, Seek as _};
use std::process::{Command, Output, Stdio};

const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

pub fn bounded_output(command: &mut Command, context: &str) -> Output {
    let mut stdout = tempfile::tempfile()
        .unwrap_or_else(|error| panic!("{context}: prepare stdout capture: {error}"));
    let mut stderr = tempfile::tempfile()
        .unwrap_or_else(|error| panic!("{context}: prepare stderr capture: {error}"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().unwrap_or_else(|error| {
            panic!("{context}: clone stdout capture: {error}")
        })))
        .stderr(Stdio::from(stderr.try_clone().unwrap_or_else(|error| {
            panic!("{context}: clone stderr capture: {error}")
        })));
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("{context}: execute command: {error}"));
    Output {
        status,
        stdout: read_output(&mut stdout, context, "stdout"),
        stderr: read_output(&mut stderr, context, "stderr"),
    }
}

fn read_output(file: &mut File, context: &str, stream: &str) -> Vec<u8> {
    file.rewind()
        .unwrap_or_else(|error| panic!("{context}: rewind {stream}: {error}"));
    let mut output = Vec::new();
    file.take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut output)
        .unwrap_or_else(|error| panic!("{context}: read {stream}: {error}"));
    assert!(
        output.len() as u64 <= MAX_OUTPUT_BYTES,
        "{context}: {stream} exceeded {MAX_OUTPUT_BYTES} bytes"
    );
    output
}
