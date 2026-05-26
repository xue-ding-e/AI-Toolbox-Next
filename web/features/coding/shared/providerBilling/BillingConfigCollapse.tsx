import React from 'react';
import { DollarSign } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import ProviderConfigCollapse from '@/features/coding/shared/providerConfig/ProviderConfigCollapse';
import type { BillingConfigState, BillingPricingModelSource } from './billingConfigUtils';
import styles from './BillingConfigCollapse.module.less';

interface BillingConfigCollapseProps {
  value: BillingConfigState;
  onChange: (value: BillingConfigState) => void;
  className?: string;
}

const BillingConfigCollapse: React.FC<BillingConfigCollapseProps> = ({
  value,
  onChange,
  className,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = React.useState(value.enabled);

  React.useEffect(() => {
    if (value.enabled) {
      setExpanded(true);
    }
  }, [value.enabled]);

  const updateConfig = React.useCallback((patch: Partial<BillingConfigState>) => {
    onChange({
      ...value,
      ...patch,
    });
  }, [onChange, value]);

  const sourceOptions: Array<{ value: BillingPricingModelSource; label: string }> = [
    { value: 'inherit', label: t('providerBilling.pricingModelSourceInherit') },
    { value: 'requested', label: t('providerBilling.pricingModelSourceRequested') },
    { value: 'upstream', label: t('providerBilling.pricingModelSourceUpstream') },
  ];

  return (
    <ProviderConfigCollapse
      className={className}
      title={t('providerBilling.title')}
      expanded={expanded}
      onExpandedChange={setExpanded}
      icon={<DollarSign />}
      actions={(
        <div
          className={styles.toggleWrap}
          onClick={(event) => event.stopPropagation()}
        >
          <span>{t('providerBilling.useCustom')}</span>
          <button
            type="button"
            className={`${styles.toggleButton} ${value.enabled ? styles.toggleButtonActive : ''}`}
            role="switch"
            aria-checked={value.enabled}
            onClick={() => {
              const enabled = !value.enabled;
              updateConfig({ enabled });
              if (enabled) {
                setExpanded(true);
              }
            }}
          >
            <span className={styles.toggleKnob} />
          </button>
        </div>
      )}
    >
      <p className={styles.description}>
        {t('providerBilling.description')}
      </p>
      <div className={styles.fields}>
        <label className={styles.field}>
          <span className={styles.fieldLabel}>{t('providerBilling.costMultiplier')}</span>
          <input
            className={styles.control}
            type="number"
            step="0.01"
            min="0"
            inputMode="decimal"
            value={value.costMultiplier || ''}
            disabled={!value.enabled}
            placeholder={t('providerBilling.costMultiplierPlaceholder')}
            onChange={(event) => updateConfig({
              costMultiplier: event.target.value || undefined,
            })}
          />
          <span className={styles.fieldHint}>
            {t('providerBilling.costMultiplierHint')}
          </span>
        </label>
        <label className={styles.field}>
          <span className={styles.fieldLabel}>{t('providerBilling.pricingModelSource')}</span>
          <select
            className={styles.control}
            value={value.pricingModelSource}
            disabled={!value.enabled}
            onChange={(event) => updateConfig({
              pricingModelSource: event.target.value as BillingPricingModelSource,
            })}
          >
            {sourceOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <span className={styles.fieldHint}>
            {t('providerBilling.pricingModelSourceHint')}
          </span>
        </label>
      </div>
    </ProviderConfigCollapse>
  );
};

export default BillingConfigCollapse;
