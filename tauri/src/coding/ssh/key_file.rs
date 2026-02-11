//! SSH private key content detection

/// Check whether the given string looks like a PEM private key (content, not a path).
pub fn is_private_key_content(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("-----BEGIN")
}
