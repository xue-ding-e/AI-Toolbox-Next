export interface OmpPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

export interface OmpSettingsConfig {
  rootDir?: string | null;
  updatedAt: string;
}

export interface OmpSettingsConfigInput {
  rootDir?: string | null;
  clearRootDir?: boolean;
}

export interface OmpRuntimeConfig {
  rootPathInfo: OmpPathInfo;
  modelsPath: string;
  mcpPath: string;
  models: Record<string, unknown>;
  providers: Record<string, Record<string, unknown>>;
}

export interface OmpProviderInput {
  providerKey: string;
  provider: Record<string, unknown>;
}
