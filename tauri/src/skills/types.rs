use serde::{Deserialize, Serialize};

/// Skill record stored in SurrealDB
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub source_type: String, // "local" | "git" | "import"
    pub source_ref: Option<String>,
    pub source_revision: Option<String>,
    pub central_path: String,
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_sync_at: Option<i64>,
    pub status: String,
}

/// Skill target record - tracks where a skill is synced
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillTarget {
    pub id: String,
    pub skill_id: String,
    pub tool: String,
    pub target_path: String,
    pub mode: String, // "symlink" | "copy" | "junction"
    pub status: String,
    pub synced_at: Option<i64>,
    pub error_message: Option<String>,
}

/// Skills settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillSettings {
    pub central_repo_path: String,
    pub git_cache_cleanup_days: i32,
    pub git_cache_ttl_secs: i32,
    pub known_tool_versions: Option<serde_json::Value>,
    pub updated_at: i64,
}

impl Default for SkillSettings {
    fn default() -> Self {
        Self {
            central_repo_path: dirs::home_dir()
                .map(|p| p.join(".skills").to_string_lossy().to_string())
                .unwrap_or_default(),
            git_cache_cleanup_days: 30,
            git_cache_ttl_secs: 60,
            known_tool_versions: None,
            updated_at: 0,
        }
    }
}

/// Tool detection status
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDetection {
    pub tool: String,
    pub installed: bool,
    pub skills_dir: Option<String>,
    pub detected_at: i64,
    pub first_seen_at: Option<i64>,
}

/// DTO for tool status response
#[derive(Debug, Serialize)]
pub struct ToolStatusDto {
    pub tools: Vec<ToolInfoDto>,
    pub installed: Vec<String>,
    pub newly_installed: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ToolInfoDto {
    pub key: String,
    pub label: String,
    pub installed: bool,
}

/// DTO for managed skills
#[derive(Debug, Serialize)]
pub struct ManagedSkillDto {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub central_path: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_sync_at: Option<i64>,
    pub status: String,
    pub targets: Vec<SkillTargetDto>,
}

#[derive(Debug, Serialize)]
pub struct SkillTargetDto {
    pub tool: String,
    pub mode: String,
    pub status: String,
    pub target_path: String,
    pub synced_at: Option<i64>,
}

/// DTO for install result
#[derive(Debug, Serialize)]
pub struct InstallResultDto {
    pub skill_id: String,
    pub name: String,
    pub central_path: String,
    pub content_hash: Option<String>,
}

/// DTO for sync result
#[derive(Debug, Serialize)]
pub struct SyncResultDto {
    pub mode_used: String,
    pub target_path: String,
}

/// DTO for update result
#[derive(Debug, Serialize)]
pub struct UpdateResultDto {
    pub skill_id: String,
    pub name: String,
    pub content_hash: Option<String>,
    pub source_revision: Option<String>,
    pub updated_targets: Vec<String>,
}

/// Git skill candidate for multi-skill repos
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitSkillCandidate {
    pub name: String,
    pub description: Option<String>,
    pub subpath: String,
}

/// Onboarding plan for discovered skills
#[derive(Clone, Debug, Serialize)]
pub struct OnboardingPlan {
    pub total_tools_scanned: usize,
    pub total_skills_found: usize,
    pub groups: Vec<OnboardingGroup>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingGroup {
    pub name: String,
    pub variants: Vec<OnboardingVariant>,
    pub has_conflict: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingVariant {
    pub tool: String,
    pub name: String,
    pub path: String,
    pub fingerprint: Option<String>,
    pub is_link: bool,
    pub link_target: Option<String>,
}

/// Internal struct for install operations
pub struct InstallResult {
    pub skill_id: String,
    pub name: String,
    pub central_path: std::path::PathBuf,
    pub content_hash: Option<String>,
}

/// Internal struct for update operations
pub struct UpdateResult {
    pub skill_id: String,
    pub name: String,
    pub central_path: std::path::PathBuf,
    pub content_hash: Option<String>,
    pub source_revision: Option<String>,
    pub updated_targets: Vec<String>,
}

/// Sync mode used for skill syncing
#[derive(Clone, Debug)]
pub enum SyncMode {
    Auto,
    Symlink,
    Junction,
    Copy,
}

impl SyncMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncMode::Auto => "auto",
            SyncMode::Symlink => "symlink",
            SyncMode::Junction => "junction",
            SyncMode::Copy => "copy",
        }
    }
}

/// Sync outcome from sync operations
#[derive(Clone, Debug)]
pub struct SyncOutcome {
    pub mode_used: SyncMode,
    pub target_path: std::path::PathBuf,
    pub replaced: bool,
}

/// Detected skill in a tool directory
#[derive(Clone, Debug)]
pub struct DetectedSkill {
    pub tool: String,
    pub name: String,
    pub path: std::path::PathBuf,
    pub is_link: bool,
    pub link_target: Option<std::path::PathBuf>,
}

/// Helper function to get current timestamp in milliseconds
pub fn now_ms() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}
