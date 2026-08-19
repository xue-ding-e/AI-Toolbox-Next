import React from 'react';
import { Globe } from 'lucide-react';
import { Dropdown, type MenuProps } from 'antd';
import { useTranslation } from 'react-i18next';
import ProviderConfigCollapse from '@/features/coding/shared/providerConfig/ProviderConfigCollapse';
import { isValidUserAgentHeader } from './userAgentValidation';
import { USER_AGENT_PRESETS } from './userAgentPresets';
import type { CustomUserAgentState } from './customUserAgentUtils';
import styles from './CustomUserAgentCollapse.module.less';

interface CustomUserAgentCollapseProps {
  value: CustomUserAgentState;
  onChange: (value: CustomUserAgentState) => void;
  className?: string;
}

/**
 * Provider-level custom User-Agent editor. Mirrors `BillingConfigCollapse`:
 * a standalone collapse (collapsed by default) with a "use custom" toggle in
 * the actions slot; the body holds a text input plus a preset dropdown.
 * Invalid input (control chars) shows a non-blocking red hint — the runtime
 * silently ignores illegal values, matching `parse_custom_user_agent` on the
 * Rust side.
 */
const CustomUserAgentCollapse: React.FC<CustomUserAgentCollapseProps> = ({
  value,
  onChange,
  className,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = React.useState(false);

  React.useEffect(() => {
    if (value.enabled) {
      setExpanded(true);
    }
  }, [value.enabled]);

  const updateConfig = React.useCallback(
    (patch: Partial<CustomUserAgentState>) => {
      onChange({ ...value, ...patch });
    },
    [onChange, value],
  );

  const valid = isValidUserAgentHeader(value.value);

  const presetMenu: MenuProps = {
    items: USER_AGENT_PRESETS.map((preset) => ({ key: preset, label: preset })),
    onClick: ({ key }) => updateConfig({ value: key }),
  };

  return (
    <ProviderConfigCollapse
      className={className}
      title={t('providerUserAgent.title')}
      expanded={expanded}
      onExpandedChange={setExpanded}
      icon={<Globe />}
      actions={
        <div
          className={styles.toggleWrap}
          onClick={(event) => event.stopPropagation()}
        >
          <span>{t('providerUserAgent.useCustom')}</span>
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
      }
    >
      <p className={styles.description}>{t('providerUserAgent.description')}</p>
      <div className={styles.fields}>
        <label className={styles.field}>
          <span className={styles.fieldLabel}>{t('providerUserAgent.userAgent')}</span>
          <div className={styles.inputRow}>
            <input
              className={styles.control}
              type="text"
              autoComplete="off"
              value={value.value}
              disabled={!value.enabled}
              placeholder={t('providerUserAgent.placeholder')}
              onChange={(event) => updateConfig({ value: event.target.value })}
            />
            <Dropdown menu={presetMenu} trigger={['click']} disabled={!value.enabled}>
              <button
                type="button"
                className={styles.presetButton}
                disabled={!value.enabled}
              >
                {t('providerUserAgent.presets')}
              </button>
            </Dropdown>
          </div>
          {valid ? (
            <span className={styles.fieldHint}>{t('providerUserAgent.hint')}</span>
          ) : (
            <span className={styles.fieldInvalid}>
              {t('providerUserAgent.invalid')}
            </span>
          )}
        </label>
      </div>
    </ProviderConfigCollapse>
  );
};

export default CustomUserAgentCollapse;
