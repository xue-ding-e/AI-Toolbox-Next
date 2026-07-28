use serde_json::Value;

pub(crate) fn append_utf8_safe(buffer: &mut String, remainder: &mut Vec<u8>, chunk: &[u8]) {
    if chunk.is_empty() {
        return;
    }

    let mut bytes = Vec::with_capacity(remainder.len() + chunk.len());
    bytes.extend_from_slice(remainder);
    bytes.extend_from_slice(chunk);
    remainder.clear();

    let mut offset = 0;
    while offset < bytes.len() {
        match std::str::from_utf8(&bytes[offset..]) {
            Ok(text) => {
                buffer.push_str(text);
                return;
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    buffer.push_str(
                        std::str::from_utf8(&bytes[offset..offset + valid_up_to])
                            .expect("valid_up_to must point at valid UTF-8"),
                    );
                    offset += valid_up_to;
                }
                match error.error_len() {
                    Some(error_len) => {
                        // Invalid byte sequence: replace and keep parsing the rest.
                        buffer.push('\u{FFFD}');
                        offset += error_len;
                    }
                    None => {
                        // Incomplete trailing sequence: wait for more bytes.
                        remainder.extend_from_slice(&bytes[offset..]);
                        return;
                    }
                }
            }
        }
    }
}

pub(crate) fn take_sse_block(buffer: &mut String) -> Option<String> {
    let (index, delimiter_len) = find_sse_delimiter(buffer)?;
    let block = buffer[..index].to_string();
    let rest_start = index + delimiter_len;
    buffer.replace_range(..rest_start, "");
    Some(block)
}

fn find_sse_delimiter(buffer: &str) -> Option<(usize, usize)> {
    let bytes = buffer.as_bytes();
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4));
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2));
    match (crlf, lf) {
        (Some(crlf), Some(lf)) => Some(if crlf.0 <= lf.0 { crlf } else { lf }),
        (Some(delimiter), None) | (None, Some(delimiter)) => Some(delimiter),
        (None, None) => None,
    }
}

pub(crate) fn parse_sse_block(block: &str) -> ParsedSseBlock {
    let mut event: Option<String> = None;
    let mut data_parts = Vec::new();

    for line in block.lines() {
        if let Some(value) = strip_sse_field(line, "event") {
            event = Some(value.trim().to_string());
        } else if let Some(value) = strip_sse_field(line, "data") {
            data_parts.push(value.to_string());
        }
    }

    ParsedSseBlock {
        event,
        data: data_parts.join("\n"),
    }
}

pub(crate) fn strip_sse_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(field)?;
    let rest = rest.strip_prefix(':')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

pub(crate) fn sse_event(event: Option<&str>, value: &Value) -> Vec<u8> {
    let payload = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    match event {
        Some(event) if !event.is_empty() => format!("event: {event}\ndata: {payload}\n\n").into(),
        _ => format!("data: {payload}\n\n").into(),
    }
}

pub(crate) fn sse_done() -> Vec<u8> {
    b"data: [DONE]\n\n".to_vec()
}

pub(crate) struct ParsedSseBlock {
    pub(crate) event: Option<String>,
    pub(crate) data: String,
}

#[cfg(test)]
mod tests {
    use super::{append_utf8_safe, take_sse_block};

    #[test]
    fn parser_uses_the_earliest_mixed_line_ending_delimiter() {
        let mut buffer =
            "event: first\ndata: {\"type\":\"first\"}\n\nevent: second\r\ndata: {}\r\n\r\n"
                .to_string();
        assert_eq!(
            take_sse_block(&mut buffer).as_deref(),
            Some("event: first\ndata: {\"type\":\"first\"}")
        );
        assert_eq!(
            take_sse_block(&mut buffer).as_deref(),
            Some("event: second\r\ndata: {}")
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn append_utf8_safe_replaces_invalid_bytes_and_continues() {
        let mut buffer = String::new();
        let mut remainder = Vec::new();
        append_utf8_safe(&mut buffer, &mut remainder, b"hello");
        append_utf8_safe(&mut buffer, &mut remainder, b"\xff world");
        append_utf8_safe(&mut buffer, &mut remainder, b" tail");
        assert_eq!(buffer, "hello\u{FFFD} world tail");
        assert!(remainder.is_empty());
    }

    #[test]
    fn append_utf8_safe_keeps_incomplete_multibyte_sequence() {
        let mut buffer = String::new();
        let mut remainder = Vec::new();
        // Leading bytes of a 3-byte UTF-8 sequence without continuation.
        append_utf8_safe(&mut buffer, &mut remainder, &[0xE4, 0xBD]);
        assert!(buffer.is_empty());
        assert_eq!(remainder, vec![0xE4, 0xBD]);
        append_utf8_safe(&mut buffer, &mut remainder, &[0xA0]);
        assert_eq!(buffer.chars().count(), 1);
        assert!(remainder.is_empty());
    }
}
