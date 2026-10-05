//! OmO Native 常量。
//!
//! 上游 = `code-yeongyu/oh-my-openagent`（本地 `D:\GitHub\oh-my-opencode`），
//! Native edition = `packages/omo-native`（npm 包名 `omo-ai`，二进制 `omo`）。
//! 名单与路径语义核实于 tag `v5.1.19`（本机安装版本）。
//!
//! **不要**把这里的名单与 `oh_my_openagent`（OpenCode 插件版）的名单混用：
//! 5.0 起一个仓库里是两套 agent 名单不相交的 edition，插件版保留神话命名
//! （sisyphus/hephaestus/...），Native 已去掉。

/// 引擎状态目录的环境变量覆盖，优先级从高到低。
/// 上游 `packages/omo-native/bin/lib/agent-dir.js` `AGENT_DIR_ENV_NAMES`
/// 与 `packages/omo-senpi/src/components/agent-home/resolve-agent-home.ts` 一致。
pub const OMO_NATIVE_AGENT_DIR_ENV_KEYS: [&str; 3] = [
    "OMO_CODING_AGENT_DIR",
    "SENPI_CODING_AGENT_DIR",
    "PI_CODING_AGENT_DIR",
];

/// 用户级统一配置文件所在目录（相对 home）。
pub const OMO_NATIVE_CONFIG_DIR: &str = ".omo";

/// 统一配置文件基名；`.jsonc` 优先，`.json` 为回退。
pub const OMO_NATIVE_CONFIG_BASENAME: &str = "omo";
pub const OMO_NATIVE_CONFIG_FILE: &str = "omo.jsonc";
pub const OMO_NATIVE_CONFIG_FILE_FALLBACK: &str = "omo.json";

/// Native harness 块在统一配置里的键名。插件版读 `[opencode]`，两版互不干扰。
pub const OMO_NATIVE_HARNESS_BLOCK: &str = "[native]";

/// 已退役的 harness 块拼写（上游 `OMO_CONFIG_LEGACY_HARNESS_ALIASES = { senpi: "native" }`）。
/// 读取时要一并折叠，写入只用 `[native]`。
pub const OMO_NATIVE_HARNESS_BLOCK_LEGACY: &str = "[senpi]";

/// 引擎状态目录（`~/.omo/agent`）内的文件名。
pub const OMO_NATIVE_SETTINGS_FILE: &str = "settings.json";
pub const OMO_NATIVE_SETTINGS_FILE_FALLBACK: &str = "settings.jsonc";
pub const OMO_NATIVE_AUTH_FILE: &str = "auth.json";
pub const OMO_NATIVE_MODELS_FILE: &str = "models.json";
pub const OMO_NATIVE_MCP_FILE: &str = "mcp.json";
pub const OMO_NATIVE_SKILLS_DIR: &str = "skills";
pub const OMO_NATIVE_SESSIONS_DIR: &str = "sessions";

/// pre-unification 扁平布局的哨兵文件：`~/.omo/settings.json` 存在说明是旧布局
/// （上游 `resolve-agent-home.ts` `AGENT_HOME_SENTINEL`）。
pub const OMO_NATIVE_AGENT_HOME_SENTINEL: &str = "settings.json";

/// Native 内建 curated agents，核实于 `packages/senpi-task/src/agents/builtin/*.ts`（v5.1.19）。
///
/// 前四个是 curated read-only agents（`explore`/`librarian` 零配置可spawn，
/// `plan-consultant`/`plan-reviewer` 受 ulw-plan 门控）；后三个是 ulw-loop reviewer。
/// `metis`/`momus` 作为 `plan-consultant`/`plan-reviewer` 别名的窗口已在
/// 5.0.0-beta.51 后关闭——现在写这两个名字只会定义出一个新的自定义 agent。
pub const OMO_NATIVE_AGENTS: [&str; 7] = [
    "explore",
    "librarian",
    "plan-consultant",
    "plan-reviewer",
    "omo-native-code-reviewer",
    "omo-native-gate-reviewer",
    "omo-native-qa-executor",
];

/// Native 内建 categories，核实于 `packages/senpi-task/src/category/*-categories.ts`（v5.1.19）。
///
/// 与插件版的差异：Native 独有 `architect`；`deep` 已由上游
/// `2026-09-category-deep-split` 拆成 `deep-low`（默认深车道）/`deep-high`（升级车道），
/// 裸 `deep` 作为别名在加载时规范化，新配置应写拆分后的名字。
pub const OMO_NATIVE_CATEGORIES: [&str; 10] = [
    "architect",
    "visual-engineering",
    "artistry",
    "writing",
    "ultrabrain",
    "deep-low",
    "deep-high",
    "quick",
    "unspecified-low",
    "unspecified-high",
];

