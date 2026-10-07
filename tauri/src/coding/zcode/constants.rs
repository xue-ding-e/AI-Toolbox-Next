//! ZCode runtime file names and layout constants.
//!
//! ZCode keeps two generations of provider configuration side by side. The
//! presence of `v2/provider_config.json` marks the new generation; when it is
//! absent the runtime still reads the legacy `v2/config.json` `provider` map.
//! AI Toolbox only ever manages the new-generation file.

pub(crate) const ZCODE_DEFAULT_ROOT_DIR_NAME: &str = ".zcode";

/// New-generation provider registry. This is the file AI Toolbox owns a
/// subset of (by `providerId`), not the whole document.
pub(crate) const ZCODE_PROVIDER_CONFIG_RELATIVE_PATH: &str = "v2/provider_config.json";

/// Desktop settings (locale, workspace list, `dataBaseDir`, plan selection).
pub(crate) const ZCODE_SETTING_RELATIVE_PATH: &str = "v2/setting.json";

/// Official-account login state: a flat `string -> string` map whose values are
/// `enc:v1:` ciphertext. Snapshotting and restoring this file is what switching
/// accounts does.
pub(crate) const ZCODE_CREDENTIALS_RELATIVE_PATH: &str = "v2/credentials.json";

/// Legacy provider map. Only relevant before the new-generation registry
/// exists — see [`ZCODE_PROVIDER_CONFIG_RELATIVE_PATH`].
pub(crate) const ZCODE_LEGACY_CONFIG_RELATIVE_PATH: &str = "v2/config.json";

/// CLI configuration: MCP servers, hooks, feature switches.
pub(crate) const ZCODE_CLI_CONFIG_RELATIVE_PATH: &str = "cli/config.json";

/// The CLI's session and usage database.
pub(crate) const ZCODE_CLI_DB_RELATIVE_PATH: &str = "cli/db/db.sqlite";

/// Global rules file. Unlike Codex there is no `AGENTS.override.md` sibling.
pub(crate) const ZCODE_PROMPT_FILE_NAME: &str = "AGENTS.md";

/// Central skills directory.
pub(crate) const ZCODE_SKILLS_DIR_NAME: &str = "skills";

/// Desktop session transcripts: `v2/sessions/<workspaceId>/<taskId>.json`.
pub(crate) const ZCODE_SESSIONS_RELATIVE_PATH: &str = "v2/sessions";

/// Absolute data-root override honored by both the desktop app and the CLI.
pub(crate) const ZCODE_DATA_BASE_DIR_ENV: &str = "ZCODE_DATA_BASE_DIR";

/// Prefix reserved by ZCode for built-in account providers. Rules carrying it
/// reject `config.access` overrides and cannot be disabled.
pub(crate) const ZCODE_ACCOUNT_PROVIDER_ID_PREFIX: &str = "account:";

/// Prefix reserved by ZCode for providers shipped in the built-in catalog.
pub(crate) const ZCODE_BUILTIN_PROVIDER_ID_PREFIX: &str = "builtin:";

/// Namespace for providers that are not part of ZCode's built-in catalog.
///
/// ZCode itself uses this prefix to distinguish user-defined providers from
/// catalog ones, so AI Toolbox reuses it rather than inventing a namespace.
pub(crate) const ZCODE_MANAGED_PROVIDER_ID_PREFIX: &str = "custom:";

/// Personal-provider group marker required by the personal overlay schema.
pub(crate) const ZCODE_PERSONAL_PROVIDER_GROUP: &str = "standard-personal";
