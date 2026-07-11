pub(crate) fn encode_url_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved_and_utf8_bytes_but_preserves_unreserved_characters() {
        assert_eq!(
            encode_url_path_segment("models/gemini 2.5-测试"),
            "models%2Fgemini%202.5-%E6%B5%8B%E8%AF%95"
        );
        assert_eq!(encode_url_path_segment("a-z_A.Z~09"), "a-z_A.Z~09");
    }
}
