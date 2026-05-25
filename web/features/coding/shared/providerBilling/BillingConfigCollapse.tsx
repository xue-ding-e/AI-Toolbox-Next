import React from 'react';
import { Input, Select, Switch } from 'antd';
import { DollarOutlined, DownOutlined, RightOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
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

  const handleHeaderKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      setExpanded((current) => !current);
    }
  };

  const sourceOptions: Array<{ value: BillingPricingModelSource; label: string }> = [
    { value: 'inherit', label: t('providerBilling.pricingModelSourceInherit') },
    { value: 'requested', label: t('providerBilling.pricingModelSourceRequested') },
    { value: 'upstream', label: t('providerBilling.pricingModelSourceUpstream') },
  ];

  return (
    <div className={[styles.billingConfig, className].filter(Boolean).join(' ')}>
      <div
        className={styles.header}
        role="button"
        tabIndex={0}
        onClick={() => setExpanded((current) => !current)}
        onKeyDown={handleHeaderKeyDown}
      >
        <div className={styles.title}>
          <DollarOutlined className={styles.titleIcon} />
          <span>{t('providerBilling.title')}</span>
        </div>
        <div className={styles.headerActions}>
          <div
            className={styles.switchWrap}
            onClick={(event) => event.stopPropagation()}
          >
            <span>{t('providerBilling.useCustom')}</span>
            <Switch
              size="small"
              checked={value.enabled}
              onChange={(enabled) => {
                updateConfig({ enabled });
                if (enabled) {
                  setExpanded(true);
                }
              }}
            />
          </div>
          {expanded ? (
            <DownOutlined className={styles.chevron} />
          ) : (
            <RightOutlined className={styles.chevron} />
          )}
        </div>
      </div>
      <div className={`${styles.bodyWrap} ${expanded ? styles.expanded : ''}`}>
        <div className={styles.body}>
          <p className={styles.description}>
            {t('providerBilling.description')}
          </p>
          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.fieldLabel}>{t('providerBilling.costMultiplier')}</span>
              <Input
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
              <Select
                value={value.pricingModelSource}
                disabled={!value.enabled}
                options={sourceOptions}
                onChange={(pricingModelSource) => updateConfig({ pricingModelSource })}
              />
              <span className={styles.fieldHint}>
                {t('providerBilling.pricingModelSourceHint')}
              </span>
            </label>
          </div>
        </div>
      </div>
    </div>
  );
};

export default BillingConfigCollapse;
