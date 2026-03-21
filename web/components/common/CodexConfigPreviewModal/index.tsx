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
  const editorHeight = 'calc(75vh - 190px)';

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
      centered
      styles={{
        body: {
          padding: '16px 24px',
        },
      }}
    >
      {!hasAny ? (
        <Empty description={t('common.noData')} />
      ) : (
        <Tabs
          items={items}
          defaultActiveKey={items[0]?.key}
          destroyInactiveTabPane
        />
      )}
    </Modal>
  );
};

export default CodexConfigPreviewModal;
