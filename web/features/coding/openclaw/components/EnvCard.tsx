import React from 'react';
import { Button, Input, Space, message, Empty } from 'antd';
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { setOpenClawEnv } from '@/services/openclawApi';
import type { OpenClawEnvConfig } from '@/types/openclaw';
import styles from '../pages/OpenClawPage.module.less';

const SENSITIVE_PATTERNS = /key|token|secret|password/i;

interface Props {
  env: OpenClawEnvConfig | null;
  onSaved: () => void;
}

const EnvCard: React.FC<Props> = ({ env, onSaved }) => {
  const { t } = useTranslation();
  const [saving, setSaving] = React.useState(false);
  const [entries, setEntries] = React.useState<Array<{ key: string; value: string }>>([]);

  React.useEffect(() => {
    if (env) {
      setEntries(
        Object.entries(env).map(([key, value]) => ({
          key,
          value: String(value ?? ''),
        }))
      );
    } else {
      setEntries([]);
    }
  }, [env]);

  const handleAdd = () => {
    setEntries([...entries, { key: '', value: '' }]);
  };

  const handleRemove = (index: number) => {
    setEntries(entries.filter((_, i) => i !== index));
  };

  const handleChange = (index: number, field: 'key' | 'value', value: string) => {
    const updated = [...entries];
    updated[index] = { ...updated[index], [field]: value };
    setEntries(updated);
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      const envObj: OpenClawEnvConfig = {};
      for (const entry of entries) {
        if (entry.key.trim()) {
          envObj[entry.key.trim()] = entry.value;
        }
      }
      await setOpenClawEnv(envObj);
      message.success(t('common.success'));
      onSaved();
    } catch (error) {
      console.error('Failed to save env:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Space direction="vertical" style={{ width: '100%' }} size="middle">
      {entries.length === 0 ? (
        <Empty description={t('openclaw.env.emptyText')} image={Empty.PRESENTED_IMAGE_SIMPLE} />
      ) : (
        entries.map((entry, i) => (
          <div key={i} className={styles.envRow}>
            <Input
              value={entry.key}
              onChange={(e) => handleChange(i, 'key', e.target.value)}
              placeholder={t('openclaw.env.keyPlaceholder')}
              style={{ flex: 1 }}
            />
            {SENSITIVE_PATTERNS.test(entry.key) ? (
              <Input.Password
                value={entry.value}
                onChange={(e) => handleChange(i, 'value', e.target.value)}
                placeholder={t('openclaw.env.valuePlaceholder')}
                style={{ flex: 2 }}
              />
            ) : (
              <Input
                value={entry.value}
                onChange={(e) => handleChange(i, 'value', e.target.value)}
                placeholder={t('openclaw.env.valuePlaceholder')}
                style={{ flex: 2 }}
              />
            )}
            <Button
              type="text"
              size="small"
              danger
              icon={<DeleteOutlined />}
              onClick={() => handleRemove(i)}
            />
          </div>
        ))
      )}

      <Space>
        <Button type="dashed" size="small" icon={<PlusOutlined />} onClick={handleAdd}>
          {t('openclaw.env.addVariable')}
        </Button>
      </Space>

      <div style={{ textAlign: 'right' }}>
        <Button type="primary" onClick={handleSave} loading={saving}>
          {t('openclaw.env.save')}
        </Button>
      </div>
    </Space>
  );
};

export default EnvCard;
