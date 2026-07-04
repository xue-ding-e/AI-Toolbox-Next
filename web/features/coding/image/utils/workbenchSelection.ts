import type { ImageChannel } from '../services/imageApi';

export type ImageModeKey = 'text_to_image' | 'image_to_image';

export interface WorkbenchChannelOption {
  id: string;
  name: string;
  sortOrder: number;
  supportsTextToImage: boolean;
  supportsImageToImage: boolean;
}

export interface WorkbenchModelOption {
  id: string;
  label: string;
  supportsTextToImage: boolean;
  supportsImageToImage: boolean;
  availableChannels: WorkbenchChannelOption[];
}

interface ResolveWorkbenchSelectionInput {
  mode: ImageModeKey;
  modelId: string;
  channelId: string;
  modelOptions: WorkbenchModelOption[];
  preferFirstChannel?: boolean;
}

export const supportsImageMode = (
  option: Pick<WorkbenchModelOption | WorkbenchChannelOption, 'supportsTextToImage' | 'supportsImageToImage'>,
  mode: ImageModeKey
): boolean => (
  mode === 'text_to_image'
    ? option.supportsTextToImage
    : option.supportsImageToImage
);

export const buildWorkbenchModelOptions = (channels: ImageChannel[]): WorkbenchModelOption[] => {
  const modelMap = new Map<string, WorkbenchModelOption>();

  for (const channel of channels) {
    if (!channel.enabled) continue;

    for (const model of channel.models) {
      if (!model.enabled) continue;

      const existingModel = modelMap.get(model.id);
      const nextChannelOption: WorkbenchChannelOption = {
        id: channel.id,
        name: channel.name,
        sortOrder: channel.sort_order,
        supportsTextToImage: model.supports_text_to_image,
        supportsImageToImage: model.supports_image_to_image,
      };

      if (existingModel) {
        existingModel.supportsTextToImage =
          existingModel.supportsTextToImage || model.supports_text_to_image;
        existingModel.supportsImageToImage =
          existingModel.supportsImageToImage || model.supports_image_to_image;

        if (!existingModel.availableChannels.some((item) => item.id === channel.id)) {
          existingModel.availableChannels.push(nextChannelOption);
        }
        continue;
      }

      modelMap.set(model.id, {
        id: model.id,
        label: model.name?.trim() || model.id,
        supportsTextToImage: model.supports_text_to_image,
        supportsImageToImage: model.supports_image_to_image,
        availableChannels: [nextChannelOption],
      });
    }
  }

  return [...modelMap.values()]
    .map((item) => ({
      ...item,
      availableChannels: [...item.availableChannels].sort(
        (left, right) => left.sortOrder - right.sortOrder
      ),
    }))
    .sort((left, right) => left.label.localeCompare(right.label));
};

export const filterModelsByMode = (
  modelOptions: WorkbenchModelOption[],
  mode: ImageModeKey
): WorkbenchModelOption[] => (
  modelOptions.filter((modelOption) => supportsImageMode(modelOption, mode))
);

export const getAvailableChannelsForMode = (
  modelOption: WorkbenchModelOption | null,
  mode: ImageModeKey
): WorkbenchChannelOption[] => (
  modelOption?.availableChannels.filter((channelOption) => supportsImageMode(channelOption, mode)) ?? []
);

export const resolveWorkbenchSelection = ({
  mode,
  modelId,
  channelId,
  modelOptions,
  preferFirstChannel = false,
}: ResolveWorkbenchSelectionInput): { modelId: string; channelId: string } => {
  const availableModels = filterModelsByMode(modelOptions, mode);
  const selectedModel = availableModels.find((item) => item.id === modelId) ?? availableModels[0] ?? null;

  if (!selectedModel) {
    return { modelId: '', channelId: '' };
  }

  const availableChannels = getAvailableChannelsForMode(selectedModel, mode);
  const selectedChannelId =
    !preferFirstChannel && availableChannels.some((item) => item.id === channelId)
      ? channelId
      : availableChannels[0]?.id ?? '';

  return {
    modelId: selectedModel.id,
    channelId: selectedChannelId,
  };
};
