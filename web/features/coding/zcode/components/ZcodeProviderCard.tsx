import React from 'react';
import { Tag } from 'antd';
import { useTranslation } from 'react-i18next';
import AppliedTag from '@/components/common/AppliedTag';
import OpenCodeStyleCard from '@/features/coding/shared/providerCardVariants/OpenCodeStyleCard';
import type { ProviderCardVariantProps } from '@/features/coding/shared/providerCardVariants/types';
import type {
  ModelDisplayData,
  ProviderConnectivityStatusItem,
} from '@/components/common/ProviderCard/types';
import type { ZcodeProvider } from '@/types/zcode';
import { parseZcodeProviderSettings } from '../utils/zcodeSettingsConfig';
import { zcodeSdkName } from '../utils/zcodeFavoriteProvider';

interface ZcodeProviderCardProps {
  provider: ZcodeProvider;
  onEdit: () => void;
  /** 复制成一条新 provider（预填来源值、新 id）。 */
  onCopy?: () => void;
  onDelete: () => void;
  /** Renders a checkbox instead of the drag handle while batch selection is on. */
  selectable?: boolean;
  selected?: boolean;
  onSelectChange?: (selected: boolean) => void;
  /** Latest result of the batch connectivity test for this provider. */
  connectivityStatus?: ProviderConnectivityStatusItem;
  /** Flips the provider's disabled flag. */
  onToggleDisabled: () => void;

  /** Model catalog actions. Omit a handler to hide its button. */
  onAddModel?: () => void;
  onEditModel?: (modelId: string) => void;
  onCopyModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  /**
   * Makes this model the one ZCode starts new sessions with.
   *
   * This is the card's only "apply" path: a ZCode provider is never switched on
   * or off, so there is nothing to apply at the provider level.
   */
  onSetPrimaryModel?: (modelId: string) => void;
  /** Switches one model on or off in ZCode's catalog. */
  onToggleModelDisabled?: (modelId: string, isDisabled: boolean) => void;
  onReorderModels?: (orderedModelIds: string[]) => void;
  /** Connectivity test for the whole catalog; hidden while it cannot run. */
  onTestModels?: () => void;
  testModelsDisabled?: boolean;
  testModelsDisabledTooltip?: string;
  onFetchModels?: () => void;
  modelSelectionMode?: boolean;
  selectedModelIds?: string[];
  onToggleModelSelection?: (modelId: string, selected: boolean) => void;
  onToggleBatchDeleteMode?: () => void;
  onBatchDeleteModels?: () => void;
}

/**
 * A ZCode provider, rendered in the OpenCode style.
 *
 * The layout lives in the shared variant; this file only maps ZCode's storage
 * shape onto it, so the card cannot drift from the other OpenCode-style CLIs.
 *
 * Two ZCode-specific facts drive the mapping:
 *
 * - The provider's API format and endpoint live inside the `settings_config`
 *   JSON blob, not on the row, so they are parsed out here.
 * - A provider is always usable; "which one is active" is expressed per model,
 *   not per card. That is why the header carries no apply button and the
 *   `设为默认` action sits on the model row.
 */
const ZcodeProviderCard: React.FC<ZcodeProviderCardProps> = ({
  provider,
  onEdit,
  onCopy,
  onDelete,
  selectable = false,
  selected = false,
  onSelectChange,
  connectivityStatus,
  onToggleDisabled,
  onAddModel,
  onEditModel,
  onCopyModel,
  onDeleteModel,
  onSetPrimaryModel,
  onToggleModelDisabled,
  onReorderModels,
  onTestModels,
  testModelsDisabled = false,
  testModelsDisabledTooltip,
  onFetchModels,
  modelSelectionMode = false,
  selectedModelIds = [],
  onToggleModelSelection,
  onToggleBatchDeleteMode,
  onBatchDeleteModels,
}) => {
  const { t } = useTranslation();
  const settings = parseZcodeProviderSettings(provider.settingsConfig);
  const apiType = settings?.config?.api?.type;
  const baseUrl = settings?.config?.api?.baseUrl ?? '';
  const models = settings?.models ?? [];
  const defaultModelId = models.find((model) => model.isDefault)?.modelId;

  /**
   * `ModelListSection` renders `ModelDisplayData`, so each ZCode row is mapped
   * here. The row key is the `modelId` — ZCode keys models by it, and the
   * default flag travels separately in `isPrimary`.
   */
  const modelRows = React.useMemo<ModelDisplayData[]>(
    () =>
      models.map((model) => ({
        id: model.modelId,
        name: model.displayName?.trim() || model.modelId,
        contextLimit: model.properties?.contextWindow,
        outputLimit: model.optionSpecs?.maxOutputTokens?.max,
        isPrimary: model.modelId === defaultModelId,
        // An absent flag means enabled — only an explicit `false` is off.
        isDisabled: model.enabled === false,
      })),
    [models, defaultModelId],
  );

  const nameTags = (
    <>
      {provider.isApplied && (
        <AppliedTag>{t('zcode.provider.applied', { defaultValue: '默认' })}</AppliedTag>
      )}
      {provider.isDisabled && (
        <Tag style={{ margin: 0 }}>{t('zcode.provider.disabled', { defaultValue: '已禁用' })}</Tag>
      )}
    </>
  );

  const props: ProviderCardVariantProps = {
    provider: {
      id: provider.id,
      name: provider.name,
      sdkName: zcodeSdkName(apiType),
      baseUrl,
    },
    providerState: {
      isDisabled: provider.isDisabled,
      onToggleDisabled,
      // Card-level drag handle: the provider order is what is being sorted here.
      draggable: !selectable,
      sortableId: provider.id,
      connectivityStatus,
      selectable,
      selected,
      onSelectChange,
      dimmed: provider.isDisabled,
    },
    actions: {
      onEdit,
      onCopy,
      onDelete,
      deleteConfirm: false,
    },
    nameTags,
    modelSection: {
      models: modelRows,
      modelsDraggable: !modelSelectionMode && Boolean(onReorderModels),
      onReorderModels,
      modelSelectionMode,
      selectedModelIds,
      onToggleModelSelection,
      onToggleBatchDeleteMode,
      onBatchDeleteModels,
      onTestModels,
      testModelsDisabled,
      testModelsDisabledTooltip,
      onFetchModels,
      fetchDisabled: !baseUrl,
      fetchDisabledTooltip: t('opencode.provider.completeUrlAndKey'),
      onAddModel,
      onEditModel,
      onCopyModel,
      onDeleteModel,
      onSetPrimaryModel,
      onToggleModelDisabled,
    },
  };

  return <OpenCodeStyleCard {...props} />;
};

export default ZcodeProviderCard;
