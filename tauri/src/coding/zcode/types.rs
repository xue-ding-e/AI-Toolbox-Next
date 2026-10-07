use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A ZCode provider as returned to the frontend.
///
/// Serialized as camelCase because the web layer consumes these keys directly;
/// storage uses [`ZcodeProviderContent`], which stays snake_case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProvider {
    pub id: String,
    pub name: String,
    pub category: String,
    pub settings_config: String,
    pub source_provider_id: Option<String>,
    pub website_url: Option<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub sort_index: i64,
    pub meta: Option<Value>,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// The subset of a provider record that is written back to storage.
///
/// Deliberately snake_case: these keys are the stored JSONB shape, and the
/// adapter's compatibility readers accept either spelling on the way back in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZcodeProviderContent {
    pub name: String,
    pub category: String,
    pub settings_config: String,
    pub source_provider_id: Option<String>,
    pub website_url: Option<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub sort_index: i64,
    pub meta: Option<Value>,
    pub is_disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProviderInput {
    pub id: Option<String>,
    pub name: String,
    pub category: String,
    pub settings_config: String,
    pub source_provider_id: Option<String>,
    pub website_url: Option<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub is_disabled: Option<bool>,
    pub meta: Option<Value>,
}

/// Which `modelConfigRules` array a model row belongs to.
///
/// The two arrays are mutually exclusive for a given `(providerId, modelId)`
/// pair, so every row must declare its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZcodeModelRuleKind {
    /// Partial overlay over the built-in catalog. Every field is optional;
    /// an omitted field inherits from the catalog.
    Smart,
    /// Complete model definition. ZCode requires a fixed set of fields.
    Manual,
}

impl ZcodeModelRuleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Smart => "smart",
            Self::Manual => "manual",
        }
    }
}

/// Model input modalities. ZCode spells the PDF flag `supportsPdf`.
///
/// Note: ZCode's *manual* model rule only carries `supportsImage`,
/// `supportsVideo`, and `supportsPdf` here — `supportsText` and `supportsAudio`
/// belong to the smart overlay. See [`ZcodeModelRow::is_manual`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeModelInputFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_image: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_video: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_audio: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_pdf: Option<bool>,
}

/// Model output modalities. Only text is expressible today.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeModelOutputFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_text: Option<bool>,
}

/// `optionSpecs.maxOutputTokens`. `map` is a mapping *expression string*, not a
/// JSON object, and must round-trip byte for byte.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeMaxOutputTokensSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
}

/// `optionSpecs.reasoningLevel`. `values` is ordered low to high.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeReasoningLevelSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeModelOptionSpecs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<ZcodeMaxOutputTokensSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_level: Option<ZcodeReasoningLevelSpec>,
}

/// Model capability flags. `None` means "inherit from the catalog"; an explicit
/// `false` pins the capability off and must survive round trips.
///
/// The two rule kinds accept different subsets, and both are `.strict()`:
///
/// | field | smart | manual |
/// |---|---|---|
/// | `contextWindow` | ✓ | ✓ |
/// | `supportsJsonSchemaOutput` | ✓ | ✓ |
/// | `supportsNativeWebSearch` | ✓ | ✓ |
/// | `supportsMidConversationSystem` | ✓ | ✓ |
/// | `inputFormat.{supportsImage,supportsVideo,supportsPdf}` | ✓ | ✓ |
/// | `inputFormat.supportsText` / `supportsAudio` | ✓ | ✗ |
/// | `outputFormat` | ✓ | ✗ |
/// | `supportsToolCall` | ✓ | ✗ |
/// | `requiresMfjsToolSchema` | ✓ | ✗ |
///
/// Writing an unsupported field into a manual rule makes ZCode reject the whole
/// file, so the projection layer must filter by rule kind.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeModelProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_format: Option<ZcodeModelInputFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<ZcodeModelOutputFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_tool_call: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_json_schema_output: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_native_web_search: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_mid_conversation_system: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_mfjs_tool_schema: Option<bool>,
}

