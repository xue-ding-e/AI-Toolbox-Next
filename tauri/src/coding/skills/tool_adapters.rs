//! Tool adapters for Skills module
//!
//! This module provides backward-compatible tool adapter functionality for the Skills feature.
//! It wraps the shared tools module and provides Skills-specific types and functions.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::coding::tools::{self, BUILTIN_TOOLS};

/// Legacy CustomTool type for backward compatibility with Skills
/// This type has required fields while the new tools::CustomTool has optional fields
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CustomTool {
    pub key: String,
    pub display_name: String,
    pub relative_skills_dir: String,
    pub relative_detect_dir: String,
    pub created_at: i64,
}

/// Convert from shared CustomTool to skills CustomTool
impl From<tools::CustomTool> for CustomTool {
    fn from(tool: tools::CustomTool) -> Self {
        CustomTool {
            key: tool.key,
            display_name: tool.display_name,
            relative_skills_dir: tool.relative_skills_dir.unwrap_or_default(),
            relative_detect_dir: tool.relative_detect_dir.unwrap_or_default(),
            created_at: tool.created_at,
        }
    }
}

/// Convert from skills CustomTool to shared CustomTool
impl From<&CustomTool> for tools::CustomTool {
    fn from(tool: &CustomTool) -> Self {
        tools::CustomTool {
            key: tool.key.clone(),
            display_name: tool.display_name.clone(),
            relative_skills_dir: Some(tool.relative_skills_dir.clone()),
            relative_detect_dir: Some(tool.relative_detect_dir.clone()),
            mcp_config_path: None,
            mcp_config_format: None,
            mcp_field: None,
            created_at: tool.created_at,
        }
    }
}

/// Tool ID enum for all supported AI coding tools
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolId {
    Cursor,
    ClaudeCode,
    Codex,
    OpenCode,
    Antigravity,
    Amp,
    KiloCode,
    RooCode,
    Goose,
    GeminiCli,
    GithubCopilot,
    Clawdbot,
    Droid,
    Windsurf,
}

impl ToolId {
    pub fn as_key(&self) -> &'static str {
        match self {
            ToolId::Cursor => "cursor",
            ToolId::ClaudeCode => "claude_code",
            ToolId::Codex => "codex",
            ToolId::OpenCode => "opencode",
            ToolId::Antigravity => "antigravity",
            ToolId::Amp => "amp",
            ToolId::KiloCode => "kilo_code",
            ToolId::RooCode => "roo_code",
            ToolId::Goose => "goose",
            ToolId::GeminiCli => "gemini_cli",
            ToolId::GithubCopilot => "github_copilot",
            ToolId::Clawdbot => "clawdbot",
            ToolId::Droid => "droid",
            ToolId::Windsurf => "windsurf",
        }
    }

    pub fn from_key(key: &str) -> Option<ToolId> {
        match key {
            "cursor" => Some(ToolId::Cursor),
            "claude_code" => Some(ToolId::ClaudeCode),
            "codex" => Some(ToolId::Codex),
            "opencode" => Some(ToolId::OpenCode),
            "antigravity" => Some(ToolId::Antigravity),
            "amp" => Some(ToolId::Amp),
            "kilo_code" => Some(ToolId::KiloCode),
            "roo_code" => Some(ToolId::RooCode),
            "goose" => Some(ToolId::Goose),
            "gemini_cli" => Some(ToolId::GeminiCli),
            "github_copilot" => Some(ToolId::GithubCopilot),
            "clawdbot" => Some(ToolId::Clawdbot),
            "droid" => Some(ToolId::Droid),
            "windsurf" => Some(ToolId::Windsurf),
            _ => None,
        }
    }
}

/// Tool adapter with path information (legacy type for compatibility)
#[derive(Clone, Debug)]
pub struct ToolAdapter {
    pub id: ToolId,
    pub display_name: &'static str,
    pub relative_skills_dir: &'static str,
    pub relative_detect_dir: &'static str,
}

/// Get all default tool adapters (built-in tools that support Skills)
pub fn default_tool_adapters() -> Vec<ToolAdapter> {
    BUILTIN_TOOLS
        .iter()
        .filter(|t| t.relative_skills_dir.is_some())
        .filter_map(|t| {
            let id = ToolId::from_key(t.key)?;
            Some(ToolAdapter {
                id,
                display_name: t.display_name,
                relative_skills_dir: t.relative_skills_dir?,
                relative_detect_dir: t.relative_detect_dir?,
            })
        })
        .collect()
}

