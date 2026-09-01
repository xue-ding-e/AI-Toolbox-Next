pub const KIMI_HOME_ENV_KEY: &str = "KIMI_CODE_HOME";
pub const KIMI_LOCAL_PROVIDER_ID: &str = "__local__";
pub const KIMI_CONFIG_FILE: &str = "config.toml";
pub const KIMI_PROMPT_FILE: &str = "AGENTS.md";
pub const KIMI_SKILLS_DIR: &str = "skills";
pub const KIMI_PLUGINS_DIR: &str = "plugins";
pub const KIMI_SESSIONS_DIR: &str = "sessions";
pub const KIMI_CREDENTIALS_DIR: &str = "credentials";
pub const KIMI_OFFICIAL_API_BASE_URL: &str = "https://api.kimi.com/coding/v1";

/// Official channel default model, matching what the real Kimi CLI projects:
/// catalog key `kimi-code/kimi-for-coding` -> model id `kimi-for-coding`.
pub const KIMI_OFFICIAL_DEFAULT_MODEL_KEY: &str = "kimi-code/kimi-for-coding";
pub const KIMI_OFFICIAL_DEFAULT_MODEL_ID: &str = "kimi-for-coding";
pub const KIMI_OFFICIAL_DEFAULT_MODEL_DISPLAY_NAME: &str = "K2.7 Coding";
/// Conservative official per-model context size; the CLI hard-requires a
/// positive `max_context_size` on every projected model.
pub const KIMI_DEFAULT_MODEL_MAX_CONTEXT_SIZE: i64 = 262_144;