/// One editable model row, mirrored from the ZCode "edit model" dialog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeModelRow {
    pub model_id: String,
    /// Display-only label kept by AI Toolbox; ZCode keys models by `modelId`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub rule_kind: ZcodeModelRuleKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<ZcodeModelProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_specs: Option<ZcodeModelOptionSpecs>,
    /// Marks the model written into `defaultModelSelection` when applied.
    #[serde(default)]
    pub is_default: bool,
}

/// Provider credentials. ZCode validates this with a `.strict()` schema that
/// accepts exactly these three keys — adding another field makes ZCode reject
/// the entire provider config file, so this struct must stay in lockstep.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProviderAccess {
    /// `api-key` or `zhipu-coding-plan-api-key`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_management_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProviderApi {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProviderLogo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<ZcodeProviderLogo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<ZcodeProviderAccess>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<ZcodeProviderApi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personal_model_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_order: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
}

/// The AI Toolbox-owned payload inside a provider's `settings_config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeSettingsConfig {
    /// Stable id written to `providerRules[].providerId`.
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ZcodeProviderConfig>,
    #[serde(default)]
    pub models: Vec<ZcodeModelRow>,
    /// Model applied as `defaultModelSelection.modelId`. Falls back to the
    /// first row when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model_id: Option<String>,
}

/// Stored shape of the common-config record; see [`ZcodeProviderContent`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZcodeCommonConfigRecord {
    pub id: String,
    pub config: String,
    pub root_dir: Option<String>,
    /// Where the official-account card sits in the provider list, counted as
    /// the number of provider cards above it. UI state, not ZCode state — it
    /// lives here because this record is the module's only singleton row.
    pub official_account_index: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeCommonConfig {
    pub config: String,
    pub root_dir: Option<String>,
    pub official_account_index: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeCommonConfigInput {
    pub config: String,
    pub root_dir: Option<String>,
    pub clear_root_dir: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPathInfo {
    pub path: String,
    pub source: String,
}

/// One provider template discovered in the installed ZCode built-in catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProviderTemplate {
    pub template_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodePromptConfigInput {
    pub id: Option<String>,
    pub name: String,
    pub content: String,
}

/// A ZCode prompt preset as returned to the frontend (camelCase keys).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodePromptConfig {
    pub id: String,
    pub name: String,
    pub content: String,
    pub is_applied: bool,
    pub sort_index: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// An official-account snapshot as returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeOfficialAccount {
    pub id: String,
    /// OAuth namespace the snapshot logs into: `zai` or `bigmodel`.
    pub provider_id: String,
    pub name: String,
    /// `oauth` for a saved snapshot, `local` for the virtual entry that mirrors
    /// whatever is logged in right now without having been saved.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// The provider's own user id, used to recognise the same login again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    pub is_applied: bool,
    pub is_virtual: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// The stored half of an official account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeOfficialAccountContent {
    pub provider_id: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// `v2/credentials.json` verbatim, ciphertext included.
    pub credentials_snapshot: String,
    /// `v2/config.json` verbatim, and only for installations that have not
    /// migrated to `provider_config.json` yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i64>,
    pub is_applied: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// What applying an account did.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeOfficialAccountApplyResult {
    /// Name of the login that was saved aside before being overwritten, when
    /// the live credentials held one that was not already in the list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserved_as: Option<String>,
}

/// One file as the preview modal shows it. `content` is `None` when the file
/// does not exist yet, which the modal skips rather than rendering empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodePreviewFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Every file ZCode reads, so the preview shows the same picture the runtime
/// sees rather than the one file AI Toolbox happens to own.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeConfigPreview {
    /// `v2/provider_config.json` — the current-generation provider registry.
    pub provider_config: ZcodePreviewFile,
    /// Legacy `v2/config.json`. Absent on migrated installations, where the
    /// runtime no longer reads it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_config: Option<ZcodePreviewFile>,
    /// `cli/config.json` — MCP servers, hooks, plugins, permissions.
    pub cli_config: ZcodePreviewFile,
    /// `v2/setting.json` — desktop settings, including `dataBaseDir`.
    pub setting: ZcodePreviewFile,
}
