import React from 'react';
import { Card, Space, Button, Dropdown, Tag, Typography, Switch, Tooltip, message } from 'antd';
import {
  ApiOutlined,
  CheckOutlined,
  EditOutlined,
  DeleteOutlined,
  CopyOutlined,
  MoreOutlined,
  CheckCircleOutlined,
  HolderOutlined,
  DownOutlined,
  RightOutlined,
  LinkOutlined,
  SyncOutlined,
  EyeOutlined,
} from '@ant-design/icons';
import type { MenuProps } from 'antd';
import { BarChart2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { CodexOfficialAccount, CodexProvider, CodexSettingsConfig } from '@/types/codex';
import { engageProxyGatewaySingle, restoreProxyGatewayCliDirect, type GatewayCliTakeoverStatus } from '@/services';
import { extractCodexBaseUrl, extractCodexModel, extractCodexReasoningEffort } from '@/utils/codexConfigUtils';
import ProviderConnectivityStatus from '@/features/coding/shared/providerConnectivity/ProviderConnectivityStatus';
import type { ProviderConnectivityStatusItem } from '@/components/common/ProviderCard/types';
import { CODEX_LOCAL_PROVIDER_ID, shouldShowCodexOfficialAccounts } from '../utils/localProvider';

const { Text } = Typography;

interface CodexProviderCardProps {
  provider: CodexProvider;
  isApplied: boolean;
  onEdit: (provider: CodexProvider) => void;
  onDelete: (provider: CodexProvider) => void;
  onCopy: (provider: CodexProvider) => void;
  onTest: (provider: CodexProvider) => void;
  onSelect: (provider: CodexProvider) => void;
  onToggleDisabled: (provider: CodexProvider, isDisabled: boolean) => void;
  officialAccounts?: CodexOfficialAccount[];
  onOfficialAccountLogin?: (provider: CodexProvider) => void;
  onOfficialLocalAccountSave?: (provider: CodexProvider, account: CodexOfficialAccount) => void;
  onOfficialAccountApply?: (provider: CodexProvider, account: CodexOfficialAccount) => void;
  onOfficialAccountDelete?: (provider: CodexProvider, account: CodexOfficialAccount) => void;
  onOfficialAccountRefresh?: (provider: CodexProvider, account: CodexOfficialAccount) => void;
  onOfficialAccountViewDetails?: (provider: CodexProvider, account: CodexOfficialAccount) => void;
  refreshingOfficialAccountId?: string | null;
  savingOfficialAccountId?: string | null;
  connectivityStatus?: ProviderConnectivityStatusItem;
  gatewayTakeoverActive?: boolean;
  gatewayStatus?: GatewayCliTakeoverStatus | null;
  onGatewayStatusChange?: (status: GatewayCliTakeoverStatus) => void;
}

const CodexProviderCard: React.FC<CodexProviderCardProps> = ({
  provider,
  isApplied,
  onEdit,
  onDelete,
  onCopy,
  onTest,
  onSelect,
  onToggleDisabled,
  officialAccounts = [],
  onOfficialAccountLogin,
  onOfficialLocalAccountSave,
  onOfficialAccountApply,
  onOfficialAccountDelete,
  onOfficialAccountRefresh,
  onOfficialAccountViewDetails,
  refreshingOfficialAccountId,
  savingOfficialAccountId,
  connectivityStatus,
  gatewayTakeoverActive = false,
  gatewayStatus = null,
  onGatewayStatusChange,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [accountsCollapsed, setAccountsCollapsed] = React.useState(true);
  const [engagingGatewayProxy, setEngagingGatewayProxy] = React.useState(false);
  const [restoringDirect, setRestoringDirect] = React.useState(false);

  // 拖拽排序
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: provider.id });

  const sortableStyle = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : (provider.isDisabled ? 0.6 : 1),
  };

  const handleToggleDisabled = (checked: boolean) => {
    if (isApplied && !checked) {
      message.warning(t('common.disableAppliedConfigWarning'));
      return;
    }
    onToggleDisabled(provider, !checked);
  };

  // Parse settingsConfig JSON string
  const settingsConfig: CodexSettingsConfig = React.useMemo(() => {
    try {
      return JSON.parse(provider.settingsConfig);
    } catch (error) {
      console.error('Failed to parse settingsConfig:', error);
      return {};
    }
  }, [provider.settingsConfig]);

  // Extract display info from config
  const apiKey = settingsConfig.auth?.OPENAI_API_KEY;
  const maskedApiKey = apiKey ? `${apiKey.slice(0, 8)}...${apiKey.slice(-4)}` : null;

  // Extract base_url and model from config.toml using utility function
  const baseUrl = React.useMemo(() => {
    const configContent = settingsConfig.config || '';
    return extractCodexBaseUrl(configContent);
  }, [settingsConfig.config]);

  const modelName = React.useMemo(() => {
    const configContent = settingsConfig.config || '';
    return extractCodexModel(configContent);
  }, [settingsConfig.config]);
  const reasoningEffort = React.useMemo(() => {
    const configContent = settingsConfig.config || '';
    return extractCodexReasoningEffort(configContent);
  }, [settingsConfig.config]);
  const isOfficialProvider = provider.category === 'official';
  const gatewayMode = gatewayStatus?.mode ?? null;
  const gatewayFailoverActive = gatewayMode === 'failover';
  const gatewayProxyActive = gatewayMode === 'single' || gatewayFailoverActive;
  const priorityEntry = gatewayFailoverActive
    ? gatewayStatus?.provider_priorities.find((entry) => entry.provider_id === provider.id)
    : undefined;
  const isGatewayPrimary = priorityEntry?.label === 'P0';
  const hasOfficialAccounts = isOfficialProvider && officialAccounts.length > 0;
  const displayModelName = modelName && reasoningEffort
    ? `${modelName} (${reasoningEffort})`
    : modelName;
  const requiresExplicitBaseUrl = !isOfficialProvider;
  const canRunConnectivityTest =
    !isOfficialProvider &&
    Boolean(apiKey?.trim()) &&
    Boolean(modelName?.trim()) &&
    (!requiresExplicitBaseUrl || Boolean(baseUrl?.trim()));
  const showRuntimeApplied = isApplied;
  const showProxyTag = isApplied && gatewayProxyActive;
  const showOfficialRuntimeState = !gatewayProxyActive && !gatewayTakeoverActive;
  const canShowGatewayProxyButton =
    isApplied &&
    !gatewayMode &&
    Boolean(gatewayStatus?.can_takeover) &&
    !provider.isDisabled &&
    !isOfficialProvider &&
    provider.id !== CODEX_LOCAL_PROVIDER_ID;
  const canShowRestoreDirectButton =
    isApplied && gatewayProxyActive && Boolean(gatewayStatus?.can_restore_direct);
  const showApplyAction = !gatewayFailoverActive && !isApplied;
  const showFailoverDisabledApply = gatewayFailoverActive && !isApplied;
  const actionAreaWidth =
    showRuntimeApplied || gatewayProxyActive
      ? canShowGatewayProxyButton || showFailoverDisabledApply || canShowRestoreDirectButton
        ? 140
        : 40
      : 112;
  const cardBorderColor = isGatewayPrimary
    ? 'var(--color-status-success)'
    : isApplied
      ? 'var(--ant-color-primary)'
      : 'var(--color-border-card)';
  const cardBackground = isGatewayPrimary
    ? 'linear-gradient(135deg, color-mix(in srgb, var(--color-status-success) 12%, var(--color-bg-container)), var(--color-bg-container))'
    : isApplied
      ? 'var(--color-bg-selected)'
      : undefined;
  const shouldShowOfficialAccounts = shouldShowCodexOfficialAccounts(
    provider,
    officialAccounts.length,
  );

  const formatOfficialAccountLabel = (account: CodexOfficialAccount) => {
    if (account.id === CODEX_LOCAL_PROVIDER_ID) {
      return account.email || t('codex.provider.officialAccountLocal');
    }
    return account.email || account.name;
  };

  const handleEngageGatewayProxy = async (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    setEngagingGatewayProxy(true);
    try {
      const nextStatus = await engageProxyGatewaySingle('codex', provider.id);
      onGatewayStatusChange?.(nextStatus);
      message.success(t('gateway.proxy.notice.enabled'));
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(t('gateway.proxy.notice.enableFailed', { error: errorMessage }));
    } finally {
      setEngagingGatewayProxy(false);
    }
  };

  const handleRestoreDirect = async (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    setRestoringDirect(true);
    try {
      const nextStatus = await restoreProxyGatewayCliDirect('codex');
      onGatewayStatusChange?.(nextStatus);
      message.success(t('gateway.proxy.notice.restored'));
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(t('gateway.proxy.notice.restoreFailed', { error: errorMessage }));
    } finally {
      setRestoringDirect(false);
    }
  };

  const renderOfficialAccounts = () => {
    if (!shouldShowOfficialAccounts) {
      return null;
    }

    return (
      <div
        style={{
          marginTop: 12,
          paddingTop: 12,
          borderTop: '1px solid var(--color-border)',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 12,
            marginBottom: accountsCollapsed ? 0 : 10,
          }}
        >
          <Button
            type="text"
            size="small"
            onClick={() => setAccountsCollapsed((current) => !current)}
            style={{
              padding: 0,
              height: 'auto',
              color: 'var(--color-text-secondary)',
              fontSize: 12,
            }}
          >
            <Space size={6}>
              {accountsCollapsed ? <RightOutlined /> : <DownOutlined />}
              <Text type="secondary" style={{ fontSize: 12 }}>
                {t('codex.provider.officialAccountsTitle')}
              </Text>
              <Text type="secondary" style={{ fontSize: 11 }}>
                ({officialAccounts.length})
              </Text>
            </Space>
          </Button>

          {isOfficialProvider ? (
            <Button
              type="link"
              size="small"
              icon={<LinkOutlined />}
              onClick={() => onOfficialAccountLogin?.(provider)}
              style={{ paddingInline: 0, height: 'auto', fontSize: 12 }}
            >
              {t('codex.provider.officialAccountLogin')}
            </Button>
          ) : (
            <Text type="secondary" style={{ fontSize: 11 }}>
              {t('codex.provider.officialAccountLegacyNotice')}
            </Text>
          )}
        </div>

        {!accountsCollapsed && (
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: 8,
              paddingLeft: 18,
            }}
          >
            {hasOfficialAccounts ? officialAccounts.map((account) => (
              <div
                key={account.id}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: 12,
                  padding: '6px 0',
                  borderBottom: '1px solid var(--color-border)',
                }}
              >
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    flexWrap: 'wrap',
                    gap: 8,
                    minWidth: 0,
                    flex: 1,
                  }}
                >
                  <Text
                    strong={showOfficialRuntimeState && account.isApplied}
                    style={{ fontSize: 12 }}
                    ellipsis={{ tooltip: formatOfficialAccountLabel(account) }}
                  >
                    {formatOfficialAccountLabel(account)}
                  </Text>
                  <Tag style={{ margin: 0, fontSize: 10 }}>
                    {account.id === CODEX_LOCAL_PROVIDER_ID
                      ? t('codex.provider.officialAccountLocalTag')
                      : t('codex.provider.officialAccountOauthTag')}
                  </Tag>
                  {account.planType && (
                    <Text type="secondary" style={{ fontSize: 11 }}>
                      {account.planType}
                    </Text>
                  )}
                  {account.lastError ? (
                    <Text type="danger" style={{ fontSize: 11 }}>
                      {t('codex.provider.officialAccountLastError', { message: account.lastError })}
                    </Text>
                  ) : (
                    <>
                      {account.limit5hText && (
                        <Text type="secondary" style={{ fontSize: 11 }}>
                          {`${t('codex.provider.officialAccountShortWindowLimitLabel', {
                            label: account.limitShortLabel || '5h',
                          })}: ${account.limit5hText}`}
                        </Text>
                      )}
                      {account.limitWeeklyText && (
                        <Text type="secondary" style={{ fontSize: 11 }}>
                          {`${t('codex.provider.officialAccountWeeklyLimitLabel')}: ${account.limitWeeklyText}`}
                        </Text>
                      )}
                    </>
                  )}
                  {showOfficialRuntimeState && account.isApplied && (
                    <Tag color="green" style={{ margin: 0, fontSize: 10 }}>
                      {t('codex.provider.applied')}
                    </Tag>
                  )}
                </div>

                <Space size={4} wrap>
                  <Button
                    type="text"
                    size="small"
                    icon={<SyncOutlined />}
                    onClick={() => onOfficialAccountRefresh?.(provider, account)}
                    loading={refreshingOfficialAccountId === account.id}
                    style={{ height: 'auto', paddingInline: 4, fontSize: 11 }}
                  >
                    {t('codex.provider.officialAccountRefresh')}
                  </Button>
                  <Button
                    type="text"
                    size="small"
                    icon={<EyeOutlined />}
                    onClick={() => onOfficialAccountViewDetails?.(provider, account)}
                    style={{ height: 'auto', paddingInline: 4, fontSize: 11 }}
                  >
                    {t('codex.provider.officialAccountViewDetails')}
                  </Button>
                  {account.id === CODEX_LOCAL_PROVIDER_ID ? (
                    <Button
                      type="text"
                      size="small"
                      icon={<CheckOutlined />}
                      onClick={() => onOfficialLocalAccountSave?.(provider, account)}
                      loading={savingOfficialAccountId === account.id}
                      style={{ height: 'auto', paddingInline: 4, fontSize: 11 }}
                    >
                      {t('common.save')}
                    </Button>
                  ) : showOfficialRuntimeState && !account.isApplied ? (
                    <Button
                      type="text"
                      size="small"
                      icon={<CheckOutlined />}
                      onClick={() => onOfficialAccountApply?.(provider, account)}
                      style={{ height: 'auto', paddingInline: 4, fontSize: 11 }}
                    >
                      {t('codex.provider.apply')}
                    </Button>
                  ) : null}
                  {!account.isVirtual && (
                    <Button
                      type="text"
                      danger
                      size="small"
                      icon={<DeleteOutlined />}
                      onClick={() => onOfficialAccountDelete?.(provider, account)}
                      style={{ height: 'auto', paddingInline: 4, fontSize: 11 }}
                    >
                      {t('common.delete')}
                    </Button>
                  )}
                </Space>
              </div>
            )) : (
              <Text type="secondary" style={{ fontSize: 12 }}>
                {t('codex.provider.officialAccountsEmpty')}
              </Text>
            )}
          </div>
        )}
      </div>
    );
  };

  const menuItems: MenuProps['items'] = [
    {
      key: 'toggle',
      label: (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <span>{t('common.enable')}</span>
            <Text type="secondary" style={{ fontSize: 11 }}>
              {provider.isDisabled ? t('codex.configDisabled') : t('codex.configEnabled')}
            </Text>
          </div>
          <Switch
            checked={!provider.isDisabled}
            onChange={handleToggleDisabled}
            size="small"
          />
        </div>
      ),
    },
    {
      key: 'edit',
      label: t('common.edit'),
      icon: <EditOutlined />,
      onClick: () => onEdit(provider),
    },
    {
      key: 'copy',
      label: t('common.copy'),
      icon: <CopyOutlined />,
      onClick: () => onCopy(provider),
    },
    ...(provider.id !== CODEX_LOCAL_PROVIDER_ID
      ? [
          {
            type: 'divider' as const,
          },
          {
            key: 'delete',
            label: t('common.delete'),
            icon: <DeleteOutlined />,
            danger: true,
            onClick: () => onDelete(provider),
          },
        ]
      : []),
  ].filter(Boolean) as MenuProps['items'];

  return (
    <div ref={setNodeRef} style={sortableStyle}>
      <Card
        size="small"
        style={{
          marginBottom: 12,
          borderColor: cardBorderColor,
          background: cardBackground,
          boxShadow: 'var(--color-shadow)',
          transition: 'opacity 0.3s ease, border-color 0.2s ease, box-shadow 0.2s ease',
        }}
        styles={{ body: { padding: 16 } }}
        onMouseEnter={(e) => {
          e.currentTarget.style.boxShadow = 'var(--color-shadow-secondary)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.boxShadow = 'var(--color-shadow)';
        }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
          <div style={{ flex: 1, display: 'flex', alignItems: 'flex-start', gap: 8 }}>
            {/* 拖拽手柄 */}
            <div
              {...attributes}
              {...listeners}
              style={{
                cursor: isDragging ? 'grabbing' : 'grab',
                color: 'var(--color-text-tertiary)',
                padding: '4px 0',
                touchAction: 'none',
              }}
            >
              <HolderOutlined />
            </div>
            <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: 4 }}>
              {/* Provider name and status */}
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <ProviderConnectivityStatus item={connectivityStatus} />
                <Text strong style={{ fontSize: 14 }}>
                  {provider.name}
                </Text>
                {provider.id === CODEX_LOCAL_PROVIDER_ID && (
                  <Text type="secondary" style={{ fontSize: 11 }}>
                    ({t('codex.localConfigHint')})
                  </Text>
                )}
                {isOfficialProvider && (
                  <Tag>{t('codex.provider.modeOfficial')}</Tag>
                )}
                {isOfficialProvider && gatewayTakeoverActive && (
                  <Tooltip title={t('gateway.takeover.officialBypassedTooltip')}>
                    <Tag color="gold">{t('gateway.takeover.officialBypassedTag')}</Tag>
                  </Tooltip>
                )}
                {showRuntimeApplied && (
                  <Tag color="green" icon={<CheckCircleOutlined />}>
                    {t('codex.provider.applied')}
                  </Tag>
                )}
                {showProxyTag && (
                  <Tag color="green" icon={<ApiOutlined />}>
                    {t('gateway.proxy.proxyTag')}
                  </Tag>
                )}
                {showProxyTag && (
                  <Tooltip title={t('gateway.proxy.statisticsTooltip')}>
                    <BarChart2
                      size={14}
                      aria-label={t('gateway.proxy.statisticsTooltip')}
                      onClick={(event) => {
                        event.stopPropagation();
                        navigate('/gateway/statistics');
                      }}
                      style={{
                        color: 'var(--color-text-tertiary)',
                        cursor: 'pointer',
                        flexShrink: 0,
                      }}
                    />
                  </Tooltip>
                )}
                {priorityEntry && (
                  <>
                    <span
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 4,
                        padding: '0 6px',
                        height: 20,
                        borderRadius: 10,
                        fontSize: 10,
                        fontWeight: 500,
                        background: 'rgba(16,185,129,0.08)',
                        color: '#059669',
                      }}
                    >
                      <span
                        style={{
                          width: 6,
                          height: 6,
                          borderRadius: '50%',
                          background: '#10b981',
                        }}
                      />
                      {t('gateway.page.modelHealthState.healthy')}
                    </span>
                    <Tooltip
                      title={
                        isGatewayPrimary
                          ? t('gateway.failover.priorityP0')
                          : t('gateway.failover.priorityPn', { label: priorityEntry.label })
                      }
                    >
                      <span
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          padding: '0 6px',
                          height: 20,
                          borderRadius: 4,
                          fontSize: 10,
                          fontWeight: 650,
                          background: 'rgba(16,185,129,0.08)',
                          color: '#059669',
                        }}
                      >
                        {priorityEntry.label}
                      </span>
                    </Tooltip>
                  </>
                )}
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                  {baseUrl && (
                    <Text code style={{ fontSize: 11, padding: '0 4px' }}>
                      {baseUrl}
                    </Text>
                  )}
                  {displayModelName && (
                    <Tag color="blue" style={{ fontSize: 11, margin: 0 }}>
                      {displayModelName}
                    </Tag>
                  )}
                  {(baseUrl || modelName) && maskedApiKey && (
                    <Text type="secondary" style={{ fontSize: 12 }}>|</Text>
                  )}
                  {maskedApiKey && (
                    <Text type="secondary" style={{ fontSize: 12 }}>
                      API Key: {maskedApiKey}
                    </Text>
                  )}
                  {(baseUrl || modelName || maskedApiKey) && provider.notes && (
                    <Text type="secondary" style={{ fontSize: 12 }}>|</Text>
                  )}
                  {provider.notes && (
                    <Text type="secondary" style={{ fontSize: 12 }}>
                      {provider.notes}
                    </Text>
                  )}
                <Text type="secondary" style={{ fontSize: 11 }}>|</Text>
                <Button
                  type="text"
                  size="small"
                  icon={<ApiOutlined />}
                  onClick={() => onTest(provider)}
                  disabled={!canRunConnectivityTest}
                  title={isOfficialProvider ? t('codex.provider.officialConnectivityHint') : undefined}
                  style={{ fontSize: 11, padding: '0 4px', height: 'auto', flexShrink: 0 }}
                >
                  {t('opencode.connectivity.button')}
                </Button>
              </div>
            </div>
          </div>

          {/* Action buttons */}
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'flex-end',
              gap: 8,
              width: actionAreaWidth,
              whiteSpace: 'nowrap',
            }}
          >
            {canShowGatewayProxyButton && (
              <Tooltip title={t('gateway.proxy.singleHint')}>
                <Button
                  type="link"
                  size="small"
                  icon={<ApiOutlined />}
                  onClick={handleEngageGatewayProxy}
                  loading={engagingGatewayProxy}
                >
                  {t('gateway.proxy.singleButton')}
                </Button>
              </Tooltip>
            )}
            {canShowRestoreDirectButton && (
              <Tooltip title={t('gateway.proxy.restoreDirectHint')}>
                <Button
                  type="link"
                  size="small"
                  onClick={handleRestoreDirect}
                  loading={restoringDirect}
                >
                  {t('gateway.proxy.restoreDirectButton')}
                </Button>
              </Tooltip>
            )}
            {showApplyAction && (
              <Button
                type="link"
                size="small"
                icon={<CheckOutlined />}
                onClick={() => onSelect(provider)}
                disabled={provider.isDisabled}
              >
                {t('codex.provider.apply')}
              </Button>
            )}
            {showFailoverDisabledApply && (
              <Tooltip title={t('gateway.failover.applyDisabledTooltip')}>
                <span>
                  <Button type="link" size="small" icon={<CheckOutlined />} disabled>
                    {t('codex.provider.apply')}
                  </Button>
                </span>
              </Tooltip>
            )}
            <Dropdown menu={{ items: menuItems }} trigger={['click']}>
              <Button type="text" size="small" icon={<MoreOutlined />} />
            </Dropdown>
          </div>
      </div>
        {renderOfficialAccounts()}
    </Card>
    </div>
  );
};

export default CodexProviderCard;
