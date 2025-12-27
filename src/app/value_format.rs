use std::fmt::Write;

pub fn format_bytes_inline(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(empty)".to_string();
    }

    if let Some(text) = utf8_if_printable(bytes) {
        return escape_inline(&text);
    }

    hex_inline(bytes)
}

pub fn format_bytes_block(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(empty)".to_string();
    }

    if let Some(text) = utf8_if_printable(bytes) {
        return text;
    }

    hex_multiline(bytes)
}

pub fn format_json_pretty(raw: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|_| raw.to_string()),
        Err(_) => raw.to_string(),
    }
}

fn utf8_if_printable(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;

    if text.chars().all(is_printable_char) {
        Some(text.to_string())
    } else {
        None
    }
}

fn is_printable_char(ch: char) -> bool {
    if ch.is_control() {
        matches!(ch, '\n' | '\r' | '\t')
    } else {
        true
    }
}

fn escape_inline(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn hex_inline(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (idx, byte) in bytes.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        write!(out, "{:02X}", byte).ok();
    }
    out
}

fn hex_multiline(bytes: &[u8]) -> String {
    const LINE_BYTES: usize = 16;
    let mut out = String::new();
    let total_lines = bytes.len().div_ceil(LINE_BYTES);

    for (line_index, chunk) in bytes.chunks(LINE_BYTES).enumerate() {
        let offset = line_index * LINE_BYTES;
        write!(out, "{:08X}: ", offset).ok();
        for (idx, byte) in chunk.iter().enumerate() {
            if idx > 0 {
                out.push(' ');
            }
            write!(out, "{:02X}", byte).ok();
        }
        if line_index + 1 < total_lines {
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_inline_prefers_utf8() {
        let input = b"hello";
        assert_eq!(format_bytes_inline(input), "hello");
    }

    #[test]
    fn format_bytes_inline_hex_for_binary() {
        let input = [0x00, 0xFF, 0x10];
        assert_eq!(format_bytes_inline(&input), "00 FF 10");
    }

    #[test]
    fn format_bytes_block_keeps_newlines() {
        let input = b"hi\nthere";
        assert_eq!(format_bytes_block(input), "hi\nthere");
    }

    #[test]
    fn format_json_pretty_formats_json() {
        let raw = r#"{"a":1,"b":[true,false]}"#;
        let expected = serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(raw).unwrap(),
        )
        .unwrap();
        assert_eq!(format_json_pretty(raw), expected);
    }

    #[test]
    fn format_json_pretty_falls_back_on_invalid() {
        let raw = "not-json";
        assert_eq!(format_json_pretty(raw), raw);
    }
}
