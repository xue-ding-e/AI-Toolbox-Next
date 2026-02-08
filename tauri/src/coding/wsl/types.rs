use serde::{Deserialize, Serialize};

// ============================================================================
// File Mapping Types
// ============================================================================

/// File mapping API response (camelCase for frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMapping {
    pub id: String,
    pub name: String,
    pub module: String, // "opencode" | "claude" | "codex"
    pub windows_path: String,
    pub wsl_path: String,
    pub enabled: bool,
    pub is_pattern: bool,
    pub is_directory: bool,
}

// ============================================================================
// WSL Sync Config Types
// ============================================================================

/// WSL sync configuration API response (camelCase for frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WSLSyncConfig {
    pub enabled: bool,
    pub distro: String,
    /// Sync MCP configuration to WSL (default: true)
    #[serde(default = "default_true")]
    pub sync_mcp: bool,
    /// Sync Skills to WSL (default: true)
    #[serde(default = "default_true")]
    pub sync_skills: bool,
    pub file_mappings: Vec<FileMapping>,
    pub last_sync_time: Option<String>,
    pub last_sync_status: String, // "success" | "error" | "never"
    pub last_sync_error: Option<String>,
}

impl Default for WSLSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            distro: String::new(),
            sync_mcp: true,
            sync_skills: true,
            file_mappings: vec![],
            last_sync_time: None,
            last_sync_status: "never".to_string(),
            last_sync_error: None,
        }
    }
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Sync Result Types
// ============================================================================

/// Result of a sync operation (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub success: bool,
    pub synced_files: Vec<String>,
    pub skipped_files: Vec<String>,
    pub errors: Vec<String>,
}

/// WSL detection result (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WSLErrorResult {
    pub available: bool,
    pub error: Option<String>,
}

/// WSL detection result with distros (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WSLDetectResult {
    pub available: bool,
    pub distros: Vec<String>,
    pub error: Option<String>,
}

/// WSL status result (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WSLStatusResult {
    pub wsl_available: bool,
    pub last_sync_time: Option<String>,
    pub last_sync_status: String,
    pub last_sync_error: Option<String>,
}
