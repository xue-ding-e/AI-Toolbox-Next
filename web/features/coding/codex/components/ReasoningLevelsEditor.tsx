import { Button, Checkbox, Divider, Popover, Select, Space, Typography } from 'antd';
import { SettingOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';

/**
 * Canonical reasoning effort levels Codex understands, in ascending depth
 * order. The backend drops unknown values, so the UI only offers canonical
 * ones. Mirrors `CODEX_REASONING_LEVEL_DESCRIPTIONS` in
 * `tauri/src/coding/codex/commands.rs`.
 */
export const CODEX_REASONING_LEVELS = [
  'none',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
  'ultra',
] as const;

/**
 * Sentinel for the default-level Select: antd Select forbids empty-string
 * option values, so "__auto__" maps back to undefined (no explicit default —
 * let the backend resolve: explicit → template default → highest declared).
 */
const AUTO_DEFAULT_REASONING_LEVEL = '__auto__';

interface ReasoningLevelsEditorProps {
  levels?: string[];
  defaultLevel?: string;
  onLevelsChange: (levels: string[] | undefined) => void;
  onDefaultLevelChange: (level: string | undefined) => void;
}

/**
 * Per-model reasoning-level editor for Codex model-mapping rows. A compact
 * button trigger opens a popover with a checkbox list of all canonical levels
 * (multi-select, stays open) plus a default-level dropdown offering an "Auto"
 * sentinel and only the currently-selected levels.
 *
 * Ported from cc-switch's `ReasoningLevelsEditor` (shadcn/Radix+cmdk) to antd.
 */
function ReasoningLevelsEditor({
  levels,
  defaultLevel,
  onLevelsChange,
  onDefaultLevelChange,
}: ReasoningLevelsEditorProps) {
  const { t } = useTranslation();

  // Re-filter incoming levels against the canonical list so a stored typo is
  // silently dropped before rendering.
  const selected = (levels ?? []).filter((level) =>
    (CODEX_REASONING_LEVELS as readonly string[]).includes(level),
  );

  const triggerLabel =
    selected.length > 0
      ? selected.join(', ')
      : t('codex.provider.modelMappingReasoningLevelsPlaceholder');

  const content = (
    <Space direction="vertical" size={8} style={{ width: 220 }}>
      <Checkbox.Group
        value={selected}
        onChange={(checkedValues) => {
          // Checkbox.Group emits the full checked array in click order; re-filter
          // to canonical order so storage is deterministic.
          const next = (CODEX_REASONING_LEVELS as readonly string[]).filter((item) =>
            (checkedValues as string[]).includes(item),
          );
          onLevelsChange(next.length > 0 ? next : undefined);
          if (defaultLevel && !next.includes(defaultLevel)) {
            onDefaultLevelChange(undefined);
          }
        }}
        style={{ display: 'flex', flexDirection: 'column', gap: 6 }}
      >
        {CODEX_REASONING_LEVELS.map((level) => (
          <Checkbox key={level} value={level}>
            {level}
          </Checkbox>
        ))}
      </Checkbox.Group>
      {selected.length > 0 && (
        <>
          <Divider style={{ margin: '4px 0' }} />
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {t('codex.provider.modelMappingDefaultReasoningLevel')}
          </Typography.Text>
          <Select
            size="small"
            value={defaultLevel ?? AUTO_DEFAULT_REASONING_LEVEL}
            onChange={(value: string) =>
              onDefaultLevelChange(
                value === AUTO_DEFAULT_REASONING_LEVEL ? undefined : value,
              )
            }
            style={{ width: '100%' }}
            options={[
              {
                value: AUTO_DEFAULT_REASONING_LEVEL,
                label: t('codex.provider.modelMappingDefaultReasoningLevelAuto'),
              },
              ...selected.map((level) => ({ value: level, label: level })),
            ]}
          />
        </>
      )}
    </Space>
  );

  return (
    <Popover content={content} trigger="click" placement="bottomLeft">
      <Button
        type="default"
        block
        icon={<SettingOutlined />}
        style={{
          justifyContent: 'flex-start',
          overflow: 'hidden',
          whiteSpace: 'nowrap',
          textOverflow: 'ellipsis',
          color: selected.length > 0 ? undefined : 'var(--color-text-tertiary)',
        }}
        title={triggerLabel}
      >
        {triggerLabel}
      </Button>
    </Popover>
  );
}

export default ReasoningLevelsEditor;
