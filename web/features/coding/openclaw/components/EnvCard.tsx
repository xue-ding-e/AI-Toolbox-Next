import React from 'react';
import { Button, Input, Space, Typography, message, Empty } from 'antd';
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { setOpenClawEnv } from '@/services/openclawApi';
import type { OpenClawEnvConfig } from '@/types/openclaw';

const { Text } = Typography;

const SENSITIVE_PATTERNS = /key|token|secret|password/i;

interface Props {
  env: OpenClawEnvConfig | null;
  onSaved: () => void;
}

type Entry = { key: string; value: string };

const toEntries = (value: unknown): Entry[] => {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    return Object.entries(value).map(([key, val]) => ({
      key,
      value: String(val ?? ''),
    }));
  }
  return [];
};

const toRecord = (entries: Entry[]): Record<string, string> => {
  const record: Record<string, string> = {};
  for (const entry of entries) {
    if (entry.key.trim()) {
      record[entry.key.trim()] = entry.value;
    }
  }
  return record;
};

/** 一组 `{key, value}` 环境变量编辑行。 */
const EnvGroup: React.FC<{
  title: string;
  entries: Entry[];
  onChange: (entries: Entry[]) => void;
  addLabel: string;
  keyPlaceholder: string;
  valuePlaceholder: string;
  emptyText: string;
}> = ({ title, entries, onChange, addLabel, keyPlaceholder, valuePlaceholder, emptyText }) => {
  const handleAdd = () => {
    onChange([...entries, { key: '', value: '' }]);
  };

  const handleRemove = (index: number) => {
    onChange(entries.filter((_, i) => i !== index));
  };

  const handleChange = (index: number, field: 'key' | 'value', value: string) => {
    const updated = [...entries];
    updated[index] = { ...updated[index], [field]: value };
    onChange(updated);
  };

  return (
    <div>
      <Text strong>{title}</Text>
      <Space direction="vertical" style={{ width: '100%' }} size="small">
        {entries.length === 0 ? (
          <Empty description={emptyText} image={Empty.PRESENTED_IMAGE_SIMPLE} />
        ) : (
          entries.map((entry, i) => (
            <Space.Compact key={i} style={{ width: '100%' }}>
              <Input
                value={entry.key}
                onChange={(e) => handleChange(i, 'key', e.target.value)}
                placeholder={keyPlaceholder}
                style={{ flex: 1 }}
              />
              {SENSITIVE_PATTERNS.test(entry.key) ? (
                <Input.Password
                  value={entry.value}
                  onChange={(e) => handleChange(i, 'value', e.target.value)}
                  placeholder={valuePlaceholder}
                  style={{ flex: 2 }}
                />
              ) : (
                <Input
                  value={entry.value}
                  onChange={(e) => handleChange(i, 'value', e.target.value)}
                  placeholder={valuePlaceholder}
                  style={{ flex: 2 }}
                />
              )}
              <Button
                type="text"
                size="middle"
                danger
                icon={<DeleteOutlined />}
                onClick={() => handleRemove(i)}
              />
            </Space.Compact>
          ))
        )}
        <Button type="dashed" size="small" icon={<PlusOutlined />} onClick={handleAdd}>
          {addLabel}
        </Button>
      </Space>
    </div>
  );
};

const EnvCard: React.FC<Props> = ({ env, onSaved }) => {
  const { t } = useTranslation();
  const [saving, setSaving] = React.useState(false);
  const [varsEntries, setVarsEntries] = React.useState<Entry[]>([]);
  const [shellEnvEntries, setShellEnvEntries] = React.useState<Entry[]>([]);

  React.useEffect(() => {
    setVarsEntries(toEntries(env?.vars));
    setShellEnvEntries(toEntries(env?.shellEnv));
  }, [env]);

  const handleSave = async () => {
    try {
      setSaving(true);
      const envObj: OpenClawEnvConfig = { ...env };
      envObj.vars = toRecord(varsEntries);
      envObj.shellEnv = toRecord(shellEnvEntries);
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
      <EnvGroup
        title={t('openclaw.env.varsTitle')}
        entries={varsEntries}
        onChange={setVarsEntries}
        addLabel={t('openclaw.env.addVariable')}
        keyPlaceholder={t('openclaw.env.keyPlaceholder')}
        valuePlaceholder={t('openclaw.env.valuePlaceholder')}
        emptyText={t('openclaw.env.emptyText')}
      />
      <EnvGroup
        title={t('openclaw.env.shellEnvTitle')}
        entries={shellEnvEntries}
        onChange={setShellEnvEntries}
        addLabel={t('openclaw.env.addVariable')}
        keyPlaceholder={t('openclaw.env.keyPlaceholder')}
        valuePlaceholder={t('openclaw.env.valuePlaceholder')}
        emptyText={t('openclaw.env.emptyText')}
      />
      <div style={{ textAlign: 'right' }}>
        <Button type="primary" onClick={handleSave} loading={saving}>
          {t('openclaw.env.save')}
        </Button>
      </div>
    </Space>
  );
};

export default EnvCard;