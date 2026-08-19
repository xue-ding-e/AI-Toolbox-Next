import type { FetchedModel } from '../../../../components/common/FetchModelsModal/types.ts';
import type { PresetModel } from '../../../../constants/presetModels.ts';
import { PI_INPUT_TYPES } from '../../../../utils/piModelMetadata.ts';
import { buildOmpThinkingFromPreset } from '../../../../utils/ompModelMetadata.ts';
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
 * Build an OMP model from a preset, keeping the upstream model id verbatim.
 *
 * OMP expresses thinking levels as `thinking: { efforts, defaultLevel }` (via
 * buildOmpThinkingFromPreset) — not Pi's `thinkingLevelMap`, which the page
 * drops on save. Preset matching is case-insensitive for capability
 * enrichment only; never rewrite the upstream id casing.
 */
export const buildOmpModelFromPreset = (
  preset: PresetModel,
  modelId: string,
  fallbackName: string,
  api?: string,
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
  const ompThinking = buildOmpThinkingFromPreset(preset.variants, api);

  return {
    id: modelId,
    name: preset.name || fallbackName,
    ...(preset.reasoning !== undefined ? { reasoning: preset.reasoning } : {}),
    ...(inputTypes.length > 0 ? { input: inputTypes } : {}),
    ...(preset.contextLimit ? { contextWindow: preset.contextLimit } : {}),
    ...(preset.outputLimit ? { maxTokens: preset.outputLimit } : {}),
    ...(!isRecordEmpty(piCost) ? { cost: piCost } : {}),
    ...(ompThinking !== undefined ? { thinking: ompThinking } : {}),
  };
};

/**
 * Convert a fetched upstream model into an OMP models entry.
 * Preset metadata enriches capabilities but never rewrites model id casing.
 */
export const buildFetchedOmpModel = (
  fetchedModel: FetchedModel,
  matchedPresetModel?: PresetModel | null,
  api?: string,
): Record<string, unknown> => {
  if (matchedPresetModel) {
    return buildOmpModelFromPreset(
      matchedPresetModel,
      fetchedModel.id,
      fetchedModel.name || fetchedModel.id,
      api,
    );
  }
  return {
    id: fetchedModel.id,
    ...(fetchedModel.name ? { name: fetchedModel.name } : {}),
  };
};

/** Map OMP provider api string to preset SDK npm group. */
export function ompApiToSdkName(api?: string): string {
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
