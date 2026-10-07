use chrono::Local;
use serde_json::{json, Value};

use super::types::{
    ZcodeCommonConfig, ZcodeCommonConfigRecord, ZcodeOfficialAccount, ZcodeOfficialAccountContent,
    ZcodePromptConfig, ZcodeProvider, ZcodeProviderContent,
};

/// Reads a row id verbatim.
///
/// ZCode is the one module whose business id *is* the row id, and managed ids
/// carry a `custom:` prefix. The shared `db_clean_id` treats the first colon as
/// a legacy `table:id` separator and strips everything before it, which turns
/// `custom:my-provider` into `my-provider` — an id no row has. Every read-back
/// then failed with "provider not found" while the row sat in the table
/// untouched, so this module reads the raw value instead.
fn zcode_row_id(value: &Value) -> String {
    value
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn get_str_compat(value: &Value, snake_key: &str, camel_key: &str, default: &str) -> String {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

fn get_opt_str_compat(value: &Value, snake_key: &str, camel_key: &str) -> Option<String> {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn get_i64_compat(value: &Value, snake_key: &str, camel_key: &str) -> i64 {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

fn get_opt_i64_compat(value: &Value, snake_key: &str, camel_key: &str) -> Option<i64> {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_i64())
}

fn get_bool_compat(value: &Value, snake_key: &str, camel_key: &str, default: bool) -> bool {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

pub fn from_db_value_provider(value: Value) -> ZcodeProvider {
    ZcodeProvider {
        id: zcode_row_id(&value),
        name: get_str_compat(&value, "name", "name", "Unnamed Provider"),
        category: get_str_compat(&value, "category", "category", "custom"),
        settings_config: get_str_compat(&value, "settings_config", "settingsConfig", "{}"),
        source_provider_id: get_opt_str_compat(&value, "source_provider_id", "sourceProviderId"),
        website_url: get_opt_str_compat(&value, "website_url", "websiteUrl"),
        notes: get_opt_str_compat(&value, "notes", "notes"),
        icon: get_opt_str_compat(&value, "icon", "icon"),
        icon_color: get_opt_str_compat(&value, "icon_color", "iconColor"),
        sort_index: get_i64_compat(&value, "sort_index", "sortIndex"),
        meta: value.get("meta").cloned(),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        is_disabled: get_bool_compat(&value, "is_disabled", "isDisabled", false),
        created_at: get_str_compat(&value, "created_at", "createdAt", ""),
        updated_at: get_str_compat(&value, "updated_at", "updatedAt", ""),
    }
}

pub fn to_db_value_provider(content: &ZcodeProviderContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|error| {
        eprintln!("Failed to serialize ZCode provider content: {error}");
        json!({})
    })
}

pub fn from_db_value_common(value: Value) -> ZcodeCommonConfigRecord {
    ZcodeCommonConfigRecord {
        id: zcode_row_id(&value),
        config: get_str_compat(&value, "config", "config", "{}"),
        root_dir: get_opt_str_compat(&value, "root_dir", "rootDir"),
        official_account_index: get_opt_i64_compat(
            &value,
            "official_account_index",
            "officialAccountIndex",
        ),
        updated_at: get_str_compat(&value, "updated_at", "updatedAt", ""),
    }
}

/// Writes the whole common-config record, so every caller must pass every
/// field: a blind `db_put` of a partial payload is how a field gets erased by
/// an unrelated save.
pub fn to_db_value_common(
    config: &str,
    root_dir: Option<&str>,
    official_account_index: Option<i64>,
) -> Value {
    json!({
        "config": config,
        "root_dir": root_dir,
        "official_account_index": official_account_index,
        "updated_at": Local::now().to_rfc3339(),
    })
}

pub fn from_db_value_prompt(value: Value) -> ZcodePromptConfig {
    ZcodePromptConfig {
        id: zcode_row_id(&value),
        name: get_str_compat(&value, "name", "name", "Unnamed Prompt"),
        content: get_str_compat(&value, "content", "content", ""),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        sort_index: get_i64_compat(&value, "sort_index", "sortIndex"),
        created_at: get_str_compat(&value, "created_at", "createdAt", ""),
        updated_at: get_str_compat(&value, "updated_at", "updatedAt", ""),
    }
}

/// Convenience conversion used when a caller already holds the record type.
impl From<ZcodeCommonConfigRecord> for ZcodeCommonConfig {
    fn from(record: ZcodeCommonConfigRecord) -> Self {
        Self {
            config: record.config,
            root_dir: record.root_dir,
            official_account_index: record.official_account_index,
            updated_at: record.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_round_trips_through_db_value() {
        let content = ZcodeProviderContent {
            name: "DeepSeek".to_string(),
            category: "custom".to_string(),
            settings_config: "{\"providerId\":\"custom:deepseek\"}".to_string(),
            source_provider_id: None,
            website_url: None,
            notes: Some("note".to_string()),
            icon: None,
            icon_color: None,
            sort_index: 3,
            meta: Some(json!({ "costMultiplier": 0.5 })),
            is_disabled: false,
        };
        let value = to_db_value_provider(&content);
        assert_eq!(value["settings_config"], json!("{\"providerId\":\"custom:deepseek\"}"));
        assert_eq!(value["meta"]["costMultiplier"], json!(0.5));

        let mut with_id = value.clone();
        with_id["id"] = json!("row-1");
        with_id["is_applied"] = json!(true);
        let parsed = from_db_value_provider(with_id);
        assert_eq!(parsed.id, "row-1");
        assert_eq!(parsed.sort_index, 3);
        assert!(parsed.is_applied);
        assert_eq!(parsed.notes.as_deref(), Some("note"));
    }

    /// Managed ids carry a `custom:` prefix, which the shared `db_clean_id`
    /// strips as if it were a legacy `table:id` reference. Reading the id
    /// through it made every read-back miss the row and report "not found".
    #[test]
    fn managed_provider_id_survives_the_db_round_trip() {
        let parsed = from_db_value_provider(json!({
            "id": "custom:example-relay",
            "name": "Example Relay",
            "settings_config": "{}"
        }));
        assert_eq!(parsed.id, "custom:example-relay");
    }

    #[test]
    fn common_config_tolerates_missing_root_dir() {
        let parsed = from_db_value_common(json!({ "id": "common", "config": "{}" }));
        assert_eq!(parsed.id, "common");
        assert_eq!(parsed.root_dir, None);
    }

    /// The web layer reads these keys verbatim, so the response shape must stay
    /// camelCase. The stored shape (`ZcodeProviderContent`) is the opposite and
    /// is asserted below — the two must not be confused.
    #[test]
    fn api_records_serialize_as_camel_case() {
        let provider = ZcodeProvider {
            id: "row-1".to_string(),
            name: "DeepSeek".to_string(),
            category: "custom".to_string(),
            settings_config: "{}".to_string(),
            source_provider_id: None,
            website_url: None,
            notes: None,
            icon: None,
            icon_color: None,
            sort_index: 1,
            meta: None,
            is_applied: true,
            is_disabled: false,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
        };
        let value = serde_json::to_value(&provider).expect("serialize provider");
        assert_eq!(value["settingsConfig"], json!("{}"));
        assert_eq!(value["isApplied"], json!(true));
        assert_eq!(value["isDisabled"], json!(false));
        assert_eq!(value["sortIndex"], json!(1));
        assert_eq!(value["createdAt"], json!("t"));
        assert_eq!(value["updatedAt"], json!("t"));
        assert!(value.get("settings_config").is_none());

        let prompt = ZcodePromptConfig {
            id: "p1".to_string(),
            name: "Default".to_string(),
            content: "c".to_string(),
            is_applied: false,
            sort_index: 2,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
        };
        let value = serde_json::to_value(&prompt).expect("serialize prompt");
        assert_eq!(value["isApplied"], json!(false));
        assert_eq!(value["sortIndex"], json!(2));

        let common = ZcodeCommonConfig {
            config: "{}".to_string(),
            root_dir: Some("/tmp/zcode".to_string()),
            official_account_index: Some(2),
            updated_at: "t".to_string(),
        };
        let value = serde_json::to_value(&common).expect("serialize common");
        assert_eq!(value["rootDir"], json!("/tmp/zcode"));
        // The web layer reads camelCase; the card's slot must survive the trip.
        assert_eq!(value["officialAccountIndex"], json!(2));
    }

    /// Storage keeps snake_case, and the readers accept either spelling.
    #[test]
    fn stored_records_stay_snake_case_and_read_both_spellings() {
        let content = ZcodeProviderContent {
            name: "DeepSeek".to_string(),
            category: "custom".to_string(),
            settings_config: "{}".to_string(),
            source_provider_id: None,
            website_url: None,
            notes: None,
            icon: None,
            icon_color: None,
            sort_index: 1,
            meta: None,
            is_disabled: false,
        };
        let value = to_db_value_provider(&content);
        assert!(value.get("settings_config").is_some());
        assert!(value.get("settingsConfig").is_none());

        // A camelCase row written by an older build still parses.
        let parsed = from_db_value_provider(json!({
            "id": "row-1",
            "settingsConfig": "{\"providerId\":\"custom:x\"}",
            "isApplied": true,
            "sortIndex": 5,
        }));
        assert_eq!(parsed.settings_config, "{\"providerId\":\"custom:x\"}");
        assert!(parsed.is_applied);
        assert_eq!(parsed.sort_index, 5);
    }
}

pub fn from_db_value_official_account(value: Value) -> ZcodeOfficialAccountContent {
    ZcodeOfficialAccountContent {
        provider_id: get_str_compat(&value, "provider_id", "providerId", ""),
        name: get_str_compat(&value, "name", "name", "Unnamed Account"),
        kind: get_str_compat(&value, "kind", "kind", "oauth"),
        email: get_opt_str_compat(&value, "email", "email"),
        account_id: get_opt_str_compat(&value, "account_id", "accountId"),
        credentials_snapshot: get_str_compat(
            &value,
            "credentials_snapshot",
            "credentialsSnapshot",
            "{}",
        ),
        config_snapshot: get_opt_str_compat(&value, "config_snapshot", "configSnapshot"),
        sort_index: Some(get_i64_compat(&value, "sort_index", "sortIndex")),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        created_at: get_str_compat(&value, "created_at", "createdAt", ""),
        updated_at: get_str_compat(&value, "updated_at", "updatedAt", ""),
    }
}

pub fn to_db_value_official_account(content: &ZcodeOfficialAccountContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|error| {
        eprintln!("Failed to serialize ZCode official account content: {error}");
        json!({})
    })
}

impl ZcodeOfficialAccountContent {
    /// The account as the frontend sees it. Snapshots stay behind: the page
    /// never needs a token, and the virtual entry has no row to read.
    pub fn into_api(self, account_id: String, is_virtual: bool) -> ZcodeOfficialAccount {
        ZcodeOfficialAccount {
            id: account_id,
            provider_id: self.provider_id,
            name: self.name,
            kind: self.kind,
            email: self.email,
            account_id: self.account_id,
            is_applied: self.is_applied,
            is_virtual,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
