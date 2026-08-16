import type { FC } from 'react';
import { Empty, Modal, Tabs, Typography } from 'antd';
import type { TabsProps } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import PlainTextEditor from '@/components/common/PlainTextEditor';

const { Text } = Typography;

export interface PreviewFile {
  key: string;
  label: string;
  /** JSON-compatible object/array or raw string content. */
  content: unknown;
  /** Monaco language id used when `content` is a string (e.g. `yaml`, `markdown`). */
  language?: string;
}

export interface FileConfigPreviewModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  files: PreviewFile[];
}

const FileConfigPreviewModal: FC<FileConfigPreviewModalProps> = ({
  open,
  onClose,
  title,
  files,
}) => {
  const { t } = useTranslation();
  const editorHeight = 'calc(75vh - 190px)';

  const items: TabsProps['items'] = [];

  for (const file of files) {
    if (file.content === undefined || file.content === null) {
      continue;
    }

    items.push({
      key: file.key,
      label: file.label,
      children: (
        <div style={{ padding: '4px 0' }}>
          {typeof file.content === 'string' ? (
            <PlainTextEditor
              value={file.content}
              readOnly
              language={file.language || 'plaintext'}
              height={editorHeight}
            />
          ) : (
            <JsonEditor
              value={file.content}
              readOnly
              mode="text"
              height={editorHeight}
              resizable={false}
              showMainMenuBar={false}
              showStatusBar={false}
            />
          )}
        </div>
      ),
    });
  }

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
      {items.length === 0 ? (
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

export default FileConfigPreviewModal;