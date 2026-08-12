pub const OMP_ENV_KEY: &str = "PI_CODING_AGENT_DIR";
pub const OMP_CONFIG_FILE: &str = "config.yml";
pub const OMP_MODELS_FILE: &str = "models.yml";
pub const OMP_MCP_FILE: &str = "mcp.json";
pub const OMP_PROMPT_FILE: &str = "AGENTS.md";
pub const OMP_EXTENSIONS_DIR: &str = "extensions";

/// OMP 内置供应商(与 Pi 不同,OMP 的目录更大;这里取常用子集用于"内置"标记与显示名)。
pub const OMP_BUILTIN_PROVIDERS: [(&str, &str); 10] = [
    ("anthropic", "Anthropic"),
    ("openai", "OpenAI"),
    ("google", "Google"),
    ("openrouter", "OpenRouter"),
    ("github-copilot", "GitHub Copilot"),
    ("codex", "ChatGPT / Codex"),
    ("claude", "Claude Pro / Max"),
    ("mistral", "Mistral"),
    ("grok", "Grok"),
    ("deepseek", "DeepSeek"),
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