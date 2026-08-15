//! Hermes Agent path / provider constants.

/// Environment variable used by Hermes to override its config directory.
pub const HERMES_ENV_KEY: &str = "HERMES_HOME";
/// The single YAML file Hermes reads for all its configuration.
pub const HERMES_CONFIG_FILE: &str = "config.yaml";
/// Global prompt file authored into the config directory.
///
/// Hermes reads its global prompt (personality/identity, system-prompt slot #1)
/// from `$HERMES_HOME/SOUL.md` only. `AGENTS.md` is project-scoped context
/// discovered from the working directory / git root, never from the config dir,
/// so writing a global prompt to `AGENTS.md` would have no runtime effect.
pub const HERMES_PROMPT_FILE: &str = "SOUL.md";

/// Known Hermes provider keys with official display names.
///
/// Aligned with the Hermes runtime's built-in/known providers plus the common
/// aggregators Hermes users configure (openrouter / deepseek / groq, ...).
/// This list is only used for display and built-in tagging; the authoritative
/// provider fact source is still `config.yaml`'s `custom_providers` / `providers`.
pub const HERMES_BUILTIN_PROVIDERS: [(&str, &str); 12] = [
    ("anthropic", "Anthropic"),
    ("openai", "OpenAI"),
    ("google", "Google"),
    ("openrouter", "OpenRouter"),
    ("deepseek", "DeepSeek"),
    ("groq", "Groq"),
    ("mistral", "Mistral"),
    ("nous", "Nous Research"),
    ("xai", "xAI"),
    ("github-copilot", "GitHub Copilot"),
    ("azure", "Azure"),
    ("amazon-bedrock", "Amazon Bedrock"),
];

pub fn is_builtin_provider(provider_key: &str) -> bool {
    HERMES_BUILTIN_PROVIDERS
        .iter()
        .any(|(key, _)| *key == provider_key)
}

pub fn builtin_provider_name(provider_key: &str) -> Option<&'static str> {
    HERMES_BUILTIN_PROVIDERS
        .iter()
        .find_map(|(key, name)| (*key == provider_key).then_some(*name))
}
