use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 运行时根目录信息（`~/.omo/agent` 或自定义），source 为
/// `custom` / `env` / `default`，与 `oh_my_pi` 的 `OmpPathInfo` 语义一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativePathInfo {
    pub path: String,
    pub source: String,
}

/// Native 的运行时根目录设置（DB 表 `omo_native_settings_config` 的 `common` 记录）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeSettingsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_dir: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeSettingsConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_dir: Option<String>,
    #[serde(default)]
    pub clear_root_dir: bool,
}

/// 二进制检测结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeCliInfo {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `omo --version` 的原始输出，例如 `omo 5.1.19 (engine: senpi 2026.10.10; scheme nodef)`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 从 version 输出里解析出的产品版本号，例如 `5.1.19`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_number: Option<String>,
    /// 引擎版本，例如 `2026.10.10`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Agent/Category 配置方案（DB 表 `omo_native_agents_config`）。
///
/// 只承载 `[native]` 块内的 `agents` / `categories` / `model_profiles` /
/// `model_profile` / `task` 五个键——`other_fields` 用于保留用户在方案里
/// 手写的其他 Native 合法键，避免保存时被吞掉。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeAgentsConfig {
    pub id: String,
    pub name: String,
    pub is_applied: bool,
    pub is_disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_profiles: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// 写入用的方案内容（`id`/时间戳由后端生成）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeAgentsConfigContent {
    pub name: String,
    pub is_applied: bool,
    pub is_disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_profiles: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeAgentsConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub is_applied: bool,
    #[serde(default)]
    pub is_disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_profiles: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
}

/// `read_omo_native_runtime_config` 的返回：当前生效视图 + 各文件原文。
///
/// `effective` 是**折叠后的 Native 生效视图**（共享 base 键被 `[native]` 块覆盖后的结果），
/// 因为上游 `resolveOmoConfigView` 是 base → harness 块 → profile → profile 的 harness 块，
/// 后者胜。只展示块内会让界面与实际运行不一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeRuntimeConfig {
    /// Native harness 生效视图（折叠后）。
    pub effective: Value,
    /// `[native]` 块原文（未折叠）。
    pub native_block: Value,
    /// 共享 base 层里被 Native 视图采纳的键（顶层非控制键）。
    pub shared_base: Value,
    /// 统一配置文件路径。
    pub config_path: String,
    /// 配置是否可解析。false 时 `parse_error` 有值。
    pub parsed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    /// `<agentDir>/settings.json` 原文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_content: Option<String>,
    /// `<agentDir>/models.json` 原文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models_content: Option<String>,
    /// `<agentDir>/mcp.json` 原文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_content: Option<String>,
}

/// 模型档（`model_profile` / `model_profiles`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeModelProfile {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// `daily` / `geeky`，纯元数据。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// `normal` / `heavy`，纯元数据。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Value>,
    /// 是否是上游内建档（内建档可被同名覆盖，覆盖是整体替换不是逐字段合并）。
    #[serde(default)]
    pub builtin: bool,
}

/// 一个 provider 在 `models.json` 里的条目 + 其密钥状态。
///
/// `models.json` 没有对外发布的 schema，字段形状反推自上游
/// `packages/omo-native/bin/lib/setup-opencode-providers.js` 的 `convertProvider`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeProvider {
    pub key: String,
    /// 是否引擎内建（内建 provider 不该在 `models.json` 里重复定义）。
    pub builtin: bool,
    /// 是否需要 OAuth 登录而非 API key。
    pub oauth: bool,
    /// `models.json` 里该 provider 的完整条目（含未知字段）。
    pub config: Value,
    /// `auth.json` 里是否已有该 provider 的密钥。
    pub has_key: bool,
    /// 是否在 omo 的自定义 provider 表里（`models.json` 的 `providers` 键下）。
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeProviderInput {
    pub key: String,
    pub config: Value,
    /// 可选：同时写入 `auth.json`。空字符串表示不动密钥。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// MCP server 条目（`<agentDir>/mcp.json` 的 `mcpServers` 记录）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeMcpServer {
    pub name: String,
    pub config: Value,
}

/// 一个 skill 目录（`<agentDir>/skills/<name>`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoNativeSkill {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
