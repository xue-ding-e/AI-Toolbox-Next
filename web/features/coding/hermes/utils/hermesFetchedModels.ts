import type { FetchedModel } from '@/components/common/FetchModelsModal/types';
import type { PresetModel } from '@/constants/presetModels';

/**
 * Build a Hermes model record from a fetched upstream model.
 *
 * 拉取到的模型只有 id/name;若能在预设模型库匹配到(大小写不敏感),则用预设参数
 * 补齐 `context_length` / `max_tokens` / `reasoning`,以及由 preset `variants`
 * 推导的思考等级 `reasoningEfforts`(与 DSH 一致:剔除 null/空层级)。
 * 与 OpenClaw 一致:只补参数,**不改写**上游模型 id 的大小写。
 */
export const buildFetchedHermesModel = (
  fetchedModel: FetchedModel,
  matchedPresetModel?: PresetModel | null,
): Record<string, unknown> => {
  const record: Record<string, unknown> = {
    id: fetchedModel.id,
    name: fetchedModel.name || fetchedModel.id,
  };
  if (matchedPresetModel) {
    if (typeof matchedPresetModel.contextLimit === 'number') {
      record.context_length = matchedPresetModel.contextLimit;
    }
    if (typeof matchedPresetModel.outputLimit === 'number') {
      record.max_tokens = matchedPresetModel.outputLimit;
    }
    if (matchedPresetModel.reasoning === true) {
      record.reasoning = true;
    }
    // Preset thinking levels (variants -> reasoningEfforts). Only levels that
    // carry a real `thinkingLevel` string are persisted (equivalent to DSH's
    // drop-null behavior). Kept dependency-free so it runs under node:test.
    const reasoningEfforts: Record<string, string> = {};
    for (const [level, variant] of Object.entries(matchedPresetModel.variants ?? {})) {
      const value = typeof variant?.thinkingLevel === 'string' ? variant.thinkingLevel : undefined;
      if (value) {
        reasoningEfforts[level] = value;
      }
    }
    if (Object.keys(reasoningEfforts).length > 0) {
      record.reasoningEfforts = reasoningEfforts;
    }
  }
  return record;
};