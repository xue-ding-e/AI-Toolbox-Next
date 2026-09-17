use crate::coding::proxy_gateway::{
    aggregate_naming::{AggregateNamingMode, AggregateSlugEntry},
    types::{GatewayCliKey, GatewayProxyMode},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AggregateManifestConfig {
    /// Sites selected for aggregate routing, in the user's display order.
    #[serde(default)]
    pub provider_ids: Vec<String>,
    /// Separator between site id and upstream model name. Defaults to `.`.
    #[serde(default = "default_aggregate_separator")]
    pub separator: String,
    /// Per-site display/routing aliases. A missing entry falls back to the
    /// provider id so manifests written before aliases remain compatible.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
    /// How `(site, model)` pairs are named in the generated Codex catalog.
    #[serde(default)]
    pub naming: AggregateNamingMode,
    /// Slug table the Codex catalog was generated from, in publication order.
    ///
    /// Persisted so routing replays the exact table instead of re-deriving it
    /// from the currently enabled candidates: with `model_only` a site that
    /// disappears would otherwise renumber every later `#N` slug, silently
    /// pointing it at another site. `provider_ids` + `naming` rebuild the table
    /// for manifests written before this field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slug_table: Vec<AggregateSlugEntry>,
}

fn default_aggregate_separator() -> String {
    AGGREGATE_DEFAULT_SEPARATOR.to_string()
}

impl Default for AggregateManifestConfig {
    fn default() -> Self {
        Self {
            provider_ids: Vec::new(),
            separator: default_aggregate_separator(),
            aliases: BTreeMap::new(),
            naming: AggregateNamingMode::default(),
            slug_table: Vec::new(),
        }
    }
}

/// Default separator between the site id and the upstream model name in
/// aggregate mode. `.` keeps the generated slugs acceptable to Codex's
/// telemetry tags (unlike `:`) while still being readable.
pub const AGGREGATE_DEFAULT_SEPARATOR: &str = ".";

/// Validate a user-supplied aggregate separator.
///
/// The separator must be non-empty and must not contain characters that are
/// legal inside a site id, otherwise `<site_id><sep><model>` becomes ambiguous
/// and cannot be split back reliably.
pub fn validate_aggregate_separator(separator: &str) -> Result<(), String> {
    if separator.is_empty() {
        return Err("Aggregate separator must not be empty".to_string());
    }
    if separator
        .chars()
        .any(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("Aggregate separator must not contain letters, digits, '_' or '-'".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CliProxyManifest {
    pub schema_version: u32,
    pub managed_by: String,
    pub cli_key: GatewayCliKey,
    pub enabled: bool,
    pub mode: GatewayProxyMode,
    pub primary_provider_id: String,
    pub base_origin: String,
    pub created_at: String,
    pub updated_at: String,
    pub files: Vec<CliProxyManifestFile>,
    /// Aggregate-mode routing config. Absent for single/failover manifests, and
    /// absent in manifests written before aggregate mode existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggregateManifestConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CliProxyManifestFile {
    pub kind: String,
    pub path: String,
    pub existed: bool,
    pub backup_rel_path: String,
    pub backup_sha256: Option<String>,
    pub backup_size: Option<u64>,
    pub managed_fields: Vec<String>,
}

impl CliProxyManifest {
    pub fn new(
        cli_key: GatewayCliKey,
        base_origin: String,
        timestamp: String,
        mode: GatewayProxyMode,
        primary_provider_id: String,
    ) -> Self {
        Self {
            schema_version: 1,
            managed_by: "ai-toolbox-proxy-gateway".to_string(),
            cli_key,
            enabled: true,
            mode,
            primary_provider_id,
            base_origin,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            files: Vec::new(),
            aggregate: None,
        }
    }

    /// Attach aggregate routing config and switch the manifest to aggregate mode.
    pub fn with_aggregate(
        mut self,
        provider_ids: Vec<String>,
        separator: String,
        aliases: BTreeMap<String, String>,
        naming: AggregateNamingMode,
        slug_table: Vec<AggregateSlugEntry>,
    ) -> Self {
        self.mode = GatewayProxyMode::Aggregate;
        self.aggregate = Some(AggregateManifestConfig {
            provider_ids,
            separator,
            aliases,
            naming,
            slug_table,
        });
        self
    }
}

pub fn validate_backup_rel_path(path: &str) -> Result<(), String> {
    if path.contains(':') || path.contains('\\') {
        return Err("Manifest backup path must use a relative forward-slash path".to_string());
    }
    let path = Path::new(path);
    if path.is_absolute() {
        return Err("Manifest backup path must be relative".to_string());
    }
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Manifest backup path cannot escape the backup directory".to_string())
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serializes_without_provider_data() {
        let mut manifest = CliProxyManifest::new(
            GatewayCliKey::Codex,
            "http://127.0.0.1:37123".to_string(),
            "2026-05-16T10:00:00Z".to_string(),
            GatewayProxyMode::Single,
            "provider-1".to_string(),
        );
        manifest.files.push(CliProxyManifestFile {
            kind: "codex_config_toml".to_string(),
            path: "C:\\Users\\User\\.codex\\config.toml".to_string(),
            existed: true,
            backup_rel_path: "backups/config.toml".to_string(),
            backup_sha256: Some("abc".to_string()),
            backup_size: Some(123),
            managed_fields: vec![
                "model_providers.custom.base_url".to_string(),
                "model_providers.custom.wire_api".to_string(),
                "model_providers.custom.experimental_bearer_token".to_string(),
            ],
        });

        let json = serde_json::to_string(&manifest).unwrap();

        assert!(json.contains("codex_config_toml"));
        assert!(json.contains("primary_provider_id"));
        assert!(!json.contains("settings_config"));
        assert!(!json.contains("api_key"));
    }

    #[test]
    fn backup_relative_path_accepts_normal_path() {
        assert!(validate_backup_rel_path("backups/config.toml").is_ok());
    }

    #[test]
    fn backup_relative_path_rejects_parent_escape() {
        assert!(validate_backup_rel_path("../config.toml").is_err());
        assert!(validate_backup_rel_path("backups/../../config.toml").is_err());
    }

    #[test]
    fn backup_relative_path_rejects_absolute_path() {
        assert!(validate_backup_rel_path("C:\\Users\\config.toml").is_err());
        assert!(validate_backup_rel_path("/tmp/config.toml").is_err());
    }

    #[test]
    fn aggregate_manifest_defaults_naming_for_older_manifests() {
        let parsed: AggregateManifestConfig = serde_json::from_value(serde_json::json!({
            "provider_ids": ["site-a"],
            "separator": "."
        }))
        .unwrap();

        assert!(parsed.aliases.is_empty());
        assert_eq!(parsed.naming, AggregateNamingMode::SiteModel);
        // No persisted table: routing rebuilds it from `provider_ids` + `naming`.
        assert!(parsed.slug_table.is_empty());
    }
}
