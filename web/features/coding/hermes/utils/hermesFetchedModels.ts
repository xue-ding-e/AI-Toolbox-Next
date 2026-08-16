import type { FetchedModel } from '@/components/common/FetchModelsModal/types';
import type { PresetModel } from '@/constants/presetModels';

/**
 * Build a Hermes model record from a fetched upstream model.
 *
 * 拉取到的模型只有 id/name;若能在预设模型库匹配到(大小写不敏感),则用预设参数
 * 补齐 `context_length` / `max_tokens` / `reasoning`。
 * 与 OpenClaw 一致:只补参数,**不改写**上游模型 id 的大小写。
 *
 * 注:Hermes 的 per-model 思考等级在顶层 `agent.reasoning_overrides`,不在模型条目
 * 内(拉取阶段无 provider 前缀也无从计算),因此这里不写任何 thinking 字段。
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
  }
  return record;
};