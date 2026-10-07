import React from 'react';
import { Button, Collapse, Empty, Space, Spin, Tooltip, Typography } from 'antd';
import { FileOutlined, SafetyCertificateOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import OfficialProviderCard from '@/components/common/OfficialProviderCard';
import AuthConfigModal from '@/components/common/AuthConfigModal';
import { saveOmoNativeAuthConfig } from '@/services/omoNativeApi';
import type { OmoNativeBuiltinProvider } from '@/types/omoNative';

const { Text } = Typography;

interface OmoNativeBuiltinProvidersSectionProps {
  /**
   * 已配置凭据的内建 provider（`null` = 还在查）。
   *
   * 列表由**页面**持有：同一个结果「模型设置」区块的下拉也要用，两处各查一次
   * 会白跑两遍 `omo auth check`（48 个 provider 实测约 8 秒）。
   */
  providers: OmoNativeBuiltinProvider[] | null;
  /** 重查内建渠道（凭据改动后调用）。 */
  reloadProviders: () => Promise<void>;
  /** `<agentDir>/auth.json` 原文；「auth.json」按钮用它打开编辑弹窗。 */
  authContent: string | undefined;
  /** 保存后重读配置（与页面其它区块共用）。 */
  onSaved: () => Promise<void> | void;
}

/**
 * 「引擎内建渠道」区块：列出**已配置凭据**的 `omo` 内建 provider 及其内建模型。
 *
 * 形态对齐 OpenCode 的「官方Auth认证渠道」区块（同一个 `OfficialProviderCard`
 * 组件，`i18nPrefix="omoNative"`），筛选语义也一致：**只有配好凭据的渠道才出现**
 * ——未配置的渠道在本模块里没有任何可做的事，列出来只是噪音。
 *
 * ⚠️ **凭据检查必须在后端做，而且是页面加载时做一次**：引擎只能逐个 provider 查
 * （不带 `--provider` 直接报错），48 个内建 provider 实测 12 路并发约 8 秒。
 * 前端拿到的是已经筛好的列表，不再自己补查——否则要么串行跑 30 秒以上，
 * 要么先闪出一屏未配置的渠道再逐个消失。
 */
const OmoNativeBuiltinProvidersSection: React.FC<OmoNativeBuiltinProvidersSectionProps> = ({
  providers,
  reloadProviders,
  authContent,
  onSaved,
}) => {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = React.useState(false);
  const [authModalOpen, setAuthModalOpen] = React.useState(false);

  return (
    <div
      id="omo-native-builtin-channels"
      data-sidebar-section="true"
      data-sidebar-title={t('omoNative.official.title')}
    >
      <Collapse
        style={{ marginBottom: 16 }}
        activeKey={collapsed ? [] : ['builtin']}
        onChange={(keys) => setCollapsed(!keys.includes('builtin'))}
        items={[
          {
            key: 'builtin',
            label: (
              <Space size={8}>
                <Text strong>
                  <SafetyCertificateOutlined style={{ marginRight: 8 }} />
                  {t('omoNative.official.title')}
                </Text>
                {providers && providers.length > 0 && (
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {providers.length}
                  </Text>
                )}
                {/* 入口位置对齐 OpenCode 的「官方Auth认证渠道」，但**在应用内**
                    打开可编辑的弹窗——本应用是配置管理器，密钥要能看能改，
                    不要只给一个「用资源管理器打开」的跳转。 */}
                <Tooltip title={t('common.authConfig.openHint')}>
                  <Button
                    type="link"
                    size="small"
                    icon={<FileOutlined />}
                    onClick={(event) => {
                      event.stopPropagation();
                      setAuthModalOpen(true);
                    }}
                    style={{ padding: 0, height: 'auto', fontSize: 12 }}
                  >
                    auth.json
                  </Button>
                </Tooltip>
              </Space>
            ),
            children: (
              <div>
                <div style={{ marginBottom: 12 }}>
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {t('omoNative.official.description')}
                  </Text>
                </div>
                {providers === null ? (
                  <Spin spinning>
                    <div style={{ minHeight: 80 }} />
                  </Spin>
                ) : providers.length === 0 ? (
                  <Empty description={t('omoNative.official.noConfigured')} style={{ marginTop: 40 }} />
                ) : (
                  providers.map((provider) => (
                    <OfficialProviderCard
                      key={provider.id}
                      id={provider.id}
                      name={provider.name}
                      i18nPrefix="omoNative"
                      // 引擎的 `omo --list-models` 表格只给了 context / max-out /
                      // thinking / images；卡片要的是 id/name/context/output。
                      models={provider.models.map((model) => ({
                        id: model.id,
                        name: model.id,
                        context: model.context,
                        output: model.output,
                        isFree: false,
                      }))}
                    />
                  ))
                )}
              </div>
            ),
          },
        ]}
      />

      <AuthConfigModal
        open={authModalOpen}
        content={authContent}
        path="<agentDir>/auth.json"
        onSave={saveOmoNativeAuthConfig}
        onCancel={() => setAuthModalOpen(false)}
        onSaved={async () => {
          await onSaved();
          // 改完凭据要重查就绪状态——列表本来就是按凭据筛的。
          await reloadProviders();
        }}
      />
    </div>
  );
};

export default OmoNativeBuiltinProvidersSection;
