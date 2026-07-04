import { normalizeGatewayApiFormat, type GatewayApiFormat } from './providerProtocol';

export type GatewayProviderToolKey = 'claude' | 'codex';

export interface GatewayProviderModelDefaults {
  primary?: string;
  haiku?: string;
  sonnet?: string;
  opus?: string;
  fable?: string;
}

export interface GatewayProviderEndpointProfile {
  id: string;
  label: string;
  apiFormat: GatewayApiFormat | 'anthropic';
  baseUrl: string;
  model?: string;
  reasoningField?: 'reasoning_content' | 'content' | 'reasoning' | 'none' | 'all' | string;
  defaultMaxTokens?: number;
  imageInputPolicy?: 'auto' | 'preserve' | 'strip' | 'text_only' | string;
  textOnlyModels?: string[];
  imageCapableModels?: string[];
  allowTextOnlyModelHeuristic?: boolean;
  models?: GatewayProviderModelDefaults;
  configProviderId?: string;
  modelCatalog?: {
    models?: Array<{
      model: string;
      displayName?: string;
      contextWindow?: number;
      supportsImage?: boolean;
      vision?: boolean;
      attachment?: boolean;
      modalities?: {
        input?: string[];
        output?: string[];
      };
    }>;
  };
  codexChatReasoning?: Record<string, unknown>;
}

export interface GatewayProviderToolProfile {
  defaultEndpointId: string;
  endpoints: GatewayProviderEndpointProfile[];
}

export interface GatewayProviderProfile {
  id: string;
  providerType: string;
  reasoningField?: 'reasoning_content' | 'content' | 'reasoning' | 'none' | 'all' | string;
  defaultMaxTokens?: number;
  label: string;
  category?: string;
  aliases?: string[];
  tools: Partial<Record<GatewayProviderToolKey, GatewayProviderToolProfile>>;
  compat?: Record<string, string[]>;
}

export interface GatewayProviderProfileCatalog {
  schemaVersion: number;
  updatedAt?: string;
  profiles: GatewayProviderProfile[];
}

type GatewayProviderProfileListener = () => void;

export const GATEWAY_PROVIDER_PROFILES_REMOTE_URL =
  'https://raw.githubusercontent.com/coulsontl/ai-toolbox/main/tauri/resources/gateway_provider_profiles.json';

export const GATEWAY_PROVIDER_PROFILE_CATALOG: GatewayProviderProfileCatalog = {
  schemaVersion: 1,
  profiles: [],
};

let gatewayProviderProfilesVersion = 0;
const gatewayProviderProfileListeners = new Set<GatewayProviderProfileListener>();

export const getGatewayProviderProfilesVersion = () => gatewayProviderProfilesVersion;

export const subscribeGatewayProviderProfiles = (
  listener: GatewayProviderProfileListener,
): (() => void) => {
  gatewayProviderProfileListeners.add(listener);
  return () => {
    gatewayProviderProfileListeners.delete(listener);
  };
};

const notifyGatewayProviderProfilesUpdated = () => {
  gatewayProviderProfilesVersion += 1;
  gatewayProviderProfileListeners.forEach((listener) => listener());
};

export const updateGatewayProviderProfiles = (catalog: GatewayProviderProfileCatalog) => {
  if (!catalog || catalog.schemaVersion !== 1 || !Array.isArray(catalog.profiles) || catalog.profiles.length === 0) {
    return;
  }
  GATEWAY_PROVIDER_PROFILE_CATALOG.schemaVersion = catalog.schemaVersion;
  GATEWAY_PROVIDER_PROFILE_CATALOG.updatedAt = catalog.updatedAt;
  GATEWAY_PROVIDER_PROFILE_CATALOG.profiles = catalog.profiles;
  notifyGatewayProviderProfilesUpdated();
};

export const CUSTOM_PROVIDER_PROFILE_ID = '__custom__';
export const CUSTOM_PROVIDER_ENDPOINT_KEY = `${CUSTOM_PROVIDER_PROFILE_ID}:`;

export const toGatewayProviderEndpointKey = (
  profileId: string,
  endpointId?: string | null,
) => `${profileId}:${endpointId || ''}`;

export const parseGatewayProviderEndpointKey = (value?: string | null) => {
  if (!value || value === CUSTOM_PROVIDER_ENDPOINT_KEY) {
    return {
      providerProfileId: CUSTOM_PROVIDER_PROFILE_ID,
      providerEndpointId: undefined,
    };
  }
  const separatorIndex = value.indexOf(':');
  if (separatorIndex < 0) {
    return {
      providerProfileId: value,
      providerEndpointId: undefined,
    };
  }
  return {
    providerProfileId: value.slice(0, separatorIndex),
    providerEndpointId: value.slice(separatorIndex + 1) || undefined,
  };
};

