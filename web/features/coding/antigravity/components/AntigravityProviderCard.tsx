import React from 'react';
import { Card, Space, Button, Tag, Typography, Tooltip } from 'antd';
import {
  DeleteOutlined,
  DownOutlined,
  EyeOutlined,
  RightOutlined,
  SyncOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { AntigravityOfficialAccount, AntigravityProvider } from '@/types/antigravity';
import AppliedTag from '@/components/common/AppliedTag';
import {
  shouldShowAntigravityOfficialAccounts,
} from '../utils/localProvider';

const { Text } = Typography;

interface AntigravityProviderCardProps {
  provider: AntigravityProvider;
  isApplied: boolean;
  officialAccounts?: AntigravityOfficialAccount[];
  onOfficialAccountLogin?: (provider: AntigravityProvider) => void;
  onOfficialAccountApply?: (provider: AntigravityProvider, account: AntigravityOfficialAccount) => void;
  onOfficialAccountDelete?: (provider: AntigravityProvider, account: AntigravityOfficialAccount) => void;
  onOfficialAccountRefresh?: (provider: AntigravityProvider, account: AntigravityOfficialAccount) => void;
  onOfficialAccountViewDetails?: (provider: AntigravityProvider, account: AntigravityOfficialAccount) => void;
  refreshingOfficialAccountId?: string | null;
  savingOfficialAccountId?: string | null;
  oauthPending?: boolean;
}

const AntigravityProviderCard: React.FC<AntigravityProviderCardProps> = ({
  provider,
  isApplied,
  officialAccounts = [],
  onOfficialAccountLogin,
  onOfficialAccountApply,
  onOfficialAccountDelete,
  onOfficialAccountRefresh,
  onOfficialAccountViewDetails,
  refreshingOfficialAccountId,
  savingOfficialAccountId,
  oauthPending = false,
}) => {
  const { t } = useTranslation();
  const [accountsCollapsed, setAccountsCollapsed] = React.useState(true);
  const isOfficial = provider.category === 'official';
  const showOfficialSection = shouldShowAntigravityOfficialAccounts(provider, officialAccounts.length);

  if (!isOfficial) return null;

  return (
    <div style={{ marginBottom: 12 }}>
      <Card
        size="small"
        style={{
          opacity: 1,
          borderRadius: 8,
          background: 'var(--color-bg-container, #ffffff)',
        }}
        styles={{ body: { padding: '12px 16px' } }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flex: 1, minWidth: 0 }}>
            <Text strong style={{ fontSize: 14 }}>
              {t('antigravity.provider.displayName', { defaultValue: 'Antigravity CLI 官方账号' })}
            </Text>

            {isApplied && (
              <AppliedTag>
                {t('antigravity.provider.applied')}
              </AppliedTag>
            )}

            <Tag color="geekblue">{t('antigravity.provider.official', { defaultValue: 'Antigravity CLI 官方账号' })}</Tag>
          </div>

          <Space size={8} />
        </div>

        {/* Metadata info */}
        <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: '8px 16px', fontSize: 12 }}>
          {provider.notes && (
            <div style={{ display: 'flex', gap: 4, alignItems: 'center', width: '100%' }}>
              <Text type="secondary">{t('antigravity.provider.notes')}:</Text>
              <Text type="secondary" style={{ fontStyle: 'italic' }}>{provider.notes}</Text>
            </div>
          )}
        </div>

        {/* Official accounts section */}
        {showOfficialSection && (
          <div
            style={{
              marginTop: 10,
              paddingTop: 8,
              borderTop: '1px dashed var(--color-border-secondary, #f0f0f0)',
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                cursor: 'pointer',
              }}
              onClick={() => setAccountsCollapsed(!accountsCollapsed)}
            >
              <Space size={4}>
                {accountsCollapsed ? <RightOutlined style={{ fontSize: 10 }} /> : <DownOutlined style={{ fontSize: 10 }} />}
                <Text style={{ fontSize: 12, fontWeight: 500 }}>
                  {t('antigravity.officialAccounts.title', { defaultValue: 'Antigravity CLI 账号' })} ({officialAccounts.length})
                </Text>
              </Space>

              <Space size={6} onClick={(e) => e.stopPropagation()}>
                {isOfficial && onOfficialAccountLogin && (
                  <Button
                    size="small"
                    type="link"
                    style={{ fontSize: 12, padding: 0 }}
                    onClick={() => onOfficialAccountLogin(provider)}
                    loading={oauthPending}
                    disabled={oauthPending || provider.isDisabled}
                  >
                    + {t('antigravity.officialAccounts.loginOauth', { defaultValue: 'Antigravity CLI 登录' })}
                  </Button>
                )}
              </Space>
            </div>

            {!accountsCollapsed && (
              <div style={{ marginTop: 8, display: 'flex', flexDirection: 'column', gap: 6 }}>
                {officialAccounts.length === 0 ? (
                  <Text type="secondary" style={{ fontSize: 12, textAlign: 'center', padding: '8px 0' }}>
                    {t('antigravity.officialAccounts.empty', { defaultValue: '暂无已保存账号，可直接使用默认配置或进行 OAuth 登录。' })}
                  </Text>
                ) : (
                  officialAccounts.map((account) => (
                    <div
                      key={account.id}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        padding: '6px 10px',
                        background: account.isApplied
                          ? 'var(--ant-color-primary-bg, #e6f4ff)'
                          : 'var(--color-bg-elevated, #fafafa)',
                        border: account.isApplied
                          ? '1px solid var(--ant-color-primary-border, #91caff)'
                          : '1px solid var(--color-border, #f0f0f0)',
                        borderRadius: 6,
                        fontSize: 12,
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0, flex: 1 }}>
                        <Text strong style={{ maxWidth: 160, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {account.email
                            || (account.isVirtual
                              ? t('antigravity.provider.officialAccountLocal')
                              : account.name)}
                        </Text>
                        {account.planType && (
                          <Tag style={{ margin: 0, fontSize: 10 }}>{account.planType}</Tag>
                        )}
                        {account.limitShortLabel && (
                          <Tag color="blue" style={{ margin: 0, fontSize: 10 }}>{account.limitShortLabel}</Tag>
                        )}
                        {account.isApplied && (
                          <Tag color="success" style={{ margin: 0, fontSize: 10 }}>
                            {t('antigravity.provider.applied')}
                          </Tag>
                        )}
                      </div>

                      <Space size={4}>
                        {account.isVirtual && onOfficialAccountApply && !account.isApplied && (
                          <Button
                            size="small"
                            type="link"
                            loading={savingOfficialAccountId === account.id}
                            onClick={() => onOfficialAccountApply(provider, account)}
                          >
                            {t('antigravity.officialAccounts.useDefault', { defaultValue: '使用默认配置' })}
                          </Button>
                        )}
                        {!account.isApplied && !account.isVirtual && onOfficialAccountApply && (
                          <Button
                            size="small"
                            type="link"
                            loading={savingOfficialAccountId === account.id}
                            disabled={Boolean(savingOfficialAccountId) && savingOfficialAccountId !== account.id}
                            onClick={() => onOfficialAccountApply(provider, account)}
                          >
                            {t('antigravity.provider.apply')}
                          </Button>
                        )}
                        {onOfficialAccountRefresh && !account.isVirtual && (
                          <Tooltip title={t('antigravity.officialAccounts.refreshQuota', { defaultValue: '刷新配额' })}>
                            <Button
                              size="small"
                              type="text"
                              icon={<SyncOutlined spin={refreshingOfficialAccountId === account.id} />}
                              disabled={Boolean(refreshingOfficialAccountId)}
                              onClick={() => onOfficialAccountRefresh(provider, account)}
                            />
                          </Tooltip>
                        )}
                        {onOfficialAccountViewDetails && !account.isVirtual && (
                          <Tooltip title={t('antigravity.officialAccounts.viewDetails', { defaultValue: '查看凭证详情' })}>
                            <Button
                              size="small"
                              type="text"
                              icon={<EyeOutlined />}
                              onClick={() => onOfficialAccountViewDetails(provider, account)}
                            />
                          </Tooltip>
                        )}
                        {onOfficialAccountDelete && !account.isVirtual && (
                          <Tooltip title={account.isApplied ? t('antigravity.provider.appliedCannotDelete', { defaultValue: '已生效账号不能删除' }) : t('common.delete')}>
                            <Button
                              size="small"
                              type="text"
                              danger
                              disabled={account.isApplied}
                              icon={<DeleteOutlined />}
                              onClick={() => onOfficialAccountDelete(provider, account)}
                            />
                          </Tooltip>
                        )}
                      </Space>
                    </div>
                  ))
                )}
              </div>
            )}
          </div>
        )}
      </Card>
    </div>
  );
};

export default AntigravityProviderCard;
