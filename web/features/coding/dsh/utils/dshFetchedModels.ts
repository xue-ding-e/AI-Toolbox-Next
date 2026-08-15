import type { FetchedModel } from '../../../../components/common/FetchModelsModal/types';
import type { PresetModel } from '../../../../constants/presetModels';
import { buildPiThinkingLevelMapFromPreset } from '@/utils/piModelMetadata';

/**
 * Map a dsh provider api string to a preset SDK npm group so the shared
 * fetch/connectivity tooling can reuse the same model catalog.
 */
export function dshApiToSdkName(api?: string): string {
  switch (api) {
    case 'anthropic-messages':
      return '@ai-sdk/anthropic';
    case 'google-generative-ai':
    case 'google-vertex':
      return '@ai-sdk/google';
    default:
      return '@ai-sdk/openai-compatible';
  }
}

/**
 * Convert a fetched upstream model into a dsh settings.yaml models entry
 * ({ id, contextWindow?, maxTokens? }). Preset metadata enriches capabilities
 * but never rewrites the upstream model id casing.
 */
export const buildFetchedDshModel = (
  fetchedModel: FetchedModel,
  matchedPresetModel?: PresetModel | null,
): Record<string, unknown> => {
  if (matchedPresetModel) {
    // Preset thinking levels (variants -> reasoningEfforts). The shared builder
    // fills unsupported levels with `null`, so drop them to persist only real ones.
    const reasoningEfforts: Record<string, string> = {};
    Object.entries(buildPiThinkingLevelMapFromPreset(matchedPresetModel.variants))
      .forEach(([level, value]) => {
        if (value !== null && value !== undefined && value !== '') {
          reasoningEfforts[level] = value;
        }
      });
    return {
      id: fetchedModel.id,
      name: matchedPresetModel.name || fetchedModel.name || fetchedModel.id,
      ...(matchedPresetModel.contextLimit ? { contextWindow: matchedPresetModel.contextLimit } : {}),
      ...(matchedPresetModel.outputLimit ? { maxTokens: matchedPresetModel.outputLimit } : {}),
      ...(Object.keys(reasoningEfforts).length > 0 ? { reasoningEfforts } : {}),
    };
  }
  return {
    id: fetchedModel.id,
    ...(fetchedModel.name ? { name: fetchedModel.name } : {}),
  };
};