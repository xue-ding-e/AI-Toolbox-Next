import assert from 'node:assert/strict';
import test from 'node:test';
import type { KimiOfficialAccount, KimiProvider } from '../../../../../types/kimi.ts';
import { canDeleteKimiProvider } from '../../../../../features/coding/kimi/utils/providerDeletion.ts';

const makeProvider = (id: string, isApplied = false): KimiProvider => ({
  id, name: id, category: 'custom', settingsConfig: '{}',
  isApplied, isDisabled: false, createdAt: '', updatedAt: '',
});

test('Kimi batch selection excludes local, applied, and account-owned providers', () => {
  const providers = [
    makeProvider('__local__'), makeProvider('active', true),
    makeProvider('official'), makeProvider('unused'),
  ];
  const accounts: KimiOfficialAccount[] = [{
    id: 'account', providerId: 'official', name: 'Account', kind: 'oauth',
    isApplied: false, createdAt: '', updatedAt: '',
  }];
  const selected = providers.filter((provider) => canDeleteKimiProvider(provider, accounts));
  assert.deepEqual(selected.map((provider) => provider.id), ['unused']);
  const remaining = providers.filter((provider) => !selected.includes(provider));
  assert.equal(remaining.find((provider) => provider.id === 'active')?.isApplied, true);
});

test('an official provider can be deleted after its accounts are removed and another provider is applied', () => {
  const provider = { ...makeProvider('official'), category: 'official' };
  assert.equal(canDeleteKimiProvider(provider, []), true);
});
