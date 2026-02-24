import React from 'react';
import { Modal, Table, Radio, Button, Space, Typography, message, Alert, Input, Tooltip } from 'antd';
import { CloudDownloadOutlined, ReloadOutlined, SearchOutlined, UndoOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { FetchModelsModalProps, FetchedModel, ApiType, FetchModelsResponse } from './types';

const { Text } = Typography;

const FetchModelsModal: React.FC<FetchModelsModalProps> = ({
  open,
  providerName,
  baseUrl,
  apiKey,
  headers,
  sdkType,
  existingModelIds,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  // Default to native if supported, otherwise openai_compat
  const [apiType, setApiType] = React.useState<ApiType>(() => {
    return (sdkType === '@ai-sdk/google' || sdkType === '@ai-sdk/anthropic') ? 'native' : 'openai_compat';
  });
  const [models, setModels] = React.useState<FetchedModel[]>([]);
  const [selectedRowKeys, setSelectedRowKeys] = React.useState<string[]>([]);
  const [error, setError] = React.useState<string | null>(null);
  const [fetched, setFetched] = React.useState(false);
  const [customUrl, setCustomUrl] = React.useState('');
  const [searchText, setSearchText] = React.useState('');

  // Only show Native option for Google and Anthropic SDKs
  const supportsNative = sdkType === '@ai-sdk/google' || sdkType === '@ai-sdk/anthropic';

  // Filter models based on search text
  const filteredModels = React.useMemo(() => {
    if (!searchText) return models;
    const lowerSearch = searchText.toLowerCase();
    return models.filter(m =>
      m.id.toLowerCase().includes(lowerSearch) ||
      (m.name && m.name.toLowerCase().includes(lowerSearch)) ||
      (m.ownedBy && m.ownedBy.toLowerCase().includes(lowerSearch))
    );
  }, [models, searchText]);

  // Calculate the default URL based on baseUrl, apiType, and sdkType
  const calculatedUrl = React.useMemo(() => {
    const base = baseUrl.replace(/\/$/, '');

    if (apiType === 'native' && sdkType === '@ai-sdk/google') {
      // Google Native: /models with API key in URL
      const url = `${base}/models`;
      if (apiKey) {
        return `${url}?key=${apiKey}`;
      }
      return url;
    }

    return `${base}/models`;
  }, [baseUrl, apiType, sdkType, apiKey]);

  // Update custom URL when calculated URL changes (only if not manually edited)
  React.useEffect(() => {
    setCustomUrl(calculatedUrl);
  }, [calculatedUrl]);

  // Reset state when modal opens
  React.useEffect(() => {
    if (open) {
      setModels([]);
      setSelectedRowKeys([]);
      setError(null);
      setFetched(false);
      setSearchText('');
      // Reset custom URL to calculated default
      setCustomUrl(calculatedUrl);
    }
  }, [open, calculatedUrl]);

  // Fetch models from provider API
  const handleFetch = async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await invoke<FetchModelsResponse>('fetch_provider_models', {
        request: {
          baseUrl,
          apiKey,
          headers,
          apiType,
          sdkType,
          customUrl, // Use custom URL instead of calculated one
        },
      });

      setModels(response.models);
      setFetched(true);

      // Don't auto-select, let user choose manually
      setSelectedRowKeys([]);

      if (response.models.length === 0) {
        message.info(t('opencode.fetchModels.noModelsFound'));
      }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(errorMsg);
      message.error(t('opencode.fetchModels.fetchFailed'));
    } finally {
      setLoading(false);
    }
  };

  // Confirm and add selected models
  const handleConfirm = () => {
    const selectedModels = models.filter((m) => selectedRowKeys.includes(m.id));
    onSuccess(selectedModels);
  };

  // Table columns
  const columns = [
    {
      title: t('opencode.fetchModels.modelId'),
      dataIndex: 'id',
      key: 'id',
      render: (id: string) => {
        const isExisting = existingModelIds.includes(id);
        return (
          <Space>
            <Text>{id}</Text>
            {isExisting && (
              <Text type="secondary" style={{ fontSize: 12 }}>
                ({t('opencode.fetchModels.alreadyExists')})
              </Text>
            )}
          </Space>
        );
      },
    },
    {
      title: t('opencode.fetchModels.ownedBy'),
      dataIndex: 'ownedBy',
      key: 'ownedBy',
      width: 150,
      render: (ownedBy: string | undefined) => ownedBy || '-',
    },
  ];

  const rowSelection = {
    selectedRowKeys,
    onChange: (keys: React.Key[]) => setSelectedRowKeys(keys as string[]),
    getCheckboxProps: (record: FetchedModel) => ({
      disabled: existingModelIds.includes(record.id),
      name: record.id,
    }),
  };

  return (
    <Modal
      title={
        <Space>
          <CloudDownloadOutlined />
          {t('opencode.fetchModels.title', { provider: providerName })}
        </Space>
      }
      open={open}
      onCancel={onCancel}
      width={700}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button
          key="confirm"
          type="primary"
          disabled={selectedRowKeys.length === 0}
          onClick={handleConfirm}
        >
          {t('opencode.fetchModels.addSelected', { count: selectedRowKeys.length })}
        </Button>,
      ]}
    >
      <Space direction="vertical" style={{ width: '100%' }} size="middle">
        {/* API Type Selection */}
        <div>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            {t('opencode.fetchModels.apiType')}
          </Text>
          <Radio.Group
            value={apiType}
            onChange={(e) => setApiType(e.target.value)}
          >
            <Radio value="openai_compat" style={{ marginRight: 16 }}>
              {t('opencode.fetchModels.openaiCompat')}
              <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>
                (/models)
              </Text>
            </Radio>
            {supportsNative && (
              <Radio value="native">
                {t('opencode.fetchModels.native')}
                <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>
                  ({t('opencode.fetchModels.nativeHint')})
                </Text>
              </Radio>
            )}
          </Radio.Group>
        </div>

        {/* URL Input */}
        <div>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            {t('opencode.fetchModels.apiUrl')}
          </Text>
          <Input
            value={customUrl}
            onChange={(e) => setCustomUrl(e.target.value)}
            placeholder="https://api.example.com/v1/models"
            style={{ fontFamily: 'monospace' }}
            addonAfter={
              <Tooltip title={t('opencode.fetchModels.resetToDefault')}>
                <Button
                  type="text"
                  size="small"
                  icon={<UndoOutlined />}
                  onClick={() => setCustomUrl(calculatedUrl)}
                  style={{ fontSize: 12 }}
                />
              </Tooltip>
            }
          />
        </div>

        {/* Fetch Button and Search */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Button
            type="primary"
            icon={fetched ? <ReloadOutlined /> : <CloudDownloadOutlined />}
            loading={loading}
            onClick={handleFetch}
          >
            {fetched ? t('opencode.fetchModels.refresh') : t('opencode.fetchModels.fetch')}
          </Button>
          <Input
            prefix={<SearchOutlined />}
            placeholder={t('opencode.fetchModels.searchPlaceholder')}
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
            allowClear
            style={{ width: 250 }}
          />
        </div>

        {/* Error Alert */}
        {error && (
          <Alert
            type="error"
            message={t('opencode.fetchModels.fetchFailed')}
            description={error}
            showIcon
            closable
            onClose={() => setError(null)}
          />
        )}

        {/* Models Table */}
        {fetched && (
          <Table
            rowKey="id"
            columns={columns}
            dataSource={filteredModels}
            rowSelection={rowSelection}
            pagination={false}
            scroll={{ y: 300 }}
            size="small"
            locale={{
              emptyText: searchText ? t('opencode.fetchModels.noSearchResults') : t('opencode.fetchModels.noModelsFound'),
            }}
          />
        )}
      </Space>
    </Modal>
  );
};

export default FetchModelsModal;
