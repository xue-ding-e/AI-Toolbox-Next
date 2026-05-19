//! Common database ID utilities.
//!
//! Provides standardized functions for handling current SQLite row IDs and
//! legacy table-prefixed IDs:
//! - Converting raw database IDs to clean business IDs
//! - Stripping table prefixes and legacy wrapper characters
//!
//! **Usage**:
//! ```rust
//! use ai_toolbox_lib::coding::{db_clean_id, db_extract_id};
//! use serde_json::json;
//!
//! let record = json!({
//!     "id": "claude_provider:`abc-123`",
//!     "name": "Test"
//! });
//!
//! let id = db_extract_id(&record);
//! assert_eq!(id, "abc-123");
//!
//! let clean = db_clean_id("claude_provider:`abc-123`");
//! assert_eq!(clean, "abc-123");
//! ```

use serde_json::Value;

/// Clean a database record ID by stripping table prefix and wrapper characters.
///
/// **Purpose**: IDs may be plain SQLite row IDs or legacy imported references:
/// - `"abc-123"` (plain row ID)
/// - `"claude_provider:c6bs..."` (with table prefix)
/// - `"claude_provider:⟨uuid⟩"` (with table prefix and wrapper)
/// - `"⟨uuid⟩"` (with wrapper only)
///
/// **Output**: Clean ID suitable for frontend and business logic: `"c6bs..."` or `"uuid"`
///
/// # Example
/// ```rust
/// use ai_toolbox_lib::coding::db_clean_id;
///
/// let raw_id = "claude_provider:`abc-123`";
/// let clean = db_clean_id(raw_id);
/// assert_eq!(clean, "abc-123");
/// ```
pub fn db_clean_id(raw_id: &str) -> String {
    // Strip table prefix if present (e.g., "claude_provider:xxx" -> "xxx")
    let without_prefix = if let Some(pos) = raw_id.find(':') {
        &raw_id[pos + 1..]
    } else {
        raw_id
    };
    // Strip legacy wrapper characters and backticks if present.
    without_prefix
        .trim_start_matches('⟨')
        .trim_end_matches('⟩')
        .trim_start_matches('`')
        .trim_end_matches('`')
        .to_string()
}

/// Extract a clean ID from a database record Value.
///
/// **Purpose**: Safely extract and normalize the ID from a database record.
///
/// # Example
/// ```rust
/// use ai_toolbox_lib::coding::db_extract_id;
/// use serde_json::json;
///
/// let record = json!({
///     "id": "claude_provider:`abc-123`",
///     "name": "Test"
/// });
/// let id = db_extract_id(&record);
/// assert_eq!(id, "abc-123");
/// ```
pub fn db_extract_id(record: &Value) -> String {
    record
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| db_clean_id(s))
        .unwrap_or_default()
}

/// Extract a clean ID from a database record, returning None if not found.
pub fn db_extract_id_opt(record: &Value) -> Option<String> {
    record
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| db_clean_id(s))
}

/// Build a table-prefixed record ID from table name and ID.
///
/// **Purpose**: Create a stable legacy-compatible record reference string.
///
/// # Example
/// ```rust
/// use ai_toolbox_lib::coding::db_build_id;
///
/// let record_id = db_build_id("claude_provider", "abc-123");
/// assert_eq!(record_id, "claude_provider:abc-123");
/// ```
pub fn db_build_id(table: &str, id: &str) -> String {
    format!("{}:{}", table, id)
}

/// Build a backtick-escaped legacy record reference for migration queries.
///
/// Returns format: `` table:`id` `` which ensures the ID is treated as a literal
/// string regardless of its content (hyphens, slashes, etc.).
///
/// **Purpose**: Some one-time legacy migrations still need this escaped record
/// reference format for IDs with special characters.
///
/// **Security**: Input ID is sanitized to only allow safe characters.
///
/// # Example
/// ```rust
/// use ai_toolbox_lib::coding::db_record_id;
///
/// let ref_id = db_record_id("mcp_server", "100dcf2a-3718-457f-b1ef-31d48c3478f8");
/// assert_eq!(ref_id, "mcp_server:`100dcf2a-3718-457f-b1ef-31d48c3478f8`");
/// ```
pub fn db_record_id(table: &str, id: &str) -> String {
    // Sanitize: only allow safe characters to prevent query injection
    let clean: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '/' || *c == '.')
        .collect();
    format!("{}:`{}`", table, clean)
}

/// Generate a new database record ID (UUID v4 without hyphens).
///
/// # Example
/// ```rust
/// use ai_toolbox_lib::coding::db_new_id;
///
/// let id = db_new_id(); // e.g. "a1b2c3d4e5f6..."
/// assert!(!id.is_empty());
/// assert!(!id.contains('-'));
/// ```
pub fn db_new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
