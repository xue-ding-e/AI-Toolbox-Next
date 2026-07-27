import type { FetchedModel } from '../../../../components/common/FetchModelsModal/types.ts';
import type { PresetModel } from '../../../../constants/presetModels.ts';
import {
  PI_INPUT_TYPES,
  buildPiThinkingLevelMapFromPreset,
} from '../../../../utils/piModelMetadata.ts';

const asRecord = (value: unknown): Record<string, unknown> => (
  value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}
);

const getNumberField = (value: Record<string, unknown>, key: string): number | undefined => {
  const fieldValue = value[key];
  return typeof fieldValue === 'number' && Number.isFinite(fieldValue) ? fieldValue : undefined;
};

const isRecordEmpty = (value: Record<string, unknown>): boolean => Object.keys(value).length === 0;

/**
 * Build a Pi model from a preset, keeping the upstream model id verbatim.
 *
 * Preset matching is case-insensitive for capability enrichment only.
 * Never rewrite the upstream id to the preset's canonical casing.
 */
export const buildPiModelFromPreset = (
  preset: PresetModel,
  modelId: string,
  fallbackName: string,
): Record<string, unknown> => {
  const inputTypes = (preset.modalities?.input ?? []).filter((inputType) => PI_INPUT_TYPES.has(inputType));
  const cost = asRecord(preset.cost);
  const piCost: Record<string, number> = {};
  const inputCost = getNumberField(cost, 'input');
  const outputCost = getNumberField(cost, 'output');
  const cacheReadCost = getNumberField(cost, 'cacheRead') ?? getNumberField(cost, 'cache_read');
  const cacheWriteCost = getNumberField(cost, 'cacheWrite') ?? getNumberField(cost, 'cache_write');
  if (inputCost !== undefined) {
    piCost.input = inputCost;
  }
  if (outputCost !== undefined) {
    piCost.output = outputCost;
  }
  if (cacheReadCost !== undefined) {
    piCost.cacheRead = cacheReadCost;
  }
  if (cacheWriteCost !== undefined) {
    piCost.cacheWrite = cacheWriteCost;
  }
  const thinkingLevelMap = buildPiThinkingLevelMapFromPreset(preset.variants);

  return {
    id: modelId,
    name: preset.name || fallbackName,
    ...(preset.reasoning !== undefined ? { reasoning: preset.reasoning } : {}),
    ...(inputTypes.length > 0 ? { input: inputTypes } : {}),
    ...(preset.contextLimit ? { contextWindow: preset.contextLimit } : {}),
    ...(preset.outputLimit ? { maxTokens: preset.outputLimit } : {}),
    ...(!isRecordEmpty(piCost) ? { cost: piCost } : {}),
    ...(!isRecordEmpty(thinkingLevelMap) ? { thinkingLevelMap } : {}),
  };
};

/**
 * Convert a fetched upstream model into a Pi models.json entry.
 * Preset metadata enriches capabilities but never rewrites model id casing.
 */
export const buildFetchedPiModel = (
  fetchedModel: FetchedModel,
  matchedPresetModel?: PresetModel | null,
): Record<string, unknown> => {
  if (matchedPresetModel) {
    return buildPiModelFromPreset(
      matchedPresetModel,
      fetchedModel.id,
      fetchedModel.name || fetchedModel.id,
    );
  }
  return {
    id: fetchedModel.id,
    ...(fetchedModel.name ? { name: fetchedModel.name } : {}),
  };
};

/** Map Pi provider api string to preset SDK npm group. */
export function piApiToSdkName(api?: string): string {
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