/// `[native]` 块的合法键，核实于 `packages/omo-config-core/src/schema/config.ts`
/// 的 `OmoTypedHarnessConfigSchema`（`.strict()`，未知键会触发上游 unknown-key diagnostic）。
/// 只写这些键；`[opencode]` 的键（`disabled_agents`/`sisyphus_agent`/`claude_code` 等）
/// 写进来会被上游拒绝。
pub const OMO_NATIVE_BLOCK_KEYS: [&str; 14] = [
    "formatOnMutation",
    "gateway",
    "categories",
    "agents",
    "git_master",
    "task",
    "teams",
    "models",
    "model_profiles",
    "model_profile",
    "memory",
    "telemetry",
    "computer",
    "disabled_skills",
];

/// 引擎内建 provider id，来自 `packages/omo-native/bin/lib/provider-map.json`
/// 的 `builtinProviderIds`（v5.1.19，49 个）。该表随 senpi pin 变化，
/// 升级上游时用 `provider-map.json` 重新生成。
pub const OMO_NATIVE_BUILTIN_PROVIDERS: [&str; 49] = [
    "alibaba-token-plan",
    "amazon-bedrock",
    "ant-ling",
    "anthropic",
    "azure-openai-responses",
    "bai",
    "baseten",
    "cerebras",
    "chatgpt-subscription",
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "cursor",
    "deepseek",
    "devin",
    "fireworks",
    "github-copilot",
    "google",
    "google-vertex",
    "groq",
    "huggingface",
    "kimi-coding",
    "meta",
    "minimax",
    "minimax-cn",
    "mistral",
    "moonshotai",
    "moonshotai-cn",
    "nvidia",
    "ollama",
    "openai",
    "opencode",
    "opencode-go",
    "opengateway",
    "openrouter",
    "qwen-token-plan",
    "qwen-token-plan-cn",
    "qwen-token-plan-individual",
    "radius",
    "together",
    "typesafe",
    "venice",
    "vercel-ai-gateway",
    "xai",
    "xiaomi",
    "xiaomi-token-plan-ams",
    "xiaomi-token-plan-cn",
    "xiaomi-token-plan-sgp",
    "zai",
    "zai-coding-cn",
];

/// 支持 OAuth 登录的 provider（`provider-map.json` 的 `oauthProviderIds`）。
/// 这些 provider 不能只用 API key，需要用户在 omo 内 `/login <provider>`。
pub const OMO_NATIVE_OAUTH_PROVIDERS: [&str; 10] = [
    "anthropic",
    "chatgpt-subscription",
    "cursor",
    "devin",
    "github-copilot",
    "kimi-coding",
    "meta",
    "openrouter",
    "radius",
    "xai",
];

/// 上游内建的模型档 id（`packages/omo-senpi/src/components/model-profile/builtin-profiles.ts`，v5.1.19）。
/// `recommended` 是「未设置时的隐式默认」，不是一条车道。
pub const OMO_NATIVE_BUILTIN_MODEL_PROFILE_IDS: [(&str, &str); 5] = [
    ("recommended", "Recommended (unset default)"),
    ("daily-normal", "Daily · Normal"),
    ("daily-heavy", "Daily · Heavy"),
    ("geeky-normal", "Geeky · Normal"),
    ("geeky-heavy", "Geeky · Heavy"),
];

/// `models.json` 的 provider 条目里，模型条目的合法 `api` 值。
/// 上游 `packages/omo-native/bin/lib/setup-opencode-providers.js` `API_BY_NPM`
/// 映射出的引擎 api id（pi-ai `compat.js` `BUILTIN_APIS`）。
pub const OMO_NATIVE_API_VALUES: [&str; 3] = [
    "openai-completions",
    "anthropic-messages",
    "openai-responses",
];

pub fn is_builtin_provider(provider_key: &str) -> bool {
    OMO_NATIVE_BUILTIN_PROVIDERS.contains(&provider_key)
}

pub fn is_oauth_provider(provider_key: &str) -> bool {
    OMO_NATIVE_OAUTH_PROVIDERS.contains(&provider_key)
}
