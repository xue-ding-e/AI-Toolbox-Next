pub const PI_ENV_KEY: &str = "PI_CODING_AGENT_DIR";
pub const PI_SETTINGS_FILE: &str = "settings.json";
pub const PI_AUTH_FILE: &str = "auth.json";
pub const PI_MODELS_FILE: &str = "models.json";
pub const PI_MCP_FILE: &str = "mcp.json";
pub const PI_PROMPT_FILE: &str = "AGENTS.md";
pub const PI_EXTENSIONS_DIR: &str = "extensions";

pub const PI_BUILTIN_PROVIDERS: [(&str, &str); 8] = [
    ("anthropic", "Anthropic"),
    ("openai", "OpenAI"),
    ("google", "Google"),
    ("openrouter", "OpenRouter"),
    ("github-copilot", "GitHub Copilot"),
    ("codex", "ChatGPT / Codex"),
    ("claude", "Claude Pro / Max"),
    ("mistral", "Mistral"),
];

pub fn is_builtin_provider(provider_key: &str) -> bool {
    PI_BUILTIN_PROVIDERS
        .iter()
        .any(|(key, _)| *key == provider_key)
}

pub fn builtin_provider_name(provider_key: &str) -> Option<&'static str> {
    PI_BUILTIN_PROVIDERS
        .iter()
        .find_map(|(key, name)| (*key == provider_key).then_some(*name))
}
