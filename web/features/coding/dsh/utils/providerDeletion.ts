import type { DshRuntimeProviderView } from '@/types/dsh';

/** Derive the same env-style ref for provider forms, backups, and deletion. */
export function credentialRefFromProviderKey(providerKey: string): string {
  const normalized = providerKey.trim().toUpperCase().replace(/[^A-Z0-9_]/g, '_');
  return normalized.endsWith('_API_KEY') ? normalized : `${normalized}_API_KEY`;
}

export function canDeleteDshProvider(provider: DshRuntimeProviderView): boolean {
  return !provider.isDefault
    && !provider.isBuiltin
    && provider.modelSource !== 'builtin'
    && (provider.credentialExists || Boolean(provider.provider));
}

export function buildDshProviderDeletionPlan(
  providers: readonly DshRuntimeProviderView[],
  selectedIds: readonly string[],
) {
  const providersToDelete = providers.filter(
    (provider) => selectedIds.includes(provider.providerKey) && canDeleteDshProvider(provider),
  );
  const deletedProviderIds = new Set(providersToDelete.map((provider) => provider.providerKey));
  const credentialRef = (provider: DshRuntimeProviderView) => (
    provider.apiKeyEnv || credentialRefFromProviderKey(provider.providerKey)
  );
  const retainedCredentialRefs = new Set(
    providers.filter((provider) => !deletedProviderIds.has(provider.providerKey)).map(credentialRef),
  );
  const scheduledCredentialRefs = new Set<string>();

  return providersToDelete.map((provider) => {
    const refName = credentialRef(provider);
    const deleteCredential = provider.credentialExists
      && !retainedCredentialRefs.has(refName)
      && !scheduledCredentialRefs.has(refName);
    if (deleteCredential) scheduledCredentialRefs.add(refName);
    return {
      provider,
      credentialRef: refName,
      // Capture every backup before deletion changes a shared credential ref.
      credential: provider.credentialExists && provider.apiKey
        ? { refName, value: provider.apiKey }
        : undefined,
      deleteCredential,
    };
  });
}
