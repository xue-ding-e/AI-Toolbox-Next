import React from 'react';
import { ArrowLeftRight, Plus, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import ProviderConfigCollapse from '@/features/coding/shared/providerConfig/ProviderConfigCollapse';
import {
  emptyModelRewriteEntry,
  type ModelRewriteEntry,
  type ModelRewritesState,
} from './modelRewritesUtils';
import styles from './ModelRewritesCollapse.module.less';

interface ModelRewritesCollapseProps {
  value: ModelRewritesState;
  onChange: (value: ModelRewritesState) => void;
  className?: string;
}

/**
 * Provider-level exact model rewrite editor (issue #321). Mirrors the
 * `CustomHeadersCollapse` pattern: a standalone collapse (collapsed by
 * default) with a "use custom" toggle in the actions slot; the body holds a
 * list of `requested model -> upstream model` rows. The gateway matches the
 * requested model exactly (trim + case-insensitive, `[1M]` stripped) and
 * forwards the mapped model in every proxy mode; connectivity tests keep
 * the pinned model.
 */
const ModelRewritesCollapse: React.FC<ModelRewritesCollapseProps> = ({
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
    (patch: Partial<ModelRewritesState>) => {
      onChange({ ...value, ...patch });
    },
    [onChange, value],
  );

  const updateRow = (index: number, patch: Partial<ModelRewriteEntry>) => {
    const next = value.rewrites.map((row, i) =>
      i === index ? { ...row, ...patch } : row,
    );
    update({ rewrites: next });
  };

  const addRow = () => {
    update({ rewrites: [...value.rewrites, emptyModelRewriteEntry()] });
  };

  const removeRow = (index: number) => {
    const next = value.rewrites.filter((_, i) => i !== index);
    update({ rewrites: next.length > 0 ? next : [emptyModelRewriteEntry()] });
  };

  const disabled = !value.enabled;

  return (
    <ProviderConfigCollapse
      className={className}
      title={t('providerModelRewrites.title')}
      expanded={expanded}
      onExpandedChange={setExpanded}
      icon={<ArrowLeftRight />}
      actions={
        <div
          className={styles.toggleWrap}
          onClick={(event) => event.stopPropagation()}
        >
          <span>{t('providerModelRewrites.useCustom')}</span>
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
      <p className={styles.description}>{t('providerModelRewrites.description')}</p>
      <div className={styles.rows}>
        {value.rewrites.map((row, index) => (
          <div key={index} className={styles.row}>
            <input
              className={styles.fieldInput}
              type="text"
              autoComplete="off"
              placeholder={t('providerModelRewrites.fromPlaceholder')}
              value={row.from}
              disabled={disabled}
              onChange={(event) => updateRow(index, { from: event.target.value })}
            />
            <span className={styles.arrow}>→</span>
            <input
              className={styles.fieldInput}
              type="text"
              autoComplete="off"
              placeholder={t('providerModelRewrites.toPlaceholder')}
              value={row.to}
              disabled={disabled}
              onChange={(event) => updateRow(index, { to: event.target.value })}
            />
            <button
              type="button"
              className={styles.iconButton}
              onClick={() => removeRow(index)}
              disabled={disabled}
              aria-label={t('providerModelRewrites.remove')}
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}
      </div>
      <div className={styles.toolbar}>
        <button
          type="button"
          className={styles.addButton}
          onClick={addRow}
          disabled={disabled}
        >
          <Plus size={14} style={{ marginRight: 4, verticalAlign: -2 }} />
          {t('providerModelRewrites.add')}
        </button>
      </div>
    </ProviderConfigCollapse>
  );
};

export default ModelRewritesCollapse;
