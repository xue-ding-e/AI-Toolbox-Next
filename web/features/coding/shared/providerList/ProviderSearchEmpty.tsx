import React from 'react';
import { useTranslation } from 'react-i18next';

/**
 * Lightweight "no providers match the keyword" hint shown in place of the
 * provider list while a search filter is active. Deliberately a plain text
 * line instead of an antd Empty illustration: the default Empty artwork
 * reads like a stray search icon inside the provider section (DESIGN.md:
 * compact, real empty state, no decorative artwork).
 */
const ProviderSearchEmpty: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div
      style={{
        marginTop: 40,
        marginBottom: 40,
        textAlign: 'center',
        fontSize: 12,
        color: 'var(--color-text-tertiary)',
      }}
    >
      {t('common.providerSearch.noMatch')}
    </div>
  );
};

export default ProviderSearchEmpty;
