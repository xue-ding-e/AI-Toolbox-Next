import React from 'react';
import { Globe, Plus, Trash2 } from 'lucide-react';
import { Dropdown, type MenuProps } from 'antd';
import { useTranslation } from 'react-i18next';
import ProviderConfigCollapse from '@/features/coding/shared/providerConfig/ProviderConfigCollapse';
import {
  emptyHeaderEntry,
  type CustomHeaderEntry,
  type CustomHeaderOp,
  type CustomHeadersState,
} from './customHeadersUtils';
import { HEADER_USER_AGENT_PRESETS, userAgentPresetToHeaderEntry } from './headerPresets';
import { validateHeaderEntry } from './headerValidation';
import styles from './CustomHeadersCollapse.module.less';

interface CustomHeadersCollapseProps {
  value: CustomHeadersState;
  onChange: (value: CustomHeadersState) => void;
  className?: string;
}

const HEADER_OPS: { value: CustomHeaderOp; labelKey: string }[] = [
  { value: 'set', labelKey: 'providerHeaders.opSet' },
  { value: 'delete', labelKey: 'providerHeaders.opDelete' },
  { value: 'rename', labelKey: 'providerHeaders.opRename' },
  { value: 'copy', labelKey: 'providerHeaders.opCopy' },
];

/**
 * Provider-level custom request-header override editor. Mirrors the
 * `BillingConfigCollapse` / `CustomUserAgentCollapse` pattern: a standalone
 * collapse (collapsed by default) with a "use custom" toggle in the actions
 * slot; the body holds a list of override rows plus a User-Agent preset
 * dropdown. Invalid rows show a non-blocking red hint — the runtime silently
 * ignores illegal values, matching `inject_custom_headers` on the Rust side.
 */
const CustomHeadersCollapse: React.FC<CustomHeadersCollapseProps> = ({
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

  const update = React.useCallback(
    (patch: Partial<CustomHeadersState>) => {
      onChange({ ...value, ...patch });
    },
    [onChange, value],
  );

  const updateRow = (index: number, patch: Partial<CustomHeaderEntry>) => {
    const next = value.headers.map((row, i) =>
      i === index ? { ...row, ...patch } : row,
    );
    update({ headers: next });
  };

  const addRow = () => {
    update({ headers: [...value.headers, emptyHeaderEntry()] });
  };

  const removeRow = (index: number) => {
    const next = value.headers.filter((_, i) => i !== index);
    update({ headers: next.length > 0 ? next : [emptyHeaderEntry()] });
  };

  const presetMenu: MenuProps = {
    items: HEADER_USER_AGENT_PRESETS.map((preset) => ({ key: preset, label: preset })),
    onClick: ({ key }) => {
      update({ headers: [...value.headers, userAgentPresetToHeaderEntry(key)] });
    },
  };

  const disabled = !value.enabled;

  return (
    <ProviderConfigCollapse
      className={className}
      title={t('providerHeaders.title')}
      expanded={expanded}
      onExpandedChange={setExpanded}
      icon={<Globe />}
      actions={
        <div
          className={styles.toggleWrap}
          onClick={(event) => event.stopPropagation()}
        >
          <span>{t('providerHeaders.useCustom')}</span>
          <button
            type="button"
            className={`${styles.toggleButton} ${value.enabled ? styles.toggleButtonActive : ''}`}
            role="switch"
            aria-checked={value.enabled}
            onClick={() => {
              const enabled = !value.enabled;
              update({ enabled });
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
      <p className={styles.description}>{t('providerHeaders.description')}</p>
      <div className={styles.rows}>
        {value.headers.map((row, index) => {
          const validation = validateHeaderEntry(row);
          const showInvalid = validation.meaningful && !validation.valid;
          const isRenameOrCopy = row.op === 'rename' || row.op === 'copy';
          return (
            <div key={index} className={styles.row}>
              <select
                className={styles.opSelect}
                value={row.op}
                disabled={disabled}
                onChange={(event) =>
                  updateRow(index, { op: event.target.value as CustomHeaderOp })
                }
              >
                {HEADER_OPS.map((op) => (
                  <option key={op.value} value={op.value}>
                    {t(op.labelKey)}
                  </option>
                ))}
              </select>
              {isRenameOrCopy ? (
                <>
                  <input
                    className={`${styles.fieldInput} ${showInvalid ? styles.fieldInputInvalid : ''}`}
                    type="text"
                    autoComplete="off"
                    placeholder={t('providerHeaders.fromPlaceholder')}
                    value={row.from}
                    disabled={disabled}
                    onChange={(event) => updateRow(index, { from: event.target.value })}
                  />
                  <span className={styles.arrow}>→</span>
                  <input
                    className={`${styles.fieldInput} ${showInvalid ? styles.fieldInputInvalid : ''}`}
                    type="text"
                    autoComplete="off"
                    placeholder={t('providerHeaders.toPlaceholder')}
                    value={row.to}
                    disabled={disabled}
                    onChange={(event) => updateRow(index, { to: event.target.value })}
                  />
                </>
              ) : (
                <>
                  <input
                    className={`${styles.fieldInput} ${showInvalid ? styles.fieldInputInvalid : ''}`}
                    type="text"
                    autoComplete="off"
                    placeholder={t('providerHeaders.namePlaceholder')}
                    value={row.name}
                    disabled={disabled}
                    onChange={(event) => updateRow(index, { name: event.target.value })}
                  />
                  {row.op === 'set' && (
                    <input
                      className={`${styles.fieldInput} ${showInvalid ? styles.fieldInputInvalid : ''}`}
                      type="text"
                      autoComplete="off"
                      placeholder={t('providerHeaders.valuePlaceholder')}
                      value={row.value}
                      disabled={disabled}
                      onChange={(event) => updateRow(index, { value: event.target.value })}
                    />
                  )}
                </>
              )}
              <button
                type="button"
                className={styles.iconButton}
                onClick={() => removeRow(index)}
                disabled={disabled}
                aria-label={t('providerHeaders.remove')}
              >
                <Trash2 size={14} />
              </button>
              {showInvalid && (
                <span className={styles.rowHint}>{t('providerHeaders.invalid')}</span>
              )}
            </div>
          );
        })}
      </div>
      <div className={styles.toolbar}>
        <button
          type="button"
          className={styles.addButton}
          onClick={addRow}
          disabled={disabled}
        >
          <Plus size={14} style={{ marginRight: 4, verticalAlign: -2 }} />
          {t('providerHeaders.add')}
        </button>
        <Dropdown menu={presetMenu} trigger={['click']} disabled={disabled}>
          <button type="button" className={styles.addButton} disabled={disabled}>
            {t('providerHeaders.presets')}
          </button>
        </Dropdown>
      </div>
    </ProviderConfigCollapse>
  );
};

export default CustomHeadersCollapse;
