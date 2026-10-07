import React from 'react';
import { Button, Dropdown, Space, Switch, Tooltip, Typography } from 'antd';
import { MoreOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import ProviderNameLink from '@/components/common/ProviderNameLink';
import ProviderConnectivityStatus from '@/features/coding/shared/providerConnectivity/ProviderConnectivityStatus';
import CardShell from './CardShell';
import type { ProviderCardVariantProps } from './types';

const { Text } = Typography;

/**
 * The Claude Code-style provider card.
 *
 * ```
 * ⠿  Name  [official] [applied]              [应用] [编辑] [更多 ▾]
 *    默认: claude-sonnet-4-5  Haiku: …  Sonnet: …  | notes  [连通性测试]
 * ```
 *
 * The distinguishing traits, against the other two styles:
 *
 * - **No model section.** These CLIs do not own a model catalog — the models
 *   live in the config the provider writes, so the card states the bindings
 *   (default / per-role) on the meta line instead of listing rows. A card that
 *   rendered a collapsible list here would imply an editor that does not exist.
 * - The **second line is a set of labelled bindings**, not a summary: each entry
 *   reads `Label: value` with the value monospaced. The Codex style uses the
 *   same line for free-form facts, and the OpenCode style for id/SDK/endpoint.
 * - The **primary action is a text link**, like the Codex style, because these
 *   CLIs do have a single active provider.
 */
const ClaudeStyleCard: React.FC<ProviderCardVariantProps> = ({
  provider,
  providerState,
  actions,
  nameTags,
  metaEntries,
  inlineActions,
  footer,
}) => {
  const { t } = useTranslation();
  const {
    isDisabled,
    onToggleDisabled,
    connectivityStatus,
    selectable = false,
    selected = false,
    onSelectChange,
    dimmed = false,
  } = providerState ?? {};

  const primary = actions?.primaryAction;

  const menuItems = [
    // The enable/disable switch lives in the menu, not the header: it is a
    // rarely-used, stateful toggle, and a Switch in the header row would sit
    // beside the primary action and compete with it.
    ...(onToggleDisabled
      ? [{
          key: 'toggle',
          label: (
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <span>{t('common.enable', { defaultValue: '启用' })}</span>
                <Text type="secondary" style={{ fontSize: 11 }}>
                  {isDisabled
                    ? t('common.provider.disabled')
                    : t('common.provider.enabled')}
                </Text>
              </div>
              <Switch checked={!isDisabled} onChange={() => onToggleDisabled()} size="small" />
            </div>
          ),
        }]
      : []),
    actions?.onCopy && { key: 'copy', label: t('common.copy', { defaultValue: '复制' }) },
    actions?.onShare && { key: 'share', label: t('common.share') },
    actions?.onDelete && { key: 'delete', label: t('common.delete', { defaultValue: '删除' }), danger: true },
  ].filter(Boolean) as { key: string; label: React.ReactNode; danger?: boolean }[];

  return (
    <CardShell
      sortableId={undefined}
      draggable={false}
      selectable={selectable}
      selected={selected}
      onSelectChange={onSelectChange}
      dimmed={dimmed}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12 }}>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
            <ProviderConnectivityStatus item={connectivityStatus} />
            <ProviderNameLink
              name={provider.name}
              baseUrl={provider.baseUrl}
              style={{ fontSize: 14, fontWeight: 600 }}
            />
            {nameTags}
          </div>

          {metaEntries && metaEntries.length > 0 && (
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: '8px 16px', flexWrap: 'wrap', marginTop: 4 }}>
              {metaEntries.map((entry, index) =>
                entry.kind === 'code' ? (
                  <div key={index}>
                    <Text code style={{ fontSize: 12 }}>
                      {entry.value}
                    </Text>
                  </div>
                ) : (
                  <Text key={index} type="secondary" style={{ fontSize: 12 }}>
                    {entry.value}
                  </Text>
                ),
              )}
              {inlineActions}
            </div>
          )}

          {footer && <div style={{ marginTop: 8 }}>{footer}</div>}
        </div>

        <Space size={0} style={{ whiteSpace: 'nowrap' }}>
          {primary && (primary.locked ? (
            <Tooltip title={primary.tooltip}>
              <span>
                <Button type="link" size="small" icon={primary.icon} disabled>
                  {primary.label}
                </Button>
              </span>
            </Tooltip>
          ) : (
            <Tooltip title={primary.tooltip}>
              <span>
                <Button
                  type="link"
                  size="small"
                  icon={primary.icon}
                  onClick={primary.onClick}
                  disabled={primary.disabled}
                  loading={primary.loading}
                >
                  {primary.label}
                </Button>
              </span>
            </Tooltip>
          ))}
          {actions?.onEdit && (
            <Button type="link" size="small" onClick={actions.onEdit}>
              {t('common.edit', { defaultValue: '编辑' })}
            </Button>
          )}
          {menuItems.length > 0 && (
            <Dropdown
              menu={{
                items: menuItems,
                onClick: ({ key }) => {
                  if (key === 'copy') actions?.onCopy?.();
                  else if (key === 'share') actions?.onShare?.();
                  else if (key === 'delete') actions?.onDelete?.();
                },
              }}
            >
              <Button type="text" size="small" icon={<MoreOutlined />} />
            </Dropdown>
          )}
        </Space>
      </div>
    </CardShell>
  );
};

export default ClaudeStyleCard;
