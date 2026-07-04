import type { ClaudeSettingsConfig } from '@/types/claudecode';

export const CLAUDE_ONE_M_MARKER = '[1M]';

export type ClaudeModelRole = 'sonnet' | 'opus' | 'fable' | 'haiku';

export interface ClaudeModelRoleConfig {
  role: ClaudeModelRole;
  model: string;
  displayName: string;
  supportsOneM: boolean;
}

export interface ClaudeProviderModelConfig {
  fallbackModel: string;
  roles: Record<ClaudeModelRole, ClaudeModelRoleConfig>;
  legacyReasoningModel: string;
}

export function hasClaudeOneMMarker(model: string): boolean {
  return model.trimEnd().toLowerCase().endsWith('[1m]');
}

export function stripClaudeOneMMarker(model: string): string {
  const trimmedModel = model.trimEnd();
  if (!trimmedModel.toLowerCase().endsWith('[1m]')) {
    return model;
  }
  return trimmedModel.slice(0, -CLAUDE_ONE_M_MARKER.length).trimEnd();
}

export function setClaudeOneMMarker(model: string, enabled: boolean): string {
  const baseModel = stripClaudeOneMMarker(model).trim();
  if (!baseModel) {
    return '';
  }
  return enabled ? `${baseModel}${CLAUDE_ONE_M_MARKER}` : baseModel;
}

export function parseClaudeSettingsConfig(rawConfig: string | undefined): ClaudeSettingsConfig {
  if (!rawConfig?.trim()) {
    return {};
  }

  try {
    const parsed = JSON.parse(rawConfig) as unknown;
    return typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
      ? parsed as ClaudeSettingsConfig
      : {};
  } catch (error) {
    console.error('Failed to parse Claude settings config:', error);
    return {};
  }
}

export function getClaudeProviderModelConfig(
  settingsConfig: ClaudeSettingsConfig,
): ClaudeProviderModelConfig {
  const sonnetModel = readConfigModel(settingsConfig, 'sonnetModel', 'ANTHROPIC_DEFAULT_SONNET_MODEL');
  const opusModel = readConfigModel(settingsConfig, 'opusModel', 'ANTHROPIC_DEFAULT_OPUS_MODEL');
  const fableModel = readConfigModel(settingsConfig, 'fableModel', 'ANTHROPIC_DEFAULT_FABLE_MODEL');
  const haikuModel = readConfigModel(settingsConfig, 'haikuModel', 'ANTHROPIC_DEFAULT_HAIKU_MODEL');

  return {
    fallbackModel: readConfigModel(settingsConfig, 'model', 'ANTHROPIC_MODEL'),
    roles: {
      sonnet: {
        role: 'sonnet',
        model: sonnetModel,
        displayName: readEnvString(settingsConfig, 'ANTHROPIC_DEFAULT_SONNET_MODEL_NAME') ||
          stripClaudeOneMMarker(sonnetModel),
        supportsOneM: true,
      },
      opus: {
        role: 'opus',
        model: opusModel,
        displayName: readEnvString(settingsConfig, 'ANTHROPIC_DEFAULT_OPUS_MODEL_NAME') ||
          stripClaudeOneMMarker(opusModel),
        supportsOneM: true,
      },
      fable: {
        role: 'fable',
        model: fableModel,
        displayName: readEnvString(settingsConfig, 'ANTHROPIC_DEFAULT_FABLE_MODEL_NAME') ||
          stripClaudeOneMMarker(fableModel),
        supportsOneM: true,
      },
      haiku: {
        role: 'haiku',
        model: stripClaudeOneMMarker(haikuModel),
        displayName: readEnvString(settingsConfig, 'ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME') ||
          stripClaudeOneMMarker(haikuModel),
        supportsOneM: false,
      },
    },
    legacyReasoningModel: readConfigModel(settingsConfig, 'reasoningModel', 'ANTHROPIC_REASONING_MODEL'),
  };
}

export function getClaudeConfiguredModelIds(
  settingsConfig: ClaudeSettingsConfig,
  options: { stripOneMMarker?: boolean; includeLegacyReasoning?: boolean } = {},
): string[] {
  const modelConfig = getClaudeProviderModelConfig(settingsConfig);
  const modelIds = [
    modelConfig.fallbackModel,
    modelConfig.roles.sonnet.model,
    modelConfig.roles.opus.model,
    modelConfig.roles.fable.model,
    modelConfig.roles.haiku.model,
    ...(options.includeLegacyReasoning === false ? [] : [modelConfig.legacyReasoningModel]),
  ];

  return Array.from(new Set(
    modelIds
      .map((modelId) => options.stripOneMMarker ? stripClaudeOneMMarker(modelId) : modelId)
      .map((modelId) => modelId.trim())
      .filter(Boolean),
  ));
}

function readConfigModel(
  settingsConfig: ClaudeSettingsConfig,
  legacyField: 'model' | 'haikuModel' | 'sonnetModel' | 'opusModel' | 'fableModel' | 'reasoningModel',
  envField: keyof NonNullable<ClaudeSettingsConfig['env']>,
): string {
  return readEnvString(settingsConfig, envField) || readTopLevelString(settingsConfig, legacyField);
}

function readTopLevelString(
  settingsConfig: ClaudeSettingsConfig,
  field: 'model' | 'haikuModel' | 'sonnetModel' | 'opusModel' | 'fableModel' | 'reasoningModel',
): string {
  const value = settingsConfig[field];
  return typeof value === 'string' ? value.trim() : '';
}

function readEnvString(
  settingsConfig: ClaudeSettingsConfig,
  field: keyof NonNullable<ClaudeSettingsConfig['env']>,
): string {
  const value = settingsConfig.env?.[field];
  return typeof value === 'string' ? value.trim() : '';
}
