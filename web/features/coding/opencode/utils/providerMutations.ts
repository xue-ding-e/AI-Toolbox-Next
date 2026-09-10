import type { OpenCodeConfig, OpenCodeProvider } from '@/types/opencode';
import { sanitizeOpenCodeAgentModelReferences } from './openCodeAgentConfig';

export function canDeleteOpenCodeProvider(config: OpenCodeConfig, providerId: string): boolean {
  return Boolean(config.provider[providerId]) && config.model?.split('/')[0] !== providerId;
}

export function sanitizeOpenCodeModelReferences(
  config: OpenCodeConfig,
  removedUnifiedModelIds: string[],
): OpenCodeConfig {
  if (removedUnifiedModelIds.length === 0) return config;
  const removedModelIds = new Set(removedUnifiedModelIds);
  return sanitizeOpenCodeAgentModelReferences({
    ...config,
    model: config.model && removedModelIds.has(config.model) ? undefined : config.model,
    small_model: config.small_model && removedModelIds.has(config.small_model) ? undefined : config.small_model,
  }, removedModelIds);
}

export function replaceOpenCodeProvider(
  config: OpenCodeConfig,
  providerId: string,
  provider: OpenCodeProvider,
): OpenCodeConfig {
  const remainingModelIds = new Set(Object.keys(provider.models ?? {}));
  const removedModelIds = Object.keys(config.provider[providerId]?.models ?? {})
    .filter((modelId) => !remainingModelIds.has(modelId))
    .map((modelId) => `${providerId}/${modelId}`);
  return sanitizeOpenCodeModelReferences({
    ...config,
    provider: { ...config.provider, [providerId]: provider },
  }, removedModelIds);
}

/** Keep the favorite import source intact until its replacement is saved. */
export async function overwriteOpenCodeProvider(options: {
  config: OpenCodeConfig;
  providerId: string;
  provider: OpenCodeProvider;
  saveConfig: (config: OpenCodeConfig) => Promise<void>;
  backupPreviousProvider: (provider: OpenCodeProvider) => Promise<unknown>;
  onBackupError: (error: unknown) => void;
}): Promise<void> {
  const { config, providerId, provider, saveConfig, backupPreviousProvider, onBackupError } = options;
  const previousProvider = config.provider[providerId];
  await saveConfig(replaceOpenCodeProvider(config, providerId, provider));
  if (previousProvider) {
    try {
      await backupPreviousProvider(previousProvider);
    } catch (error) {
      // The runtime save already succeeded; a backup failure must not undo it
      // or turn a completed overwrite into a reported save failure.
      onBackupError(error);
    }
  }
}
