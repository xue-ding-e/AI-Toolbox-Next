export interface ExternalProviderDisplayItem<TConfig> {
  providerId: string;
  name: string;
  baseUrl?: string;
  accountLabel: string;
  siteName?: string;
  siteType?: string;
  sourceProfileName: string;
  sourceExtensionId: string;
  isDisabled?: boolean;
  hasApiKey: boolean;
  apiKeyPreview?: string;
  balanceUsd?: number;
  balanceCny?: number;
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
  profileLabel: string;
  siteTypeLabel: string;
  loadingTokenText: string;
  tokenResolvedText: string;
  searchPlaceholder: string;
  onCancel: () => void;
  onImport: (items: ExternalProviderDisplayItem<TConfig>[]) => void;
  onResolveToken?: (providerId: string) => Promise<boolean>;
}
