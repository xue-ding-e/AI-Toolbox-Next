import { ompApiToSdkName } from './ompFetchedModels.ts';

const asRecord = (value: unknown): Record<string, unknown> => (
  value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {}
);
const stringField = (value: unknown): string => typeof value === 'string' ? value.trim() : '';

/** Derive diagnostic connection settings without changing OMP's runtime config. */
export function getOmpDiagnostics(provider: Record<string, unknown>) {
  const models = Array.isArray(provider.models) ? provider.models.map(asRecord) : [];
  const connections = (models.length ? models : [{}]).map(model => ({
    api: stringField(model.api) || stringField(provider.api),
    baseUrl: stringField(model.baseUrl) || stringField(provider.baseUrl),
  }));
  const { api, baseUrl } = connections[0];
  const mixedConnections = connections.some(connection => connection.api !== api || connection.baseUrl !== baseUrl);
  const supportsConnectivity = !mixedConnections && [
    'openai-completions', 'openai-responses', 'openai-codex-responses',
    'anthropic-messages', 'google-generative-ai',
  ].includes(api);
  let diagnosticBaseUrl = baseUrl.replace(/\/+$/, '');
  if (diagnosticBaseUrl && api === 'anthropic-messages' && !diagnosticBaseUrl.endsWith('/v1')) {
    diagnosticBaseUrl += '/v1';
  }
  if (diagnosticBaseUrl && api === 'google-generative-ai' && !/\/(v1|v1alpha|v1beta)$/.test(diagnosticBaseUrl)) {
    diagnosticBaseUrl += '/v1beta';
  }
  return {
    api,
    npm: ompApiToSdkName(api),
    baseUrl: diagnosticBaseUrl,
    apiFormat: api === 'openai-codex-responses' ? 'openai-codex-responses' as const : undefined,
    supportsConnectivity,
    supportsModelDiscovery: supportsConnectivity && api !== 'openai-codex-responses',
    mixedConnections,
  };
}
