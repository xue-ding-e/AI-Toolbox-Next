import React from 'react';
import type { ClaudeCodeProvider, ClaudeSettingsConfig } from '@/types/claudecode';
import type { CodexProvider, CodexSettingsConfig } from '@/types/codex';
import type { OpenCodeProvider } from '@/types/opencode';
import type { OpenCodeDiagnosticsConfig } from '@/services/opencodeApi';
import { extractCodexBaseUrl, extractCodexModel } from '@/utils/codexConfigUtils';
import ConnectivityTestModal from '@/features/coding/opencode/components/ConnectivityTestModal';

const DEFAULT_CLAUDE_BASE_URL = 'https://api.anthropic.com/v1';
const DEFAULT_CODEX_BASE_URL = 'https://api.openai.com/v1';

export interface ProviderConnectivityInfo {
  providerId: string;
  providerName: string;
  providerConfig: OpenCodeProvider;
  modelIds: string[];
}

interface ProviderConnectivityTestModalProps {
  open: boolean;
  connectivityInfo: ProviderConnectivityInfo | null;
  onCancel: () => void;
  diagnostics?: OpenCodeDiagnosticsConfig;
  onSaveDiagnostics?: (diagnostics: OpenCodeDiagnosticsConfig) => Promise<void>;
}

function parseJsonConfig<T>(rawConfig: string, fallbackValue: T): T {
  try {
    return JSON.parse(rawConfig) as T;
  } catch (error) {
    console.error('Failed to parse provider settings config:', error);
    return fallbackValue;
  }
}

function normalizeClaudeBaseUrl(baseUrl?: string): string {
  const trimmedBaseUrl = baseUrl?.trim();
  if (!trimmedBaseUrl) {
    return DEFAULT_CLAUDE_BASE_URL;
  }

  const normalizedBaseUrl = trimmedBaseUrl.replace(/\/+$/, '');
  if (/\/v\d+(?:beta\d*)?$/i.test(normalizedBaseUrl)) {
    return normalizedBaseUrl;
  }

  return `${normalizedBaseUrl}/v1`;
}

function buildProviderModels(modelIds: string[]): OpenCodeProvider['models'] {
  return Object.fromEntries(modelIds.map((modelId) => [modelId, {}]));
}

export function buildClaudeProviderConnectivityInfo(
  provider: ClaudeCodeProvider
): ProviderConnectivityInfo {
  const settingsConfig = parseJsonConfig<ClaudeSettingsConfig>(provider.settingsConfig, {});
  const apiKey =
    settingsConfig.env?.ANTHROPIC_AUTH_TOKEN?.trim() ||
    settingsConfig.env?.ANTHROPIC_API_KEY?.trim();
  const modelIds = [
    settingsConfig.model,
    settingsConfig.haikuModel,
    settingsConfig.sonnetModel,
    settingsConfig.opusModel,
    settingsConfig.reasoningModel,
  ].filter((modelId): modelId is string => Boolean(modelId?.trim()));
  const uniqueModelIds = Array.from(new Set(modelIds));

  return {
    providerId: provider.id,
    providerName: provider.name,
    providerConfig: {
      npm: '@ai-sdk/anthropic',
      name: provider.name,
      options: {
        baseURL: normalizeClaudeBaseUrl(settingsConfig.env?.ANTHROPIC_BASE_URL),
        ...(apiKey ? { apiKey } : {}),
      },
      models: buildProviderModels(uniqueModelIds),
    },
    modelIds: uniqueModelIds,
  };
}

export function buildCodexProviderConnectivityInfo(provider: CodexProvider): ProviderConnectivityInfo {
  const settingsConfig = parseJsonConfig<CodexSettingsConfig>(provider.settingsConfig, {});
  const modelId = extractCodexModel(settingsConfig.config)?.trim();
  const apiKey = settingsConfig.auth?.OPENAI_API_KEY?.trim();
  const baseUrl = extractCodexBaseUrl(settingsConfig.config)?.trim() || DEFAULT_CODEX_BASE_URL;
  const modelIds = modelId ? [modelId] : [];

  return {
    providerId: provider.id,
    providerName: provider.name,
    providerConfig: {
      npm: '@ai-sdk/openai',
      name: provider.name,
      options: {
        baseURL: baseUrl,
        ...(apiKey ? { apiKey } : {}),
      },
      models: buildProviderModels(modelIds),
    },
    modelIds,
  };
}

const ProviderConnectivityTestModal: React.FC<ProviderConnectivityTestModalProps> = ({
  open,
  connectivityInfo,
  onCancel,
  diagnostics,
  onSaveDiagnostics,
}) => {
  if (!connectivityInfo) {
    return null;
  }

  return (
    <ConnectivityTestModal
      open={open}
      onCancel={onCancel}
      providerId={connectivityInfo.providerId}
      providerName={connectivityInfo.providerName}
      providerConfig={connectivityInfo.providerConfig}
      modelIds={connectivityInfo.modelIds}
      diagnostics={diagnostics}
      onSaveDiagnostics={onSaveDiagnostics || (async () => {})}
    />
  );
};

export default ProviderConnectivityTestModal;
