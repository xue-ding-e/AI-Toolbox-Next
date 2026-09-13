import type { ApiType } from './types';

export function getDefaultModelsApiType(sdkType?: string): ApiType {
  return sdkType === '@ai-sdk/google' || sdkType === '@ai-sdk/anthropic'
    ? 'native'
    : 'openai_compat';
}

/** Keep the editable URL preview aligned with models_api.rs discovery paths. */
export function buildModelsUrl(
  baseUrl: string,
  apiType: ApiType,
  sdkType?: string,
  apiKey?: string,
): string {
  let base = baseUrl.trim().replace(/\/+$/, '');
  if (!base) return '';

  if (apiType === 'native' && sdkType === '@ai-sdk/google') {
    if (!/\/(?:v1|v1alpha|v1beta)$/.test(base)) base += '/v1beta';
    const modelsUrl = `${base}/models`;
    return apiKey ? `${modelsUrl}?key=${encodeURIComponent(apiKey)}` : modelsUrl;
  }
  return `${base}/models`;
}
