export interface ExternalProviderDisplayItem<TConfig> {
  providerId: string;
  name: string;
  baseUrl?: string;
  accountLabel: string;
  siteName?: string;
  siteType?: string;
  sourceProfileName: string;
  sourceExtensionId: string;
  requiresBrowserOpen?: boolean;
  isDisabled?: boolean;
  hasApiKey: boolean;
  apiKeyPreview?: string;
  balanceUsd?: number;
  balanceCny?: number;
  models?: string[];
  modelsStatus?: 'idle' | 'loading' | 'loaded' | 'error' | 'unsupported';
  modelsError?: string;
  config: TConfig;
  secondaryLabel?: string;
}

export interface ImportExternalProvidersModalProps<TConfig> {
  open: boolean;
  title: string;
  loading: boolean;
  items: ExternalProviderDisplayItem<TConfig>[];
  existingProviderIds: string[];
  emptyDescription: string;
  cancelText: string;
  importButtonText: string;
  selectAllText: string;
  deselectAllText: string;
  existingTagText: string;
  noApiKeyTagText: string;
  disabledTagText: string;
  balanceLabelText: string;
  accountLabelText: string;
  modelsLabelText: string;
  loadingModelsText: string;
  emptyModelsText: string;
  modelsErrorText: string;
  unsupportedModelsText: string;
  unsupportedImportText: string;
  expandModelsText: string;
  collapseModelsText: string;
  profileLabel: string;
  siteTypeLabel: string;
  loadingTokenText: string;
  tokenResolvedText: string;
  retryResolveText: string;
  searchPlaceholder: string;
  onCancel: () => void;
  onImport: (items: ExternalProviderDisplayItem<TConfig>[]) => void;
  onResolveToken?: (providerId: string) => Promise<boolean>;
}
