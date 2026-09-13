/**
 * OMP provider `api` 词表与元数据。
 *
 * 取值镜像上游 oh-my-pi 的 `ApiSchema`（models.yml 中 `api` 的合法枚举，见
 * oh-my-pi packages/coding-agent/src/config/models-config-schema-bundle.ts）：
 * 未知 `api` 值会导致整个 models.yml 校验失败、所有自定义 provider 被禁用。
 * 但 omp 的 `Api` 类型对扩展开放（扩展可注册自定义 API），因此这里只提供
 * 词表选项、不做枚举校验。
 */
export const OMP_API_VALUES = [
  'openai-completions',
  'openai-responses',
  'openai-codex-responses',
  'azure-openai-responses',
  'anthropic-messages',
  'bedrock-converse-stream',
  'google-generative-ai',
  'google-gemini-cli',
  'google-vertex',
] as const;

export type OmpApiValue = (typeof OMP_API_VALUES)[number];

export const OMP_API_OPTIONS: Array<{ value: OmpApiValue; label: string }> = OMP_API_VALUES.map(
  (value) => ({ value, label: value }),
);

/** api → 下拉项场景说明的 i18n key（`ohMyPi.apiDescription.*`）。 */
export const OMP_API_DESCRIPTION_I18N_KEYS: Record<OmpApiValue, string> = {
  'openai-completions': 'ohMyPi.apiDescription.openaiCompletions',
  'openai-responses': 'ohMyPi.apiDescription.openaiResponses',
  'openai-codex-responses': 'ohMyPi.apiDescription.openaiCodexResponses',
  'azure-openai-responses': 'ohMyPi.apiDescription.azureOpenaiResponses',
  'anthropic-messages': 'ohMyPi.apiDescription.anthropicMessages',
  'bedrock-converse-stream': 'ohMyPi.apiDescription.bedrockConverseStream',
  'google-generative-ai': 'ohMyPi.apiDescription.googleGenerativeAi',
  'google-gemini-cli': 'ohMyPi.apiDescription.googleGeminiCli',
  'google-vertex': 'ohMyPi.apiDescription.googleVertex',
};

/**
 * 新建 provider 选择 api 时的官方默认 baseUrl 预填值。只收录端点稳定、
 * 不依赖用户资源的协议；Azure（资源地址）、Vertex/Bedrock（区域/OAuth）、
 * gemini-cli（OAuth）不预填。
 */
export const OMP_API_DEFAULT_BASE_URL: Partial<Record<OmpApiValue, string>> = {
  'openai-responses': 'https://api.openai.com/v1',
  'openai-codex-responses': 'https://chatgpt.com/backend-api',
  'anthropic-messages': 'https://api.anthropic.com',
  'google-generative-ai': 'https://generativelanguage.googleapis.com/v1beta',
};
