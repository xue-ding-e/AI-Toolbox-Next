//! Common Database ID Utilities for SurrealDB Record IDs
//!
//! Provides standardized functions for handling SurrealDB record IDs:
//! - Converting raw database IDs to clean business IDs
//! - Stripping table prefixes and SurrealDB wrapper characters
//!
//! **Usage**:
//! ```rust
//! use super::db_id::{db_clean_id, db_extract_id};
//!
//! let id = db_extract_id(&record);
//! ```

use serde_json::Value;

/// Clean a SurrealDB record ID by stripping table prefix and wrapper characters.
///
/// **Purpose**: SurrealDB returns IDs in formats like:
/// - `"claude_provider:c6bs..."` (with table prefix)
/// - `"claude_provider:⟨uuid⟩"` (with table prefix and wrapper)
/// - `"⟨uuid⟩"` (with wrapper only)
///
/// **Output**: Clean ID suitable for frontend and business logic: `"c6bs..."` or `"uuid"`
///
/// # Example
/// ```rust
/// let raw_id = "claude_provider:⟨abc-123⟩";
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
    // Strip SurrealDB wrapper characters ⟨⟩ if present
    without_prefix
        .trim_start_matches('⟨')
        .trim_end_matches('⟩')
        .to_string()
}

/// Extract a clean ID from a database record Value.
///
/// **Purpose**: Safely extract and clean the ID from a SurrealDB record.
///
/// # Example
/// ```rust
/// let record = json!({
///     "id": "claude_provider:⟨abc-123⟩",
///     "name": "Test"
/// });
/// let id = db_extract_id(&record);
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

/// Build a SurrealDB record ID from table name and ID.
///
/// **Purpose**: Create a proper Thing ID string for queries.
///
/// # Example
/// ```rust
/// let thing_id = db_build_id("claude_provider", "abc-123");
/// assert_eq!(thing_id, "claude_provider:abc-123");
/// ```
pub fn db_build_id(table: &str, id: &str) -> String {
    format!("{}:{}", table, id)
}
