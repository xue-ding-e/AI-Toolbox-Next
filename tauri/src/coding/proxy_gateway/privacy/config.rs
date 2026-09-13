use crate::db::helpers::{db_get, db_put};
use crate::db::schema::DbTable;
use crate::db::SqliteDbState;
use serde::{Deserialize, Serialize};

const SETTINGS_ID: &str = "privacy";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    pub enabled: bool,
    pub rules: PrivacyRules,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            rules: PrivacyRules::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyRules {
    pub builtins: Vec<String>,
    pub custom: Vec<PrivacyCustomRule>,
    pub allowlist: Vec<String>,
}

impl Default for PrivacyRules {
    fn default() -> Self {
        Self {
            builtins: [
                "credentials",
                "private_keys",
                "passwords",
                "connection_strings",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            custom: Vec::new(),
            allowlist: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyCustomRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: PrivacyRuleKind,
    pub pattern: String,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyRuleKind {
    Literal,
    Regex,
}

/// Partial updates keep an open rule editor from overwriting a newer toggle.
#[derive(Debug, Deserialize)]
pub struct PrivacySettingsUpdate {
    pub enabled: Option<bool>,
    pub rules: Option<PrivacyRules>,
}

pub fn load_settings(db: &SqliteDbState) -> Result<PrivacySettings, String> {
    db.with_conn(
        |conn| match db_get(conn, DbTable::ProxyGatewaySettings, SETTINGS_ID)? {
            Some(value) => serde_json::from_value(value).map_err(|_| {
                "privacy_config_invalid: saved privacy settings are invalid".to_string()
            }),
            None => Ok(PrivacySettings::default()),
        },
    )
}

pub(super) fn save_settings(db: &SqliteDbState, settings: &PrivacySettings) -> Result<(), String> {
    let value = serde_json::to_value(settings).map_err(|_| "privacy_config_invalid".to_string())?;
    db.with_conn(|conn| db_put(conn, DbTable::ProxyGatewaySettings, SETTINGS_ID, &value))
}