/// Find adapter by key
pub fn adapter_by_key(key: &str) -> Option<ToolAdapter> {
    default_tool_adapters()
        .into_iter()
        .find(|adapter| adapter.id.as_key() == key)
}

/// Resolve default skills path for a tool
pub fn resolve_default_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(adapter.relative_skills_dir).components().collect())
}

/// Resolve detect path for a tool
pub fn resolve_detect_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(adapter.relative_detect_dir).components().collect())
}

/// Check if a tool is installed
pub fn is_tool_installed(adapter: &ToolAdapter) -> Result<bool> {
    Ok(resolve_detect_path(adapter)?.exists())
}

/// Runtime tool adapter (can be built-in or custom)
#[derive(Clone, Debug)]
pub struct RuntimeToolAdapter {
    pub key: String,
    pub display_name: String,
    pub relative_skills_dir: String,
    pub relative_detect_dir: String,
    pub is_custom: bool,
}

impl From<&ToolAdapter> for RuntimeToolAdapter {
    fn from(adapter: &ToolAdapter) -> Self {
        RuntimeToolAdapter {
            key: adapter.id.as_key().to_string(),
            display_name: adapter.display_name.to_string(),
            relative_skills_dir: adapter.relative_skills_dir.to_string(),
            relative_detect_dir: adapter.relative_detect_dir.to_string(),
            is_custom: false,
        }
    }
}

impl From<&CustomTool> for RuntimeToolAdapter {
    fn from(tool: &CustomTool) -> Self {
        RuntimeToolAdapter {
            key: tool.key.clone(),
            display_name: tool.display_name.clone(),
            relative_skills_dir: tool.relative_skills_dir.clone(),
            relative_detect_dir: tool.relative_detect_dir.clone(),
            is_custom: true,
        }
    }
}

/// Get all tool adapters (built-in + custom)
pub fn get_all_tool_adapters(custom_tools: &[CustomTool]) -> Vec<RuntimeToolAdapter> {
    let mut adapters: Vec<RuntimeToolAdapter> = default_tool_adapters()
        .iter()
        .map(RuntimeToolAdapter::from)
        .collect();

    for tool in custom_tools {
        adapters.push(RuntimeToolAdapter::from(tool));
    }

    adapters
}

/// Find adapter by key (supports both built-in and custom)
pub fn runtime_adapter_by_key(key: &str, custom_tools: &[CustomTool]) -> Option<RuntimeToolAdapter> {
    // Check built-in first
    if let Some(adapter) = adapter_by_key(key) {
        return Some(RuntimeToolAdapter::from(&adapter));
    }
    // Check custom tools
    custom_tools
        .iter()
        .find(|t| t.key == key)
        .map(RuntimeToolAdapter::from)
}

/// Check if a runtime tool is installed
pub fn is_runtime_tool_installed(adapter: &RuntimeToolAdapter) -> Result<bool> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(&adapter.relative_detect_dir).exists())
}

/// Resolve skills path for a runtime tool
pub fn resolve_runtime_skills_path(adapter: &RuntimeToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(&adapter.relative_skills_dir).components().collect())
}

/// Scan a tool directory for skills
pub fn scan_tool_dir(adapter: &ToolAdapter, dir: &Path) -> Result<Vec<super::types::DetectedSkill>> {
    let mut results = Vec::new();
    if !dir.exists() {
        return Ok(results);
    }

    // Ignore paths containing our central repo
    let ignore_hint = "Application Support/com.ai-toolbox/skills";

    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {:?}", dir))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let is_dir = file_type.is_dir() || (file_type.is_symlink() && path.is_dir());
        if !is_dir {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        // Skip system directories
        if adapter.id == ToolId::Codex && name == ".system" {
            continue;
        }

        let (is_link, link_target) = detect_link(&path);
        if path.to_string_lossy().contains(ignore_hint)
            || link_target
                .as_ref()
                .map(|p| p.to_string_lossy().contains(ignore_hint))
                .unwrap_or(false)
        {
            continue;
        }

        results.push(super::types::DetectedSkill {
            tool: adapter.id.as_key().to_string(),
            name,
            path,
            is_link,
            link_target,
        });
    }

    Ok(results)
}

fn detect_link(path: &Path) -> (bool, Option<PathBuf>) {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = std::fs::read_link(path).ok();
            (true, target)
        }
        _ => {
            let target = std::fs::read_link(path).ok();
            if target.is_some() {
                (true, target)
            } else {
                (false, None)
            }
        }
    }
}
