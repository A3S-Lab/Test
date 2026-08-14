use std::collections::VecDeque;

use a3s_test_core::DriverError;
use serde_json::{json, Value};

use crate::TuiSize;

const MAX_OBSERVATION_TEXT_BYTES: usize = 256 * 1024;

pub(crate) struct TerminalState {
    parser: vt100::Parser,
    raw_chunks: VecDeque<Vec<u8>>,
    raw_bytes: usize,
    max_output_bytes: usize,
    total_output_bytes: u64,
    truncated: bool,
    exited: Option<ProcessExit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessExit {
    pub code: Option<u32>,
    pub signal: Option<String>,
}

impl TerminalState {
    pub(crate) fn new(size: TuiSize, scrollback_rows: usize, max_output_bytes: usize) -> Self {
        Self {
            parser: vt100::Parser::new(size.rows, size.columns, scrollback_rows),
            raw_chunks: VecDeque::new(),
            raw_bytes: 0,
            max_output_bytes,
            total_output_bytes: 0,
            truncated: false,
            exited: None,
        }
    }

    pub(crate) fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
        self.total_output_bytes = self
            .total_output_bytes
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        self.raw_chunks.push_back(bytes.to_vec());
        self.raw_bytes = self.raw_bytes.saturating_add(bytes.len());
        while self.raw_bytes > self.max_output_bytes {
            let excess = self.raw_bytes - self.max_output_bytes;
            let Some(first) = self.raw_chunks.front_mut() else {
                break;
            };
            if first.len() <= excess {
                let removed = self.raw_chunks.pop_front().expect("front was present");
                self.raw_bytes = self.raw_bytes.saturating_sub(removed.len());
            } else {
                first.drain(..excess);
                self.raw_bytes -= excess;
            }
            self.truncated = true;
        }
    }

    pub(crate) fn resize(&mut self, size: TuiSize) {
        self.parser.screen_mut().set_size(size.rows, size.columns);
    }

    pub(crate) fn set_exit(&mut self, exit: ProcessExit) {
        self.exited = Some(exit);
    }

    pub(crate) fn contents(&mut self) -> String {
        bounded_text(self.semantic_contents(), MAX_OBSERVATION_TEXT_BYTES)
    }

    pub(crate) fn contains_text(&mut self, expected: &str) -> bool {
        self.semantic_contents().contains(expected)
    }

    pub(crate) fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    pub(crate) fn bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste()
    }

    pub(crate) fn data(&self) -> Value {
        let screen = self.parser.screen();
        let viewport_text = bounded_text(screen.contents(), MAX_OBSERVATION_TEXT_BYTES);
        self.data_with_text(viewport_text)
    }

    pub(crate) fn data_with_history(&mut self) -> Value {
        let text = bounded_text(self.semantic_contents(), MAX_OBSERVATION_TEXT_BYTES);
        self.data_with_text(text)
    }

    fn data_with_text(&self, text: String) -> Value {
        let screen = self.parser.screen();
        let (rows, columns) = screen.size();
        let (cursor_row, cursor_column) = screen.cursor_position();
        json!({
            "surface": "tui",
            "viewport": {
                "rows": rows,
                "columns": columns,
                "text": text,
                "cursor": {
                    "row": cursor_row,
                    "column": cursor_column,
                    "visible": !screen.hide_cursor(),
                },
                "alternate_screen": screen.alternate_screen(),
                "application_cursor": screen.application_cursor(),
                "bracketed_paste": screen.bracketed_paste(),
            },
            "process": self.exited.as_ref().map(|exit| json!({
                "running": false,
                "exit_code": exit.code,
                "signal": exit.signal,
            })).unwrap_or_else(|| json!({ "running": true })),
            "output": {
                "total_bytes": self.total_output_bytes,
                "retained_bytes": self.raw_bytes,
                "truncated": self.truncated,
            }
        })
    }

    pub(crate) fn recording(&self) -> Result<Vec<u8>, DriverError> {
        let mut bytes = Vec::with_capacity(self.raw_bytes);
        for chunk in &self.raw_chunks {
            bytes.extend_from_slice(chunk);
        }
        if bytes.len() != self.raw_bytes {
            return Err(DriverError::new(
                "test.driver.tui.recording_invalid",
                "retained terminal recording length is inconsistent",
            ));
        }
        Ok(bytes)
    }

    fn semantic_contents(&mut self) -> String {
        if self.parser.screen().alternate_screen() {
            return self.parser.screen().contents();
        }
        self.parser.screen_mut().set_scrollback(usize::MAX);
        let scrollback = self.parser.screen().scrollback();
        if scrollback == 0 {
            return self.parser.screen().contents();
        }
        let rows = usize::from(self.parser.screen().size().0);
        let total_rows = scrollback.saturating_add(rows);
        let mut text = Vec::with_capacity(MAX_OBSERVATION_TEXT_BYTES);
        let mut start = 0;
        let mut complete = true;
        while start < total_rows && text.len() < MAX_OBSERVATION_TEXT_BYTES {
            let offset = scrollback.saturating_sub(start);
            self.parser.screen_mut().set_scrollback(offset);
            let window_start = scrollback - offset;
            let skip = start - window_start;
            let take = rows.saturating_sub(skip).min(total_rows - start);
            complete = append_rows(&mut text, self.parser.screen(), skip, take);
            start = start.saturating_add(take);
            if !complete {
                break;
            }
        }
        self.parser.screen_mut().set_scrollback(0);
        if start < total_rows || !complete {
            let marker = b"\n[terminal observation truncated]";
            let keep = MAX_OBSERVATION_TEXT_BYTES.saturating_sub(marker.len());
            text.truncate(keep);
            while std::str::from_utf8(&text).is_err() {
                text.pop();
            }
            text.extend_from_slice(marker);
        }
        String::from_utf8_lossy(&text).into_owned()
    }
}

fn append_rows(text: &mut Vec<u8>, screen: &vt100::Screen, skip: usize, take: usize) -> bool {
    let (_, columns) = screen.size();
    for row in screen.rows(0, columns).skip(skip).take(take) {
        if !text.is_empty() && !append_bounded(text, b"\n") {
            return false;
        }
        if !append_bounded(text, row.trim_end().as_bytes()) {
            return false;
        }
    }
    true
}

fn append_bounded(output: &mut Vec<u8>, bytes: &[u8]) -> bool {
    let remaining = MAX_OBSERVATION_TEXT_BYTES.saturating_sub(output.len());
    if bytes.len() <= remaining {
        output.extend_from_slice(bytes);
        return true;
    }
    output.extend_from_slice(&bytes[..remaining]);
    false
}

fn bounded_text(mut text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let mut boundary = max_bytes;
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    text.truncate(boundary);
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_viewport_tracks_alternate_screen_cursor_and_bounds_raw_output() {
        let mut state = TerminalState::new(TuiSize::default(), 10, 8);
        state.process(b"first");
        state.process(b"\x1b[?1049hmenu\x1b[2;3H");

        let data = state.data();
        assert_eq!(data["viewport"]["alternate_screen"], true);
        assert_eq!(data["viewport"]["cursor"]["row"], 1);
        assert_eq!(data["viewport"]["cursor"]["column"], 2);
        assert_eq!(data["output"]["truncated"], true);
        assert!(state.recording().expect("recording").len() <= 8);
    }
}
