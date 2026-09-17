import { getProviderModelRecords } from '../../../../utils/ompModelMetadata.ts';
import type { ProviderModelConnections } from '../../shared/providerConnectivity/modelConnection.ts';
import { ompApiToSdkName } from './ompFetchedModels.ts';

const stringField = (value: unknown): string => typeof value === 'string' ? value.trim() : '';

/** 一次真实请求就能验证的协议（连通性测试）。 */
const CONNECTIVITY_APIS = [
  'openai-completions',
  'openai-responses',
  'openai-codex-responses',
  'anthropic-messages',
  'google-generative-ai',
];

/**
 * 能列举模型目录的协议。`openai-codex-responses` 走的是 ChatGPT 订阅后端，
 * 没有 `/models`，所以获取模型对它保持禁用（诊断本身仍然可用）。
 */
const MODEL_DISCOVERY_APIS = [
  'openai-completions',
  'openai-responses',
  'anthropic-messages',
  'google-generative-ai',
];

/**
 * 诊断/发现端点按 api 补版本路径：Anthropic 补 `/v1`、Gemini 补版本段。
 * 只用于探测，不能反写 `models.yml`。
 */
const normalizeProbeBaseUrl = (api: string, baseUrl: string): string => {
  let normalized = baseUrl.replace(/\/+$/, '');
  if (normalized && api === 'anthropic-messages' && !normalized.endsWith('/v1')) {
    normalized += '/v1';
  }
  if (normalized && api === 'google-generative-ai' && !/\/(v1|v1alpha|v1beta)$/.test(normalized)) {
    normalized += '/v1beta';
  }
  return normalized;
};

const isCodexApi = (api: string) => api === 'openai-codex-responses';

/** 单个模型自己的连接：模型级 `api`/`baseUrl` 覆盖优先，否则继承供应商级声明。 */
export interface OmpModelConnection {
  modelId: string;
  api: string;
  npm: string;
  baseUrl: string;
  apiFormat?: 'openai-codex-responses';
  /** 该连接是否落在连通性测试支持词表内；false 的模型不该进入测试列表。 */
  supported: boolean;
}

const buildModelConnection = (modelId: string, api: string, baseUrl: string): OmpModelConnection => ({
  modelId,
  api,
  npm: ompApiToSdkName(api),
  baseUrl: normalizeProbeBaseUrl(api, baseUrl),
  apiFormat: isCodexApi(api) ? 'openai-codex-responses' : undefined,
  supported: CONNECTIVITY_APIS.includes(api),
});

/** 能进连通性测试的模型 id：协议在测试词表内的才有可执行连接。 */
export function getTestableOmpModelIds(connections: OmpModelConnection[]): string[] {
  return connections.filter((connection) => connection.supported).map((connection) => connection.modelId);
}

/** 连通性弹窗按模型覆盖连接时的入参（未列出的模型沿用供应商级请求）。 */
export function toOmpModelConnectionMap(
  connections: OmpModelConnection[],
): ProviderModelConnections {
  return Object.fromEntries(
    connections
      .filter((connection) => connection.supported)
      .map((connection) => [
        connection.modelId,
        {
          npm: connection.npm,
          baseUrl: connection.baseUrl,
          apiFormat: connection.apiFormat,
        },
      ]),
  );
}

/**
 * 在不改写 OMP 运行配置的前提下推导诊断连接。
 *
 * 模型级 `api`/`baseUrl` 覆盖只决定该模型自己的请求走法（上游
 * `ModelDefinitionSchema` 允许逐模型覆盖），因此：
 * - 模型连接一致时沿用这些覆盖值（`模型连接一致时可用模型覆盖值`）；
 * - 连接混用时，供应商级 `api`/`baseUrl` 才是这个 provider 的公共端点，
 *   目录请求（获取模型）与未覆盖模型的兜底连接都取它；
 * - 连通性测试不再因混用而整体禁用，改为按 `modelConnections` 逐模型测试。
 *
 * 注意返回值里的 `api`/`npm`/`baseUrl`/`apiFormat` 是**供应商级代表性连接**
 * （供目录请求与未覆盖模型兜底）；单个模型自己的连接在 `modelConnections` 里。
 */
export function getOmpDiagnostics(provider: Record<string, unknown>) {
  const providerApi = stringField(provider.api);
  const providerBaseUrl = stringField(provider.baseUrl);
  const records = getProviderModelRecords(provider);
  const connections = (records.length ? records.map((record) => record.model) : [{}]).map((model) => ({
    api: stringField(model.api) || providerApi,
    baseUrl: stringField(model.baseUrl) || providerBaseUrl,
  }));
  const { api, baseUrl } = connections[0];
  const mixedConnections = connections.some(connection => connection.api !== api || connection.baseUrl !== baseUrl);

  const diagnosticApi = mixedConnections ? providerApi : api;
  const diagnosticBaseUrl = normalizeProbeBaseUrl(diagnosticApi, mixedConnections ? providerBaseUrl : baseUrl);
  const modelConnections = records.map((record) => buildModelConnection(
    record.id,
    stringField(record.model.api) || providerApi,
    stringField(record.model.baseUrl) || providerBaseUrl,
  ));

  return {
    api: diagnosticApi,
    npm: ompApiToSdkName(diagnosticApi),
    baseUrl: diagnosticBaseUrl,
    apiFormat: isCodexApi(diagnosticApi) ? 'openai-codex-responses' as const : undefined,
    // 有模型时以逐模型测试为准（混用不再整体禁用）；没有模型可测时才回落到
    // 代表性连接——否则会出现“按钮可用但测试列表为空”。
    supportsConnectivity: modelConnections.length > 0
      ? modelConnections.some(connection => connection.supported)
      : CONNECTIVITY_APIS.includes(diagnosticApi),
    supportsModelDiscovery: MODEL_DISCOVERY_APIS.includes(diagnosticApi),
    mixedConnections,
    modelConnections,
  };
}
