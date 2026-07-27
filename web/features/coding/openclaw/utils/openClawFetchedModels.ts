import type { FetchedModel } from '../../../../components/common/FetchModelsModal/types.ts';
import type { PresetModel } from '../../../../constants/presetModels.ts';
import type { OpenClawModel } from '../../../../types/openclaw.ts';

const OPENAI_COMPATIBLE_NPM = '@ai-sdk/openai-compatible';

export const getOpenClawFetchedModelDefaultInput = (providerNpm?: string): string[] => (
  providerNpm === OPENAI_COMPATIBLE_NPM ? ['text'] : ['text', 'image']
);

/**
 * Build an OpenClaw model from a preset, keeping the upstream model id verbatim.
 *
 * Preset matching is case-insensitive for capability enrichment only.
 * Never rewrite the upstream id to the preset's canonical casing.
 */
export const buildOpenClawModelFromPreset = (
  preset: PresetModel,
  modelId: string,
  fallbackName: string,
): OpenClawModel => ({
  id: modelId,
  name: preset.name || fallbackName,
  contextWindow: preset.contextLimit,
  maxTokens: preset.outputLimit,
  reasoning: preset.reasoning ?? false,
  ...(preset.modalities?.input ? { input: preset.modalities.input } : {}),
});

/**
 * Convert a fetched upstream model into an OpenClaw models entry.
 * Preset metadata enriches capabilities but never rewrites model id casing.
 */
export const buildFetchedOpenClawModel = (
  fetchedModel: FetchedModel,
  providerNpm?: string,
  matchedPresetModel?: PresetModel | null,
): OpenClawModel => {
  if (matchedPresetModel) {
    return buildOpenClawModelFromPreset(
      matchedPresetModel,
      fetchedModel.id,
      fetchedModel.name || fetchedModel.id,
    );
  }

  return {
    id: fetchedModel.id,
    name: fetchedModel.name || fetchedModel.id,
    reasoning: true,
    input: getOpenClawFetchedModelDefaultInput(providerNpm),
  };
};
