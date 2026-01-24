import React from 'react';
import { Tag, Input, Space, Empty, Typography, Collapse, message, Tooltip, Popconfirm } from 'antd';
import { PlusOutlined, CloseOutlined, DownOutlined, RightOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import {
  listFavoritePlugins,
  addFavoritePlugin,
  deleteFavoritePlugin,
  OpenCodeFavoritePlugin,
} from '@/services/opencodeApi';

const { Text } = Typography;

// Core plugin that cannot be deleted
const CORE_PLUGIN = 'oh-my-opencode';

// Mutually exclusive plugins - if one is selected, the other should be disabled
const MUTUALLY_EXCLUSIVE_PLUGINS: Record<string, string[]> = {
  'oh-my-opencode': ['oh-my-opencode-slim'],
  'oh-my-opencode-slim': ['oh-my-opencode'],
};

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
  const [favoritePlugins, setFavoritePlugins] = React.useState<OpenCodeFavoritePlugin[]>([]);
  const [favoriteExpanded, setFavoriteExpanded] = React.useState(false);

  // Load favorite plugins on mount
  React.useEffect(() => {
    loadFavoritePlugins();
  }, []);

  React.useEffect(() => {
    if (inputVisible) {
      inputRef.current?.focus();
    }
  }, [inputVisible]);

  const loadFavoritePlugins = async () => {
    try {
      const plugins = await listFavoritePlugins();
      setFavoritePlugins(plugins);
    } catch (error) {
      console.error('Failed to load favorite plugins:', error);
    }
  };

  // Get plugins that should be disabled due to mutual exclusivity
  const getDisabledPlugins = React.useCallback((): Set<string> => {
    const disabled = new Set<string>();
    for (const selectedPlugin of plugins) {
      const exclusiveList = MUTUALLY_EXCLUSIVE_PLUGINS[selectedPlugin];
      if (exclusiveList) {
        exclusiveList.forEach((p) => disabled.add(p));
      }
    }
    return disabled;
  }, [plugins]);

  const disabledPlugins = getDisabledPlugins();

  const handleClose = (removedPlugin: string) => {
    const newPlugins = plugins.filter((plugin) => plugin !== removedPlugin);
    onChange(newPlugins);
  };

  const handleInputConfirm = async () => {
    if (inputValue && !plugins.includes(inputValue)) {
      // Add to current plugins
      onChange([...plugins, inputValue]);

      // Save to favorites if not already exists
      const existsInFavorites = favoritePlugins.some((p) => p.pluginName === inputValue);
      if (!existsInFavorites) {
        try {
          await addFavoritePlugin(inputValue);
          message.success(t('opencode.plugin.savedToFavorites'));
          await loadFavoritePlugins();
        } catch (error) {
          console.error('Failed to save to favorites:', error);
        }
      }
    }
    setInputVisible(false);
    setInputValue('');
  };

  const handleFavoriteClick = (pluginName: string) => {
    // Check if disabled due to mutual exclusivity
    if (disabledPlugins.has(pluginName)) {
      return;
    }

    // Add to current plugins if not already added
    if (!plugins.includes(pluginName)) {
      onChange([...plugins, pluginName]);
    }
  };

  const handleDeleteFavorite = async (pluginName: string) => {
    // Cannot delete core plugin
    if (pluginName === CORE_PLUGIN) {
      message.warning(t('opencode.plugin.cannotDelete'));
      return;
    }

    try {
      await deleteFavoritePlugin(pluginName);
      message.success(t('opencode.plugin.favoriteDeleted'));
      await loadFavoritePlugins();
    } catch (error) {
      console.error('Failed to delete favorite:', error);
      message.error(t('opencode.plugin.deleteError'));
    }
  };

  // ============================================================================
  // Styles - Designed for visual hierarchy
  // ============================================================================

  // Section title style - darker color
  const sectionTitleStyle: React.CSSProperties = {
    fontSize: 13,
    fontWeight: 500,
    display: 'block',
    marginBottom: 8,
    color: 'rgba(0, 0, 0, 0.85)',
  };

  // Enabled plugins: Prominent blue style (active state)
  const enabledTagStyle: React.CSSProperties = {
    backgroundColor: '#1677ff',
    borderColor: '#1677ff',
    color: '#fff',
    marginBottom: 4,
  };

  // Favorite plugins: Subtle default style (available to add)
  const favoriteTagStyle: React.CSSProperties = {
    cursor: 'pointer',
    transition: 'all 0.2s',
    marginBottom: 4,
    backgroundColor: 'transparent',
  };

  // Already added to enabled: Dimmed to show it's already selected
  const favoriteAddedTagStyle: React.CSSProperties = {
    ...favoriteTagStyle,
    opacity: 0.5,
    cursor: 'default',
  };

  // Disabled due to mutual exclusivity
  const disabledTagStyle: React.CSSProperties = {
    opacity: 0.4,
    cursor: 'not-allowed',
    marginBottom: 4,
    textDecoration: 'line-through',
  };

  const content = (
    <Space direction="vertical" style={{ width: '100%' }} size={12}>
      {/* Enabled plugins - Prominent style */}
      <div>
        <Text style={sectionTitleStyle}>
          {t('opencode.plugin.enabledPlugins')}:
        </Text>
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
              style={enabledTagStyle}
              closeIcon={<CloseOutlined style={{ color: '#fff' }} />}
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
              style={{
                cursor: 'pointer',
                borderStyle: 'dashed',
                borderColor: '#8c8c8c',
                color: 'rgba(0, 0, 0, 0.85)',
                backgroundColor: 'transparent',
              }}
            >
              <PlusOutlined /> {t('opencode.plugin.addPlugin')}
            </Tag>
          )}
        </Space>
      </div>

      {/* Favorite plugins - Collapsible section */}
      <div>
        <div
          onClick={() => setFavoriteExpanded(!favoriteExpanded)}
          style={{ cursor: 'pointer', display: 'inline-flex', alignItems: 'center' }}
        >
          {favoriteExpanded ? (
            <DownOutlined style={{ fontSize: 10, marginRight: 6, color: 'rgba(0, 0, 0, 0.85)' }} />
          ) : (
            <RightOutlined style={{ fontSize: 10, marginRight: 6, color: 'rgba(0, 0, 0, 0.85)' }} />
          )}
          <Text style={{ ...sectionTitleStyle, marginBottom: 0 }}>
            {t('opencode.plugin.favoritePlugins')} ({favoritePlugins.length})
          </Text>
        </div>

        {favoriteExpanded && (
          <div style={{ marginTop: 8, marginLeft: 16 }}>
            <Space wrap>
              {/* All favorite plugins from database */}
              {favoritePlugins.map((plugin) => {
                const isCore = plugin.pluginName === CORE_PLUGIN;
                const isDisabled = disabledPlugins.has(plugin.pluginName);
                const isAlreadyAdded = plugins.includes(plugin.pluginName);

                // Determine tag style based on state
                const tagStyle = isDisabled
                  ? disabledTagStyle
                  : isAlreadyAdded
                  ? favoriteAddedTagStyle
                  : favoriteTagStyle;

                return (
                  <Tooltip
                    key={plugin.id}
                    title={
                      isDisabled
                        ? t('opencode.plugin.mutuallyExclusive')
                        : isAlreadyAdded
                        ? t('opencode.plugin.alreadyEnabled')
                        : t('opencode.plugin.clickToAdd')
                    }
                  >
                    <Tag
                      style={tagStyle}
                      onClick={() => handleFavoriteClick(plugin.pluginName)}
                    >
                      {plugin.pluginName}
                      {!isCore && (
                        <span onClick={(e) => e.stopPropagation()}>
                          <Popconfirm
                            title={t('opencode.plugin.confirmDeleteFavorite')}
                            onConfirm={() => handleDeleteFavorite(plugin.pluginName)}
                            okText={t('common.confirm')}
                            cancelText={t('common.cancel')}
                          >
                            <CloseOutlined
                              style={{
                                marginLeft: 4,
                                fontSize: 10,
                                cursor: 'pointer',
                              }}
                              aria-label={t('opencode.plugin.deleteFavorite')}
                            />
                          </Popconfirm>
                        </span>
                      )}
                    </Tag>
                  </Tooltip>
                );
              })}
            </Space>
          </div>
        )}
      </div>
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
