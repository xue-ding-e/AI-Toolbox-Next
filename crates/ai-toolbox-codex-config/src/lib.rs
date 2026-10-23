// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure Codex TOML composition, extracted from AI Toolbox's Codex commands.
//!
//! The original AI Toolbox authorship and repository license remain applicable.
//! This crate has no filesystem, authentication, database, event, or runtime owner.
//! Composition is not a live-file projection: runtime-owned field preservation,
//! previous-managed cleanup, catalog generation, and application remain with the
//! product service. Do not use this helper as a replacement for that service.

pub fn parse_toml_document(raw_toml: &str, context: &str) -> Result<toml_edit::DocumentMut, String> {
    if raw_toml.trim().is_empty() {
        Ok(toml_edit::DocumentMut::new())
    } else {
        raw_toml
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| format!("Failed to parse {}: {}", context, e))
    }
}

pub fn merge_toml_tables(base: &mut toml_edit::Table, overlay: &toml_edit::Table) {
    for (key, overlay_item) in overlay.iter() {
        if let Some(base_item) = base.get_mut(key) {
            merge_toml_items(base_item, overlay_item);
        } else {
            base.insert(key, overlay_item.clone());
        }
    }
}

fn merge_toml_items(base: &mut toml_edit::Item, overlay: &toml_edit::Item) {
    match (base, overlay) {
        (toml_edit::Item::Table(base_table), toml_edit::Item::Table(overlay_table)) => {
            merge_toml_tables(base_table, overlay_table);
        }
        (base_item, overlay_item) => {
            *base_item = overlay_item.clone();
        }
    }
}

/// Merge common defaults with provider config while preserving explicit provider overrides.
pub fn append_toml_configs(provider: &str, common: &str) -> Result<String, String> {
    let provider_content = provider.trim();
    let common_content = common.trim();

    if provider_content.is_empty() {
        return Ok(common_content.to_string());
    }
    if common_content.is_empty() {
        return Ok(provider_content.to_string());
    }

    let provider_doc = parse_toml_document(provider_content, "provider config")?;
    let mut common_doc = parse_toml_document(common_content, "common config")?;

    merge_toml_tables(common_doc.as_table_mut(), provider_doc.as_table());
    Ok(common_doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml_edit::DocumentMut;

    #[test]
    fn append_toml_configs_keeps_common_root_keys_at_root() {
        let provider = r#"
model_provider = "custom"

[model_providers.custom]
name = "custom"
wire_api = "responses"
requires_openai_auth = true
"#;

        let common = r#"
approval_policy = "never"
sandbox_mode = "danger-full-access"
"#;

        let merged = append_toml_configs(provider, common).unwrap();
        let doc: DocumentMut = merged.parse().unwrap();

        assert_eq!(doc["approval_policy"].as_str(), Some("never"));
        assert_eq!(doc["sandbox_mode"].as_str(), Some("danger-full-access"));
        assert_eq!(
            doc["model_providers"]["custom"]["name"].as_str(),
            Some("custom")
        );
    }

    #[test]
    fn append_toml_configs_merges_common_tables_without_overwriting_provider_table() {
        let provider = r#"
[model_providers.custom]
name = "custom"
"#;

        let common = r#"
[model_providers.custom]
wire_api = "responses"
"#;

        let merged = append_toml_configs(provider, common).unwrap();
        let doc: DocumentMut = merged.parse().unwrap();

        assert_eq!(
            doc["model_providers"]["custom"]["name"].as_str(),
            Some("custom")
        );
        assert_eq!(
            doc["model_providers"]["custom"]["wire_api"].as_str(),
            Some("responses")
        );
    }

    #[test]
    fn append_toml_configs_keeps_explicit_provider_scalar_overrides() {
        let provider = r#"
model = "glm-5.2"
model_reasoning_effort = "high"
"#;
        let common = r#"
model = "gpt-5.4"
model_reasoning_effort = "medium"
approval_policy = "never"
"#;

        let merged = append_toml_configs(provider, common).unwrap();
        let doc: DocumentMut = merged.parse().unwrap();

        assert_eq!(doc["model"].as_str(), Some("glm-5.2"));
        assert_eq!(doc["model_reasoning_effort"].as_str(), Some("high"));
        assert_eq!(doc["approval_policy"].as_str(), Some("never"));
    }

    #[test]
    fn parse_empty_document_and_report_context() {
        assert!(parse_toml_document(" \n", "empty").unwrap().is_empty());
        let error = parse_toml_document("invalid = [", "common config").unwrap_err();
        assert!(error.starts_with("Failed to parse common config:"));
    }

    #[test]
    fn append_preserves_existing_empty_side_fast_path() {
        // This extraction deliberately preserves the desktop helper's behavior:
        // a lone non-empty input is trimmed and returned without validation.
        // A future typed plan must validate both documents before composing.
        assert_eq!(append_toml_configs("", " invalid = [ ").unwrap(), "invalid = [");
        assert_eq!(append_toml_configs(" model = 'x' ", "").unwrap(), "model = 'x'");
        assert_eq!(append_toml_configs(" ", "\n").unwrap(), "");
    }

    #[test]
    fn append_rejects_invalid_toml_when_composing_two_documents() {
        assert!(append_toml_configs("model = [", "other = true").is_err());
        assert!(append_toml_configs("other = true", "model = [").is_err());
    }

    #[test]
    fn append_keeps_unmodified_comments_and_provider_precedence() {
        let common = "# Common model\nmodel = 'default' # default comment\n# Untouched\nother = 'retained' # retain this\n\n[features]\nalpha = true # keep alpha\n";
        let provider = "model = 'chosen' # chosen comment\n\n[features]\nbeta = false\n";
        let merged = append_toml_configs(provider, common).unwrap();
        assert!(merged.contains("# Untouched\nother = 'retained' # retain this"));
        assert!(merged.contains("alpha = true # keep alpha"));
        assert!(merged.contains("model = 'chosen' # chosen comment"));
        let document = parse_toml_document(&merged, "result").unwrap();
        assert_eq!(document["features"]["alpha"].as_bool(), Some(true));
        assert_eq!(document["features"]["beta"].as_bool(), Some(false));
    }

    #[test]
    fn merge_replaces_non_tables_instead_of_inventing_array_or_inline_merges() {
        let common = "items = [1, 2]\ninline = { old = true }\n\n[replace]\nold = true\n";
        let provider = "items = [3]\ninline = { new = true }\nreplace = 'scalar'\n";
        let merged = append_toml_configs(provider, common).unwrap();
        let document = parse_toml_document(&merged, "result").unwrap();
        let items = document["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items.get(0).and_then(toml_edit::Value::as_integer), Some(3));
        let inline = document["inline"].as_inline_table().unwrap();
        assert!(inline.get("old").is_none());
        assert_eq!(inline.get("new").and_then(toml_edit::Value::as_bool), Some(true));
        assert_eq!(document["replace"].as_str(), Some("scalar"));
    }
}
