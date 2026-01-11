import React from 'react';
import { Tag, Input, Select, Space, Empty, Typography, Collapse } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';

const { Text } = Typography;

// Common plugins for OpenCode
const COMMON_PLUGINS = [
  { value: 'oh-my-opencode', label: 'oh-my-opencode' },
  { value: 'opencode-antigravity-auth', label: 'opencode-antigravity-auth (Antigravity OAuth)' },
  { value: 'opencode-openai-codex-auth', label: 'opencode-openai-codex-auth (Codex OAuth)' },
];

interface PluginSettingsProps {
  plugins: string[];
  onChange: (plugins: string[]) => void;
  defaultCollapsed?: boolean;
}

const PluginSettings: React.FC<PluginSettingsProps> = ({ plugins, onChange, defaultCollapsed = true }) => {
  const { t } = useTranslation();
  const [inputValue, setInputValue] = React.useState('');
  const [inputVisible, setInputVisible] = React.useState(false);
  const inputRef = React.useRef<HTMLInputElement>(null);
  const [collapsed, setCollapsed] = React.useState(defaultCollapsed);

  React.useEffect(() => {
    if (inputVisible) {
      inputRef.current?.focus();
    }
  }, [inputVisible]);

  const handleClose = (removedPlugin: string) => {
    const newPlugins = plugins.filter((plugin) => plugin !== removedPlugin);
    onChange(newPlugins);
  };

  const handleInputConfirm = () => {
    if (inputValue && !plugins.includes(inputValue)) {
      onChange([...plugins, inputValue]);
    }
    setInputVisible(false);
    setInputValue('');
  };

  const handleSelectPlugin = (value: string) => {
    if (value && !plugins.includes(value)) {
      onChange([...plugins, value]);
    }
  };

  // Filter out already added plugins from common plugins
  const availableCommonPlugins = COMMON_PLUGINS.filter(
    (plugin) => !plugins.includes(plugin.value)
  );

  const content = (
    <Space orientation="vertical" style={{ width: '100%' }} size={12}>
      {/* Plugin tags */}
      <div>
        {plugins.length === 0 && !inputVisible && (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={t('opencode.plugin.emptyText')}
            style={{ margin: '8px 0' }}
          />
        )}
        <Space wrap>
          {plugins.map((plugin) => (
            <Tag
              key={plugin}
              closable
              onClose={() => handleClose(plugin)}
              style={{ marginBottom: 4 }}
            >
              {plugin}
            </Tag>
          ))}
          {inputVisible ? (
            <Input
              ref={inputRef as React.RefObject<any>}
              type="text"
              size="small"
              style={{ width: 250 }}
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onBlur={handleInputConfirm}
              onPressEnter={handleInputConfirm}
              placeholder={t('opencode.plugin.inputPlaceholder')}
            />
          ) : (
            <Tag
              onClick={() => setInputVisible(true)}
              style={{ cursor: 'pointer', borderStyle: 'dashed' }}
            >
              <PlusOutlined /> {t('opencode.plugin.addPlugin')}
            </Tag>
          )}
        </Space>
      </div>

      {/* Common plugins selector */}
      {availableCommonPlugins.length > 0 && (
        <div>
          <Text type="secondary" style={{ fontSize: 12, marginRight: 8 }}>
            {t('opencode.plugin.commonPlugins')}:
          </Text>
          <Select
            size="small"
            style={{ width: 380 }}
            placeholder={t('opencode.plugin.selectPlugin')}
            options={availableCommonPlugins}
            value={undefined}
            onChange={handleSelectPlugin}
            allowClear
          />
        </div>
      )}
    </Space>
  );

  return (
    <Collapse
      style={{ marginBottom: 16 }}
      activeKey={collapsed ? [] : ['plugin']}
      onChange={(keys) => setCollapsed(!keys.includes('plugin'))}
      items={[
        {
          key: 'plugin',
          label: <Text strong>{t('opencode.plugin.title')}</Text>,
          children: content,
        },
      ]}
    />
  );
};

export default PluginSettings;
