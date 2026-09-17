/**
 * Per-model connection overrides for provider connectivity tests.
 *
 * Most tools have exactly one connection per provider, so callers that pass no
 * `modelConnections` keep the existing behaviour: every tested model uses the
 * provider-level request. OMP is the exception — its `models.yml` allows each
 * model to override `api`/`baseUrl`, and those overrides only change that
 * model's own requests, so tests must follow the model instead of the provider.
 */
export interface ProviderModelConnection {
  npm: string;
  baseUrl: string;
  apiFormat?: 'openai-codex-responses';
}

export type ProviderModelConnections = Record<string, ProviderModelConnection>;

/** Extra fields a connectivity request needs to carry per model. */
export interface ProviderModelConnectionTarget {
  npm: string;
  baseUrl: string;
  apiFormat?: 'openai-codex-responses';
}

/**
 * Overlay a model's own connection onto the provider-level request. Returns a
 * copy; models without an override are untouched. `apiFormat` must be dropped
 * when the model resolves to a different protocol — otherwise a Codex-native
 * request shape would be sent to (say) an Anthropic endpoint.
 */
export function resolveModelConnection<T extends ProviderModelConnectionTarget>(
  baseRequest: T,
  modelId: string,
  modelConnections?: ProviderModelConnections,
): T {
  const connection = modelConnections?.[modelId];
  if (!connection) {
    return baseRequest;
  }
  return {
    ...baseRequest,
    npm: connection.npm,
    baseUrl: connection.baseUrl,
    apiFormat: connection.apiFormat,
  };
}

/**
 * Name the token cap the way the SDK serving the request expects it: Google's
 * backend path reads `maxOutputTokens`, every other path reads `maxTokens`.
 * The name follows the resolved npm, since a model may override the provider's.
 */
export function buildTokenCapFields(
  npm: string,
  maxTokens: number | undefined,
): { maxTokens?: number; maxOutputTokens?: number } {
  if (maxTokens === undefined) {
    return {};
  }
  return npm === '@ai-sdk/google' ? { maxOutputTokens: maxTokens } : { maxTokens };
}