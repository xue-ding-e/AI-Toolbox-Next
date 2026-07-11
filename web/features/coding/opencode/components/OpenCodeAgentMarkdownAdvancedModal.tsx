import React from 'react';
import { Alert, Button, Modal } from 'antd';
import type { editor } from 'monaco-editor';
import MonacoEditor from 'react-monaco-editor';
import { useTranslation } from 'react-i18next';

import { useThemeStore } from '@/stores/themeStore';

import styles from './OpenCodeAgentAdvancedModal.module.less';

interface OpenCodeAgentMarkdownAdvancedModalProps {
  open: boolean;
  agentName: string;
  initialValue: string;
  editFullFile?: boolean;
  onCancel: () => void;
  onSave: (value: string) => Promise<void>;
}

const OpenCodeAgentMarkdownAdvancedModal: React.FC<OpenCodeAgentMarkdownAdvancedModalProps> = ({
  open,
  agentName,
  initialValue,
  editFullFile = false,
  onCancel,
  onSave,
}) => {
  const { t } = useTranslation();
  const { resolvedTheme } = useThemeStore();
  const [draft, setDraft] = React.useState('');
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string>();

  React.useEffect(() => {
    if (!open) return;
    setDraft(initialValue);
    setError(undefined);
  }, [initialValue, open]);

  const handleSave = async () => {
    setSaving(true);
    setError(undefined);
    try {
      await onSave(draft);
      onCancel();
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={t('opencode.agentSettings.markdownAdvancedTitle', { name: agentName })}
      open={open}
      width={760}
      onCancel={onCancel}
      footer={[
        <Button key="cancel" onClick={onCancel}>{t('common.cancel')}</Button>,
        <Button key="save" type="primary" loading={saving} onClick={() => void handleSave()}>
          {t('common.save')}
        </Button>,
      ]}
    >
      <div className={styles.content}>
        <div className={styles.hint}>
          {t(editFullFile
            ? 'opencode.agentSettings.markdownRepairHint'
            : 'opencode.agentSettings.markdownAdvancedHint')}
        </div>
        {error ? <Alert className={styles.error} type="error" showIcon message={error} /> : null}
        <MonacoEditor
          value={draft}
          language={editFullFile ? 'markdown' : 'yaml'}
          theme={resolvedTheme === 'dark' ? 'vs-dark' : 'vs'}
          height="420px"
          onChange={(value) => setDraft(value)}
          options={{
            automaticLayout: true,
            fontFamily: 'Menlo, Monaco, "Courier New", monospace',
            fontSize: 13,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            tabSize: 2,
            wordWrap: 'on',
          } satisfies editor.IStandaloneEditorConstructionOptions}
        />
      </div>
    </Modal>
  );
};

export default OpenCodeAgentMarkdownAdvancedModal;
