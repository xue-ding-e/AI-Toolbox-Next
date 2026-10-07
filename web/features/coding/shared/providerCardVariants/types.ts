import type React from 'react';
import type {
  ModelDisplayData,
  ProviderConnectivityStatusItem,
} from '@/components/common/ProviderCard/types';

/**
 * The data every provider card style needs, regardless of how it lays it out.
 *
 * Each CLI maps its own provider record into this shape, so the three styles
 * stay presentation-only and never reach into a CLI's storage format.
 */
export interface ProviderCardModel {
  id: string;
  name: string;
  /** `@ai-sdk/...` family, shown as a tag. Empty hides the tag. */
  sdkName?: string;
  baseUrl?: string;
  /**
   * Free-form second-line facts, rendered in the order given.
   *
   * The three styles differ in *where* the second line goes, not in what it
   * carries, so the caller decides the content and the style decides the
   * placement. A `code` entry renders monospaced (ids, model names); a `text`
   * entry renders secondary (labels, notes).
   */
  meta?: ProviderCardMetaEntry[];
}

export interface ProviderCardMetaEntry {
  kind: 'id' | 'code' | 'text' | 'tag' | 'sdk';
  value: string;
  /** Only meaningful for `tag`: the Ant Design color. */
  color?: string;
}

/**
 * Actions rendered in the card header, in this order. Omit one to hide it.
 *
 * The styles place these differently — the OpenCode style renders them as
 * icon buttons with the toggle first, the Claude/Codex styles render the
 * primary actions as text links and fold the rest into a "more" menu — but
 * the set is the same, so a CLI wires its handlers once.
 */
export interface ProviderCardActions {
  onEdit?: () => void;
  onCopy?: () => void;
  onShare?: () => void;
  onDelete?: () => void;
  /** Disables the delete button and explains why on hover. */
  deleteDisabledReason?: string;
  /** Wraps delete in a Popconfirm; the OpenCode style relies on the caller's own confirm. */
  deleteConfirm?: boolean;
  /**
   * Primary action, rendered as a text link ("应用" / "设为默认") in the Claude
   * and Codex styles. The OpenCode style has no header-level primary action:
   * its equivalent lives on the model row, so it ignores this.
   */
  primaryAction?: {
    label: string;
    icon?: React.ReactNode;
    onClick: () => void;
    disabled?: boolean;
    loading?: boolean;
    tooltip?: string;
    /** A disabled primary action with a tooltip still renders, greyed out. */
    locked?: boolean;
  };
  /** Extra header actions for the OpenCode style (batch delete, connectivity). */
  extraActions?: React.ReactNode;
}

export interface ProviderCardState {
  /** The provider is the one currently applied / selected. */
  isApplied?: boolean;
  /** The provider is disabled (only meaningful with `onToggleDisabled`). */
  isDisabled?: boolean;
  onToggleDisabled?: () => void;
  connectivityStatus?: ProviderConnectivityStatusItem;
  /** Batch-selection mode: the drag handle is replaced by a checkbox. */
  selectable?: boolean;
  selected?: boolean;
  onSelectChange?: (checked: boolean) => void;
  /** Renders the card at reduced opacity, e.g. while a disabled provider is listed. */
  dimmed?: boolean;
}

export interface ProviderCardModels {
  models: ModelDisplayData[];
  /** Read-only catalog rows merged into the list (OpenCode/Codex). */
  officialModels?: import('@/components/common/ProviderCard/types').OfficialModelDisplayData[];
  /** Drag handle for the card itself. */
  draggable?: boolean;
  sortableId?: string;
  /** Model-row actions. Omit one to hide its button. */
  onAddModel?: () => void;
  onEditModel?: (modelId: string) => void;
  onCopyModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  onSetPrimaryModel?: (modelId: string) => void;
  /** Flips a model's enabled flag; omit for CLIs that cannot disable one model. */
  onToggleModelDisabled?: (modelId: string, isDisabled: boolean) => void;
  onReorderModels?: (orderedModelIds: string[]) => void;
  modelsDraggable?: boolean;
  modelSelectionMode?: boolean;
  selectedModelIds?: string[];
  onToggleModelSelection?: (modelId: string, selected: boolean) => void;
  onToggleBatchDeleteMode?: () => void;
  onBatchDeleteModels?: () => void;
  onTestModels?: () => void;
  testModelsDisabled?: boolean;
  testModelsDisabledTooltip?: string;
  onFetchModels?: () => void;
  fetchDisabled?: boolean;
  fetchDisabledTooltip?: string;
  /** Short tag beside the model-section title. */
  modelSourceTag?: string;
}

/**
 * What every style receives.
 *
 * Deliberately one flat interface rather than one per style: the styles exist
 * to fix *layout*, and a caller that has to pick a different prop shape per
 * style would be re-implementing the divergence this module removes. A style
 * ignores what it does not render.
 */
export interface ProviderCardVariantProps {
  provider: ProviderCardModel;
  providerState?: ProviderCardState;
  actions?: ProviderCardActions;
  modelSection?: ProviderCardModels;
  /** Tags rendered beside the provider name (applied / disabled / official…). */
  nameTags?: React.ReactNode;
  /**
   * Free-form second line, in the order given.
   *
   * The Codex and Claude styles use this for whatever facts the CLI wants to
   * show — endpoint, model, masked key, notes, role bindings. The OpenCode
   * style ignores it and builds the line from `provider.id` / `sdkName` /
   * `baseUrl` instead, which is what makes that style's second line uniform.
   */
  metaEntries?: ProviderCardMetaEntry[];
  /** Actions rendered at the end of the meta line (e.g. an inline connectivity button). */
  inlineActions?: React.ReactNode;
  /** Free-form block below the meta line, before the model section. */
  footer?: React.ReactNode;
}
