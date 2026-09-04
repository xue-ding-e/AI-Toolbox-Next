import { Button, Checkbox, Popover, Space, Typography } from 'antd';
import { ThunderboltOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';

/**
 * Canonical service (speed) tier ids Codex understands. The backend drops
 * unknown ids, so the UI only offers canonical ones. Each id expands to a
 * full `{ id, name, description }` object at catalog-generation time. Mirrors
 * `CODEX_SERVICE_TIER_ENTRIES` in `tauri/src/coding/codex/commands.rs`.
 *
 * `SERVICE_TIER_LABELS` are proper nouns (the `name` field codex writes into
 * the catalog), not translatable copy, so they are a static map rather than
 * i18n keys — mirrors how `ReasoningLevelsEditor` renders raw effort ids.
 */
const CODEX_SERVICE_TIERS = ['priority', 'ultrafast'] as const;
const SERVICE_TIER_LABELS: Record<string, string> = {
  priority: 'Fast',
  ultrafast: 'Ultrafast',
};

interface ServiceTiersEditorProps {
  tiers?: string[];
  onTiersChange: (tiers: string[] | undefined) => void;
}

/**
 * Per-model service (speed) tier editor for Codex model-mapping rows. A compact
 * button trigger opens a popover with a checkbox list of the canonical tiers
 * (Fast / Ultrafast, multi-select, stays open). Unlike reasoning levels there
 * is no "default" selector — `service_tiers` only declares which speed tiers
 * the model advertises; codex picks the active tier at request time.
 *
 * Ported from the same pattern as `ReasoningLevelsEditor` (antd).
 */
function ServiceTiersEditor({ tiers, onTiersChange }: ServiceTiersEditorProps) {
  const { t } = useTranslation();

  // Re-filter incoming tiers against the canonical list so a stored typo is
  // silently dropped before rendering.
  const selected = (tiers ?? []).filter((tier) =>
    (CODEX_SERVICE_TIERS as readonly string[]).includes(tier),
  );

  const triggerLabel =
    selected.length > 0
      ? selected.map((tier) => SERVICE_TIER_LABELS[tier] ?? tier).join(', ')
      : t('codex.provider.modelMappingServiceTiersPlaceholder');

  const content = (
    <Space direction="vertical" size={8} style={{ width: 220 }}>
      <Checkbox.Group
        value={selected}
        onChange={(checkedValues) => {
          // Checkbox.Group emits the full checked array in click order; re-filter
          // to canonical order so storage is deterministic.
          const next = (CODEX_SERVICE_TIERS as readonly string[]).filter((item) =>
            (checkedValues as string[]).includes(item),
          );
          onTiersChange(next.length > 0 ? next : undefined);
        }}
        style={{ display: 'flex', flexDirection: 'column', gap: 6 }}
      >
        {CODEX_SERVICE_TIERS.map((tier) => (
          <Checkbox key={tier} value={tier}>
            {SERVICE_TIER_LABELS[tier] ?? tier}
          </Checkbox>
        ))}
      </Checkbox.Group>
      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
        {t('codex.provider.modelMappingServiceTiersHint')}
      </Typography.Text>
    </Space>
  );

  return (
    <Popover content={content} trigger="click" placement="bottomLeft">
      <Button
        type="default"
        block
        icon={<ThunderboltOutlined />}
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

export default ServiceTiersEditor;

