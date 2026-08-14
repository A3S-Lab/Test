use a3s_test_core::DriverError;

const MAX_KEY_CHORD: usize = 64;
const MAX_PASTE_BYTES: usize = 1024 * 1024;

pub(crate) fn key_bytes(key: &str, application_cursor: bool) -> Result<Vec<u8>, DriverError> {
    if key.is_empty() || key.len() > MAX_KEY_CHORD || key.trim() != key {
        return Err(input_error("terminal key chord is empty or unbounded"));
    }

    let parts = key.split('+').collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) {
        return Err(input_error("terminal key chord contains an empty part"));
    }

    if parts.len() == 1 {
        return named_key(parts[0], application_cursor);
    }
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("Control") {
        return control_key(parts[1]);
    }
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("Alt") {
        let mut bytes = vec![0x1b];
        bytes.extend(single_character(parts[1])?);
        return Ok(bytes);
    }
    Err(input_error("unsupported terminal key chord"))
}

pub(crate) fn paste_bytes(text: &str, bracketed: bool) -> Result<Vec<u8>, DriverError> {
    if text.len() > MAX_PASTE_BYTES {
        return Err(input_error("terminal paste exceeds 1048576 bytes"));
    }
    if bracketed {
        let mut bytes = Vec::with_capacity(text.len() + 12);
        bytes.extend_from_slice(b"\x1b[200~");
        bytes.extend_from_slice(text.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        Ok(bytes)
    } else {
        Ok(text.as_bytes().to_vec())
    }
}

fn named_key(key: &str, application_cursor: bool) -> Result<Vec<u8>, DriverError> {
    let bytes: &[u8] = match key {
        "Enter" => b"\r",
        "Tab" => b"\t",
        "Backspace" => b"\x7f",
        "Escape" => b"\x1b",
        "Space" => b" ",
        "Up" if application_cursor => b"\x1bOA",
        "Down" if application_cursor => b"\x1bOB",
        "Right" if application_cursor => b"\x1bOC",
        "Left" if application_cursor => b"\x1bOD",
        "Up" => b"\x1b[A",
        "Down" => b"\x1b[B",
        "Right" => b"\x1b[C",
        "Left" => b"\x1b[D",
        "Home" => b"\x1b[H",
        "End" => b"\x1b[F",
        "PageUp" => b"\x1b[5~",
        "PageDown" => b"\x1b[6~",
        "Insert" => b"\x1b[2~",
        "Delete" => b"\x1b[3~",
        "F1" => b"\x1bOP",
        "F2" => b"\x1bOQ",
        "F3" => b"\x1bOR",
        "F4" => b"\x1bOS",
        _ => return single_character(key),
    };
    Ok(bytes.to_vec())
}

fn control_key(key: &str) -> Result<Vec<u8>, DriverError> {
    let character = key
        .chars()
        .next()
        .filter(|_| key.chars().count() == 1)
        .ok_or_else(|| input_error("Control chords require one ASCII key"))?;
    let upper = character.to_ascii_uppercase();
    if !upper.is_ascii_alphabetic() {
        return Err(input_error("Control chords require one ASCII letter"));
    }
    Ok(vec![(upper as u8) & 0x1f])
}

fn single_character(value: &str) -> Result<Vec<u8>, DriverError> {
    if value.chars().count() != 1 {
        return Err(input_error(
            "terminal key must be a named key or one character",
        ));
    }
    Ok(value.as_bytes().to_vec())
}

fn input_error(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.tui.input_invalid", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_encoding_honors_application_cursor_mode() {
        assert_eq!(key_bytes("Up", false).expect("normal cursor"), b"\x1b[A");
        assert_eq!(
            key_bytes("Up", true).expect("application cursor"),
            b"\x1bOA"
        );
        assert_eq!(key_bytes("Control+C", false).expect("interrupt"), b"\x03");
    }

    #[test]
    fn bracketed_paste_is_explicitly_framed() {
        assert_eq!(
            paste_bytes("hello", true).expect("paste"),
            b"\x1b[200~hello\x1b[201~"
        );
    }
}
