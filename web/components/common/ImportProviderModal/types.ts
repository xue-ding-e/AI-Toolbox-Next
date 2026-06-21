import type { OpenCodeFavoriteProvider } from '@/services/opencodeApi';

export interface ImportProviderModalProps {
  open: boolean;
  onClose: () => void;
  /** Callback when providers are imported successfully */
  onImport: (providers: OpenCodeFavoriteProvider[]) => void;
  /** Provider IDs that already exist in current config */
  existingProviderIds: string[];
  /** Optional title override */
  title?: string;
  /** Optional empty description override */
  emptyDescription?: string;
  /** Translation prefix for generic provider import texts */
  i18nPrefix?: string;
  /** Optional provider filter */
  providerFilter?: (provider: OpenCodeFavoriteProvider) => boolean;
  /** Optional provider list transformer for source-specific dedupe or ordering */
  providerListTransform?: (providers: OpenCodeFavoriteProvider[]) => OpenCodeFavoriteProvider[];
}

export interface ProviderCardItemProps {
  provider: OpenCodeFavoriteProvider;
  /** Whether this provider already exists in current config */
  isExisting: boolean;
  /** Whether this provider is selected for import */
  isSelected: boolean;
  /** Callback when selection changes */
  onSelectionChange: (selected: boolean) => void;
  /** Callback when delete is confirmed */
  onDelete: () => void;
}
