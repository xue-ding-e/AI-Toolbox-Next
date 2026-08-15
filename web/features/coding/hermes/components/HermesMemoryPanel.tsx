import React from 'react';
import { Button, Collapse, Input, Space, Switch, Typography, message } from 'antd';
import { FileTextOutlined, SaveOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { HermesMemoryKind, HermesMemoryLimits } from '@/types/hermes';
import {
  getHermesMemory,
  getHermesMemoryLimits,
  setHermesMemory,
  setHermesMemoryEnabled,
} from '@/services/hermesApi';
import styles from './HermesMemoryPanel.module.less';

const { Text } = Typography;

interface MemoryPaneProps {
  kind: HermesMemoryKind;
  label: string;
  fileLabel: string;
  limit: number;
  enabled: boolean;
  onToggle: (next: boolean) => Promise<void>;
}

const MemoryPane: React.FC<MemoryPaneProps> = ({
  kind,
  label,
  fileLabel,
  limit,
  enabled,
  onToggle,
}) => {
  const { t } = useTranslation();
  const [content, setContent] = React.useState('');
  const [loaded, setLoaded] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [toggling, setToggling] = React.useState(false);

  React.useEffect(() => {
    let cancelled = false;
    getHermesMemory(kind)
      .then((value) => {
        if (!cancelled) {
          setContent(value);
          setLoaded(true);
        }
      })
      .catch((error) => {
        console.error('Failed to load Hermes memory:', error);
        if (!cancelled) {
          message.error(t('common.error'));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [kind, t]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await setHermesMemory(kind, content);
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to save Hermes memory:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  const handleToggle = async (next: boolean) => {
    setToggling(true);
    try {
      await onToggle(next);
    } finally {
      setToggling(false);
    }
  };

  const charCount = content.length;
  const isOver = charCount > limit;

  return (
    <div className={styles.pane}>
      <div className={styles.paneHeader}>
        <Text strong>{label}</Text>
        <Space size="middle">
          <Text type="secondary" className={styles.usage}>
            {t('hermes.memory.usage', { current: charCount, limit })}
            {isOver && (
              <Text type="danger"> — {t('hermes.memory.overLimit', { defaultValue: 'over budget' })}</Text>
            )}
          </Text>
          <Switch
            size="small"
            checked={enabled}
            disabled={toggling}
            onChange={(next) => void handleToggle(next)}
          />
        </Space>
      </div>
      <Input.TextArea
        className={styles.editor}
        value={content}
        onChange={(event) => setContent(event.target.value)}
        autoSize={{ minRows: 6, maxRows: 18 }}
        disabled={!loaded}
        placeholder={t('hermes.memory.editorPlaceholder', {
          defaultValue: 'Hermes truncates over-budget content at load time.',
        })}
      />
      <div className={styles.paneFooter}>
        <Text type="secondary" className={styles.fileLabel}>
          {fileLabel}
        </Text>
        <Button
          size="small"
          type="primary"
          loading={saving}
          disabled={!loaded}
          onClick={() => void handleSave()}
        >
          <SaveOutlined /> {t('common.save')}
        </Button>
      </div>
    </div>
  );
};

/**
 * Hermes memory editor: agent `MEMORY.md` + user `USER.md` blobs under
 * `<config>/memories/`, plus the enable toggles from the `memory:` section of
 * config.yaml. Hermes' own UI only exposes on/off + budgets, so the content
 * editing lives here.
 */
const HermesMemoryPanel: React.FC = () => {
  const { t } = useTranslation();
  const [limits, setLimits] = React.useState<HermesMemoryLimits | null>(null);

  React.useEffect(() => {
    getHermesMemoryLimits()
      .then(setLimits)
      .catch((error) => {
        console.error('Failed to load Hermes memory limits:', error);
        message.error(t('common.error'));
      });
  }, [t]);

  const handleToggle = async (kind: HermesMemoryKind, next: boolean) => {
    try {
      const nextLimits = await setHermesMemoryEnabled(kind, next);
      setLimits(nextLimits);
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to toggle Hermes memory:', error);
      message.error(t('common.error'));
    }
  };

  return (
    <Collapse
      style={{ marginBottom: 0 }}
      items={[
        {
          key: 'hermes-memory',
          label: (
            <Space>
              <Text strong>
                <FileTextOutlined style={{ marginRight: 8 }} />
                {t('hermes.memory.title', { defaultValue: 'Memory' })}
              </Text>
            </Space>
          ),
          children: (
            <div className={styles.panel}>
              <MemoryPane
                kind="memory"
                label={t('hermes.memory.agentTab', { defaultValue: 'Agent memory (MEMORY.md)' })}
                fileLabel="memories/MEMORY.md"
                limit={limits?.memory ?? 2200}
                enabled={limits?.memoryEnabled ?? true}
                onToggle={(next) => handleToggle('memory', next)}
              />
              <MemoryPane
                kind="user"
                label={t('hermes.memory.userTab', { defaultValue: 'User profile (USER.md)' })}
                fileLabel="memories/USER.md"
                limit={limits?.user ?? 1375}
                enabled={limits?.userEnabled ?? true}
                onToggle={(next) => handleToggle('user', next)}
              />
            </div>
          ),
        },
      ]}
    />
  );
};

export default HermesMemoryPanel;
