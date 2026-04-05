use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginRuntimeStatus {
    pub mode: String,
    pub source: String,
    pub root_dir: String,
    pub config_path: String,
    pub plugins_dir: String,
    #[serde(default)]
    pub plugins_feature_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curated_marketplace_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux_root_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginMarketplace {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub plugin_count: usize,
    #[serde(default)]
    pub is_curated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMarketplacePlugin {
    pub plugin_id: String,
    pub marketplace_name: String,
    pub marketplace_path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub install_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexInstalledPlugin {
    pub plugin_id: String,
    pub marketplace_name: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_version: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub has_skills: bool,
    #[serde(default)]
    pub has_mcp_servers: bool,
    #[serde(default)]
    pub has_apps: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginActionInput {
    pub plugin_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginWorkspaceRoot {
    pub path: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_marketplace_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_repo_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginWorkspaceRootInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRefreshCuratedPluginsInput {
    #[serde(default)]
    pub force: bool,
}
