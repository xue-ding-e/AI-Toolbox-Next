pub const OMP_ENV_KEY: &str = "PI_CODING_AGENT_DIR";
pub const OMP_CONFIG_FILE: &str = "config.yml";
/// Legacy-compatible main config filename: OMP reads `config.yaml` only when
/// `config.yml` is absent (MAIN_CONFIG_FILENAMES = ["config.yml", "config.yaml"]).
pub const OMP_CONFIG_FILE_LEGACY: &str = "config.yaml";
pub const OMP_MODELS_FILE: &str = "models.yml";
pub const OMP_MCP_FILE: &str = "mcp.json";
pub const OMP_PROMPT_FILE: &str = "AGENTS.md";
pub const OMP_EXTENSIONS_DIR: &str = "extensions";

/// OMP 内置供应商 key 与官方显示名,对齐上游 docs/providers.md 的 CORE 列表
/// (anthropic/openai/openai-codex/google/google-vertex/groq/openrouter/mistral/
/// xai/xai-oauth/github-copilot/cursor/azure/amazon-bedrock)。grok 的官方
/// provider key 是 xai-oauth(xAI GroK OAuth),不是 `grok`。
pub const OMP_BUILTIN_PROVIDERS: [(&str, &str); 14] = [
    ("anthropic", "Anthropic"),
    ("openai", "OpenAI"),
    ("openai-codex", "OpenAI Codex"),
    ("google", "Google"),
    ("google-vertex", "Google Vertex"),
    ("groq", "Groq"),
    ("openrouter", "OpenRouter"),
    ("mistral", "Mistral"),
    ("xai", "xAI API"),
    ("xai-oauth", "xAI Grok OAuth"),
    ("github-copilot", "GitHub Copilot"),
    ("cursor", "Cursor"),
    ("azure", "Azure"),
    ("amazon-bedrock", "Amazon Bedrock"),
];

pub fn is_builtin_provider(provider_key: &str) -> bool {
    OMP_BUILTIN_PROVIDERS
        .iter()
        .any(|(key, _)| *key == provider_key)
}

pub fn builtin_provider_name(provider_key: &str) -> Option<&'static str> {
    OMP_BUILTIN_PROVIDERS
        .iter()
        .find_map(|(key, name)| (*key == provider_key).then_some(*name))
}