export const getGatewayProviderProfilesForTool = (tool: GatewayProviderToolKey) =>
  GATEWAY_PROVIDER_PROFILE_CATALOG.profiles.filter((profile) => profile.tools?.[tool]);

export const findGatewayProviderProfile = (profileId?: string | null) =>
  GATEWAY_PROVIDER_PROFILE_CATALOG.profiles.find((profile) => profile.id === profileId);

export const findGatewayProviderToolProfile = (
  profileId: string | null | undefined,
  tool: GatewayProviderToolKey,
) => findGatewayProviderProfile(profileId)?.tools?.[tool];

export const findGatewayProviderEndpoint = (
  profileId: string | null | undefined,
  tool: GatewayProviderToolKey,
  endpointId?: string | null,
) => {
  const toolProfile = findGatewayProviderToolProfile(profileId, tool);
  if (!toolProfile) {
    return undefined;
  }
  const selectedEndpointId = endpointId || toolProfile.defaultEndpointId;
  return toolProfile.endpoints.find((endpoint) => endpoint.id === selectedEndpointId)
    ?? toolProfile.endpoints.find((endpoint) => endpoint.id === toolProfile.defaultEndpointId)
    ?? toolProfile.endpoints[0];
};

const normalizeEndpointBaseUrl = (baseUrl?: string | null) =>
  baseUrl?.trim().replace(/\/+$/, '').toLowerCase() || '';

export const endpointBaseUrlMatches = (
  endpoint: GatewayProviderEndpointProfile | undefined,
  baseUrl?: string | null,
) => Boolean(endpoint && normalizeEndpointBaseUrl(endpoint.baseUrl) === normalizeEndpointBaseUrl(baseUrl));

export const inferGatewayProviderEndpointSelection = (params: {
  tool: GatewayProviderToolKey;
  providerType?: string | null;
  baseUrl?: string | null;
  apiFormat?: string | null;
}) => {
  const normalizedProviderType = params.providerType?.trim().toLowerCase();
  const normalizedBaseUrl = normalizeEndpointBaseUrl(params.baseUrl);
  const normalizedApiFormat = normalizeGatewayApiFormat(params.apiFormat);

  if (normalizedProviderType) {
    const providerTypeMatches = getGatewayProviderProfilesForTool(params.tool).filter(
      (profile) => profile.providerType.toLowerCase() === normalizedProviderType,
    );
    const exactEndpointMatch = providerTypeMatches.flatMap((profile) => {
      const toolProfile = profile.tools[params.tool];
      return (toolProfile?.endpoints || []).map((endpoint) => ({ profile, endpoint }));
    }).find(({ endpoint }) =>
      normalizeEndpointBaseUrl(endpoint.baseUrl) === normalizedBaseUrl &&
      normalizeGatewayApiFormat(endpoint.apiFormat) === normalizedApiFormat,
    );
    if (exactEndpointMatch) {
      return {
        providerProfileId: exactEndpointMatch.profile.id,
        providerEndpointId: exactEndpointMatch.endpoint.id,
      };
    }
    const firstProfile = providerTypeMatches[0];
    return {
      providerProfileId: firstProfile?.id ?? CUSTOM_PROVIDER_PROFILE_ID,
      providerEndpointId: firstProfile?.tools[params.tool]?.defaultEndpointId,
    };
  }

  if (normalizedBaseUrl) {
    const exactEndpointMatch = getGatewayProviderProfilesForTool(params.tool).flatMap((profile) => {
      const toolProfile = profile.tools[params.tool];
      return (toolProfile?.endpoints || []).map((endpoint) => ({ profile, endpoint }));
    }).find(({ endpoint }) =>
      normalizeEndpointBaseUrl(endpoint.baseUrl) === normalizedBaseUrl &&
      (!normalizedApiFormat || normalizeGatewayApiFormat(endpoint.apiFormat) === normalizedApiFormat),
    );
    if (exactEndpointMatch) {
      return {
        providerProfileId: exactEndpointMatch.profile.id,
        providerEndpointId: exactEndpointMatch.endpoint.id,
      };
    }
  }

  return {
    providerProfileId: CUSTOM_PROVIDER_PROFILE_ID,
    providerEndpointId: undefined,
  };
};
