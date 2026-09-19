import type { FC } from 'react';
import { Empty, Modal, Tabs, Typography } from 'antd';
import type { TabsProps } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import TomlEditor from '@/components/common/TomlEditor';
import type { CodexSettings } from '@/types/codex';

const { Text } = Typography;

export interface CodexConfigPreviewModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  data: CodexSettings | null;
}

const CodexConfigPreviewModal: FC<CodexConfigPreviewModalProps> = ({
  open,
  onClose,
  title,
  data,
}) => {
  const { t } = useTranslation();

  const authValue = data?.auth ?? null;
  const configValue = data?.config ?? null;
  const modelCatalogValue = data?.modelCatalog ?? null;
  const modelCatalogActive = data?.modelCatalogActive ?? null;
  const showModelCatalogTab = modelCatalogValue !== null || modelCatalogActive === true;
  const editorHeight = 'calc(75vh - 190px)';
  const editorHeightWithHint = 'calc(75vh - 212px)';

  const items: TabsProps['items'] = [];

  if (configValue !== null) {
    items.push({
      key: 'config',
      label: t('codex.preview.configTomlTitle'),
      children: (
        <div style={{ padding: '4px 0' }}>
          <TomlEditor
            value={configValue ?? ''}
            readOnly
            height={editorHeight}
            resizable={false}
          />
        </div>
      ),
    });
  }

  if (authValue) {
    items.push({
      key: 'auth',
      label: t('codex.preview.authJsonTitle'),
      children: (
        <div style={{ padding: '4px 0' }}>
          <JsonEditor
            value={authValue}
            readOnly
            mode="text"
            height={editorHeight}
            resizable={false}
            showMainMenuBar={false}
            showStatusBar={false}
          />
        </div>
      ),
    });
  }

  if (showModelCatalogTab) {
    items.push({
      key: 'modelCatalog',
      label: t('codex.preview.modelCatalogJsonTitle'),
      children: (
        <div style={{ padding: '4px 0' }}>
          {modelCatalogValue !== null ? (
            <>
              {modelCatalogActive === false && (
                <Typography.Text
                  type="secondary"
                  style={{ fontSize: 10, display: 'block', marginBottom: 4 }}
                >
                  {t('codex.preview.modelCatalogInactiveHint')}
                </Typography.Text>
              )}
              <JsonEditor
                value={modelCatalogValue}
                readOnly
                mode="text"
                height={modelCatalogActive === false ? editorHeightWithHint : editorHeight}
                resizable={false}
                showMainMenuBar={false}
                showStatusBar={false}
              />
            </>
          ) : (
            <Empty
              description={t('codex.preview.modelCatalogMissing')}
              style={{ marginTop: 48 }}
            />
          )}
        </div>
      ),
    });
  }

  const hasAny = items.length > 0;

  return (
    <Modal
      title={
        <span>
          {title || t('common.previewConfig')}{' '}
          <Text type="secondary" style={{ fontSize: 12, fontWeight: 'normal' }}>
            ({t('common.readOnly')})
          </Text>
        </span>
      }
      open={open}
      onCancel={onClose}
      footer={null}
      width={1000}
    >
      {!hasAny ? (
        <Empty description={t('common.noData')} />
      ) : (
        <Tabs
          items={items}
          defaultActiveKey={items[0]?.key}
          destroyOnHidden
        />
      )}
    </Modal>
  );
};

export default CodexConfigPreviewModal;
