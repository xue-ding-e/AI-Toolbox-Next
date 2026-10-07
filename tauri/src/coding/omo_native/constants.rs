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

/// 全局提示词文件，写在引擎状态目录里（默认 `~/.omo/agent/AGENTS.md`）。
///
/// senpi 引擎的项目规则文件；OMP 的 `OMP_PROMPT_FILE` 与 Pi 的 `PI_PROMPT_FILE`
/// 同为 `AGENTS.md`，三者的「全局提示词」区块语义一致。
pub const OMO_NATIVE_PROMPT_FILE: &str = "AGENTS.md";

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

/// 引擎内建 provider id。
///
/// ⚠️ **这是引擎的事实，不是我们维护的名单**。它随 senpi 版本变化，而我们这份
/// 手抄的副本已经错过一次：2026-10-07 发现 `anthropic-subscription` /
/// `cursor-cli-oauth` 是内建却被漏掉（于是被当成自定义 provider 列进「可编辑」
/// 列表），而 `bai` / `ollama` / `typesafe` 早已不是内建却还留着。
///
/// **重新生成**（升级 senpi 后跑一次）：
/// ```bash
/// # 1. 引擎报出的全部 provider（含自定义），减掉 models.json 里的自定义
/// omo --list-models | tail -n +2 | awk '{print $1}' | sort -u
/// # 2. 但 --list-models 只列「当前有模型可列」的 provider：像原生 `cursor`
/// #    这种目录按账号发现的（docs/providers.md 的 Cursor 一节）不会出现，
/// #    要对着 docs/providers.md 手工补回。
/// ```
/// 拿不到 CLI 时这份常量就是兜底。
pub const OMO_NATIVE_BUILTIN_PROVIDERS: [&str; 48] = [
    "alibaba-token-plan",
    "amazon-bedrock",
    "ant-ling",
    "anthropic",
    "anthropic-subscription",
    "azure-openai-responses",
    "baseten",
    "cerebras",
    "chatgpt-subscription",
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "cursor",
    "cursor-cli-oauth",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 名单必须**有序且无重复**：它是手抄自引擎的，排序让「与引擎对照」变成
    /// 一次逐行 diff，而不是人工扫。
    #[test]
    fn builtin_providers_are_sorted_and_unique() {
        let mut sorted = OMO_NATIVE_BUILTIN_PROVIDERS.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            sorted, OMO_NATIVE_BUILTIN_PROVIDERS,
            "OMO_NATIVE_BUILTIN_PROVIDERS 必须按字典序排列"
        );
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(deduped.len(), sorted.len(), "OMO_NATIVE_BUILTIN_PROVIDERS 有重复项");
    }

    /// OAuth 名单必须是内建名单的子集：不是内建的 provider 不可能有 OAuth 流程。
    #[test]
    fn oauth_providers_are_a_subset_of_builtins() {
        for provider in OMO_NATIVE_OAUTH_PROVIDERS {
            assert!(
                is_builtin_provider(provider),
                "`{provider}` 在 OAuth 名单里却不在内建名单里"
            );
        }
    }

    /// 上游把 provider id 写成 kebab-case；出现下划线通常意味着手抄时看错了。
    #[test]
    fn builtin_provider_ids_use_kebab_case() {
        for provider in OMO_NATIVE_BUILTIN_PROVIDERS {
            assert!(
                !provider.contains('_') && provider == provider.to_lowercase(),
                "`{provider}` 不像引擎的 provider id（应是小写 kebab-case）"
            );
        }
    }
}
