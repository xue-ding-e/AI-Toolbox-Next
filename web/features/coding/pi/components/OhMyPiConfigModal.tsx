import React from 'react';
import { Button, Empty, Form, Input, List, Modal, Space, Spin, Typography, message } from 'antd';
import { DeleteOutlined, EditOutlined, PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';

import JsonEditor from '@/components/common/JsonEditor';
import {
  deleteOmpProvider,
  readOmpRuntimeConfig,
  saveOmpProvider,
  saveOmpSettingsConfig,
} from '@/services/ohMyPiApi';
import type { OmpRuntimeConfig } from '@/types/ohMyPi';

import styles from './OhMyPiConfigModal.module.less';

interface Props {
  open: boolean;
  onClose: () => void;
}

interface ProviderEditorState {
  originalKey?: string;
  providerKey: string;
  provider: Record<string, unknown>;
}

const asRecord = (value: unknown): Record<string, unknown> => (
  value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}
);

const OhMyPiConfigModal: React.FC<Props> = ({ open, onClose }) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [runtime, setRuntime] = React.useState<OmpRuntimeConfig | null>(null);
  const [rootDir, setRootDir] = React.useState('');
  const [editor, setEditor] = React.useState<ProviderEditorState | null>(null);
  const [providerValid, setProviderValid] = React.useState(true);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const next = await readOmpRuntimeConfig();
      setRuntime(next);
      setRootDir(next.rootPathInfo.path);
    } catch (error) {
      console.error('Failed to load Oh My Pi config:', error);
      message.error(t('ohMyPi.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (open) void load();
  }, [load, open]);

  const saveRoot = async () => {
    setSaving(true);
    try {
      await saveOmpSettingsConfig({ rootDir: rootDir.trim() });
      await load();
      message.success(t('common.success'));
    } finally {
      setSaving(false);
    }
  };

  const resetRoot = async () => {
    setSaving(true);
    try {
      await saveOmpSettingsConfig({ clearRootDir: true });
      await load();
      message.success(t('common.success'));
    } finally {
      setSaving(false);
    }
  };

  const submitProvider = async () => {
    if (!editor || !editor.providerKey.trim() || !providerValid) return;
    setSaving(true);
    try {
      let next = await saveOmpProvider({
        providerKey: editor.providerKey.trim(),
        provider: editor.provider,
      });
      if (editor.originalKey && editor.originalKey !== editor.providerKey.trim()) {
        next = await deleteOmpProvider(editor.originalKey);
      }
      setRuntime(next);
      setEditor(null);
      message.success(t('common.success'));
    } finally {
      setSaving(false);
    }
  };

  const removeProvider = (providerKey: string) => {
    Modal.confirm({
      title: t('ohMyPi.deleteTitle'),
      content: providerKey,
      okButtonProps: { danger: true },
      onOk: async () => {
        setRuntime(await deleteOmpProvider(providerKey));
      },
    });
  };

  const providers = Object.entries(runtime?.providers ?? {});

  return (
    <>
      <Modal
        open={open}
        onCancel={onClose}
        footer={null}
        width={860}
        title={t('ohMyPi.title')}
        destroyOnHidden
      >
        <Spin spinning={loading || saving}>
          <div className={styles.rootSection}>
            <div>
              <Typography.Text strong>{t('ohMyPi.rootDir')}</Typography.Text>
              <Input value={rootDir} onChange={(event) => setRootDir(event.target.value)} />
              <div className={styles.pathMeta}>
                {runtime?.modelsPath}<br />{runtime?.mcpPath}
              </div>
            </div>
            <Space>
              <Button onClick={resetRoot}>{t('common.reset')}</Button>
              <Button type="primary" onClick={saveRoot}>{t('common.save')}</Button>
            </Space>
          </div>

          <div className={styles.providerHeader}>
            <Typography.Text strong>{t('ohMyPi.providers')}</Typography.Text>
            <Space>
              <Button icon={<ReloadOutlined />} onClick={() => void load()}>
                {t('common.refresh')}
              </Button>
              <Button
                type="primary"
                icon={<PlusOutlined />}
                onClick={() => setEditor({ providerKey: '', provider: {} })}
              >
                {t('common.add')}
              </Button>
            </Space>
          </div>

          {providers.length === 0 ? <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} /> : (
            <List
              className={styles.providerList}
              dataSource={providers}
              renderItem={([providerKey, provider]) => (
                <List.Item
                  actions={[
                    <Button
                      key="edit"
                      type="text"
                      icon={<EditOutlined />}
                      aria-label={t('common.edit')}
                      onClick={() => setEditor({ originalKey: providerKey, providerKey, provider })}
                    />,
                    <Button
                      key="delete"
                      type="text"
                      danger
                      icon={<DeleteOutlined />}
                      aria-label={t('common.delete')}
                      onClick={() => removeProvider(providerKey)}
                    />,
                  ]}
                >
                  <List.Item.Meta
                    title={<span className={styles.providerKey}>{providerKey}</span>}
                    description={(
                      <span className={styles.providerMeta}>
                        {[provider.api, provider.baseUrl].filter(Boolean).join(' · ') || t('ohMyPi.overrideOnly')}
                      </span>
                    )}
                  />
                </List.Item>
              )}
            />
          )}
        </Spin>
      </Modal>

      <Modal
        open={!!editor}
        onCancel={() => setEditor(null)}
        onOk={() => void submitProvider()}
        confirmLoading={saving}
        okButtonProps={{ disabled: !editor?.providerKey.trim() || !providerValid }}
        title={editor?.originalKey ? t('ohMyPi.editProvider') : t('ohMyPi.addProvider')}
        width={720}
        destroyOnHidden
      >
        <Form layout="vertical">
          <Form.Item label={t('ohMyPi.providerKey')} required>
            <Input
              value={editor?.providerKey}
              onChange={(event) => setEditor((current) => current && ({ ...current, providerKey: event.target.value }))}
            />
          </Form.Item>
          <Form.Item label={t('ohMyPi.providerConfig')} required>
            <JsonEditor
              value={editor?.provider}
              height={360}
              onChange={(value, valid) => {
                setProviderValid(valid);
                if (valid) setEditor((current) => current && ({ ...current, provider: asRecord(value) }));
              }}
            />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
};

export default OhMyPiConfigModal;
