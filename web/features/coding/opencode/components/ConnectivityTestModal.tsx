import React from 'react';
import { Modal, Form, Input, InputNumber, Button, Collapse, Table, Tag, Space, Tooltip, message, Switch, Typography, Row, Col, Checkbox, Popconfirm, type TableProps } from 'antd';
import { CaretRightOutlined, SettingOutlined, InfoCircleOutlined, DeleteOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import {
  testProviderModelConnectivity,
  type ConnectivityTestRequest,
  type ConnectivityTestResult,
  type OpenCodeDiagnosticsConfig,
} from '@/services/opencodeApi';
import type { OpenCodeProvider } from '@/types/opencode';


interface ConnectivityTestModalProps {
  open: boolean;
  onCancel: () => void;
  providerId: string;
  providerName: string;
  providerConfig: OpenCodeProvider;
  modelIds: string[];
  diagnostics?: OpenCodeDiagnosticsConfig;
  onSaveDiagnostics: (diagnostics: OpenCodeDiagnosticsConfig) => Promise<void>;
  onRemoveModels?: (modelIds: string[]) => Promise<void>;
}

interface TestResult extends Partial<ConnectivityTestResult> {
  key: string;
  modelId: string;
  status: string;
  loading?: boolean;
}

const SUPPORTED_NPMS = [
  '@ai-sdk/openai',
  '@ai-sdk/openai-compatible',
  '@ai-sdk/google',
  '@ai-sdk/anthropic',
];

const ConnectivityTestModal: React.FC<ConnectivityTestModalProps> = ({
  open,
  onCancel,
  providerName,
  providerConfig,
  modelIds,
  diagnostics,
  onSaveDiagnostics,
  onRemoveModels,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [testing, setTesting] = React.useState(false);
  const [results, setResults] = React.useState<TestResult[]>([]);
  const [advancedActive, setAdvancedActive] = React.useState<string | string[]>([]);
  const [selectedModelIds, setSelectedModelIds] = React.useState<string[]>([]);
  const [removing, setRemoving] = React.useState(false);
  
  // JSON editor states
  const [headersJson, setHeadersJson] = React.useState<unknown>({});
  const [headersValid, setHeadersValid] = React.useState(true);
  const [bodyJson, setBodyJson] = React.useState<unknown>({});
  const [bodyValid, setBodyValid] = React.useState(true);

  // Details modal state
  const [detailsModalOpen, setDetailsModalOpen] = React.useState(false);
  const [selectedResult, setSelectedResult] = React.useState<TestResult | null>(null);

  // Initialize form with diagnostics prop
  React.useEffect(() => {
    if (open) {
      form.setFieldsValue({
        prompt: diagnostics?.prompt || 'say hi!',
        temperature: diagnostics?.temperature,
        maxTokens: diagnostics?.maxTokens ?? diagnostics?.maxOutputTokens,
        stream: diagnostics?.stream ?? true,
      });

      setHeadersJson(diagnostics?.headers || {});
      setBodyJson(diagnostics?.body || {});
      setSelectedModelIds([]);

      setResults(modelIds.map(id => ({
        key: id,
        modelId: id,
        status: 'pending',
        requestUrl: '',
        requestHeaders: {},
        requestBody: {},
        responseHeaders: undefined,
        responseBody: undefined,
      })));
    }
  }, [open, diagnostics, modelIds, form]);

  const handleRunTest = async () => {
    try {
      const values = await form.validateFields();
      
      if (!headersValid || !bodyValid) {
        message.error(t('opencode.connectivity.invalidJson'));
        return;
      }

      const headersHasValue = headersJson !== undefined && headersJson !== null && (
        typeof headersJson !== 'string' || headersJson.trim() !== ''
      );
      const headersIsObject = headersJson && typeof headersJson === 'object' && !Array.isArray(headersJson);
      if (headersHasValue && !headersIsObject) {
        message.error(t('opencode.connectivity.invalidJson'));
        return;
      }

      const bodyHasValue = bodyJson !== undefined && bodyJson !== null && (
        typeof bodyJson !== 'string' || bodyJson.trim() !== ''
      );
      const bodyIsObject = bodyJson && typeof bodyJson === 'object' && !Array.isArray(bodyJson);
      if (bodyHasValue && !bodyIsObject) {
        message.error(t('opencode.connectivity.invalidJson'));
        return;
      }

      setTesting(true);

      // 1. Save diagnostics configuration
      const npm = providerConfig.npm || '@ai-sdk/openai-compatible';
      const isGoogle = npm === '@ai-sdk/google';

      const headersObject = (headersJson && typeof headersJson === 'object' && !Array.isArray(headersJson))
        ? (headersJson as Record<string, unknown>)
        : undefined;
      const bodyObject = (bodyJson && typeof bodyJson === 'object' && !Array.isArray(bodyJson))
        ? (bodyJson as Record<string, unknown>)
        : undefined;

      const newDiagnostics: OpenCodeDiagnosticsConfig = {
        prompt: values.prompt,
        stream: values.stream,
        ...(values.temperature !== undefined ? { temperature: values.temperature } : {}),
        ...(values.maxTokens !== undefined
          ? (isGoogle ? { maxOutputTokens: values.maxTokens } : { maxTokens: values.maxTokens })
          : {}),
        ...(headersObject ? { headers: headersObject } : {}),
        ...(bodyObject ? { body: bodyObject } : {}),
      };

      await onSaveDiagnostics(newDiagnostics);

      const providerHeaders = (providerConfig.options?.headers as Record<string, unknown>) || {};
      const mergedHeaders = { ...providerHeaders, ...(headersObject || {}) };

      const baseRequest: ConnectivityTestRequest = {
        npm,
        baseUrl: providerConfig.options?.baseURL || '',
        apiKey: providerConfig.options?.apiKey,
        prompt: values.prompt,
        stream: values.stream,
        ...(values.temperature !== undefined ? { temperature: values.temperature } : {}),
        ...(values.maxTokens !== undefined
          ? (isGoogle ? { maxOutputTokens: values.maxTokens } : { maxTokens: values.maxTokens })
          : {}),
        ...(Object.keys(mergedHeaders).length > 0 ? { headers: mergedHeaders } : {}),
        ...(bodyObject ? { body: bodyObject } : {}),
        modelIds: [],
        timeoutSecs: 30,
      };

      // Reset results status to loading
      setResults(prev => prev.map(r => ({ ...r, status: 'loading', loading: true, firstByteMs: undefined, totalMs: undefined, errorMessage: undefined })));

      // 3. Run tests in parallel (streaming effect)
      const failedModelIds: string[] = [];
      const promises = modelIds.map(async (modelId) => {
        try {
          const response = await testProviderModelConnectivity({
            ...baseRequest,
            modelIds: [modelId],
          });

          const result = response.results[0];

          if (result.status !== 'success') {
            failedModelIds.push(modelId);
          }

          setResults(prev => prev.map(r => {
            if (r.modelId === modelId) {
              return { ...result, key: modelId, loading: false };
            }
            return r;
          }));
        } catch (error: any) {
          failedModelIds.push(modelId);
          setResults(prev => prev.map(r => {
            if (r.modelId === modelId) {
              return {
                key: modelId,
                modelId,
                status: 'error',
                errorMessage: error.message || 'Unknown error',
                loading: false,
                requestUrl: '',
                requestHeaders: {},
                requestBody: {},
                responseHeaders: undefined,
                responseBody: undefined,
              };
            }
            return r;
          }));
        }
      });

      await Promise.all(promises);

      // Auto-select failed models
      if (failedModelIds.length > 0) {
        setSelectedModelIds(failedModelIds);
      }

    } catch (error) {
      console.error('Test failed:', error);
      message.error(t('common.error'));
    } finally {
      setTesting(false);
    }
  };

  const handleShowDetails = (record: TestResult) => {
    setSelectedResult(record);
    setDetailsModalOpen(true);
  };

  const handleRemoveModels = async () => {
    if (!onRemoveModels || selectedModelIds.length === 0) return;

    setRemoving(true);
    try {
      await onRemoveModels(selectedModelIds);
      setSelectedModelIds([]);
      message.success(t('opencode.connectivity.removeSuccess', { count: selectedModelIds.length }));
    } catch {
      message.error(t('common.error'));
    } finally {
      setRemoving(false);
    }
  };

  const handleSelectModel = (modelId: string, checked: boolean) => {
    if (checked) {
      setSelectedModelIds(prev => [...prev, modelId]);
    } else {
      setSelectedModelIds(prev => prev.filter(id => id !== modelId));
    }
  };

  const hasFailedModels = results.some(r => r.status === 'error' || r.status === 'timeout');
  const isTestCompleted = results.length > 0 && results.every(r => !r.loading && r.status !== 'pending');

  const columns: TableProps<TestResult>['columns'] = [
    ...(isTestCompleted && hasFailedModels && onRemoveModels ? [{
      title: '',
      key: 'select',
      width: 40,
      render: (_: unknown, record: TestResult) => (
        <Checkbox
          checked={selectedModelIds.includes(record.modelId)}
          onChange={(e) => handleSelectModel(record.modelId, e.target.checked)}
        />
      ),
    }] : []),
    {
      title: t('opencode.connectivity.modelId'),
      dataIndex: 'modelId',
      key: 'modelId',
      width: 320,
    },
    {
      title: t('opencode.connectivity.status'),
      dataIndex: 'status',
      key: 'status',
      width: 100,
      align: 'center',
      render: (status: string, record: TestResult) => {
        if (record.loading) return <Tag color="processing">{t('common.loading')}</Tag>;
        if (status === 'success') return <Tag color="success">{t('opencode.connectivity.success')}</Tag>;
        if (status === 'error' || status === 'timeout') return <Tooltip title={record.errorMessage}><Tag color="error">{status}</Tag></Tooltip>;
        return <Tag>{status}</Tag>;
      },
    },
    {
      title: t('opencode.connectivity.firstByte'),
      dataIndex: 'firstByteMs',
      key: 'firstByteMs',
      width: 120,
      align: 'right',
      render: (value: number | null | undefined) => (value === undefined || value === null ? '-' : `${value}ms`),
    },
    {
      title: t('opencode.connectivity.totalTime'),
      dataIndex: 'totalMs',
      key: 'totalMs',
      width: 180,
      align: 'right',
      render: (value: number | null | undefined, record: TestResult) => {
        if (value === undefined || value === null) return '-';
        const canShowDetails = Boolean(record.requestUrl) && !record.loading;

        return (
          <Space>
            <span>{value}ms</span>
            {canShowDetails && (
              <Button
                type="link"
                size="small"
                onClick={() => handleShowDetails(record)}
                style={{ padding: 0, height: 'auto' }}
              >
                {t('opencode.connectivity.requestDetails')}
              </Button>
            )}
          </Space>
        );
      },
    },
  ];

  const npm = providerConfig.npm || '@ai-sdk/openai-compatible';
  const isSupported = SUPPORTED_NPMS.includes(npm);

  return (
    <Modal
      title={t('opencode.connectivity.title', { name: providerName })}
      open={open}
      onCancel={onCancel}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t('common.close')}
        </Button>,
        <Tooltip title={!isSupported ? t('opencode.connectivity.unsupportedNpm', { npm }) : ''}>
          <Button 
            key="submit" 
            type="primary" 
            icon={<CaretRightOutlined />} 
            loading={testing} 
            onClick={handleRunTest}
            disabled={!isSupported}
          >
            {t('opencode.connectivity.startTest')}
          </Button>
        </Tooltip>
      ]}
      width={800}
      styles={{ body: { paddingBottom: 16 } }}
    >
      <Form 
        form={form} 
        labelCol={{ span: 4 }}
        wrapperCol={{ span: 20 }}
      >
        <Form.Item 
          label={t('opencode.connectivity.prompt')} 
          name="prompt" 
          rules={[{ required: true }]}
        >
          <Input.TextArea rows={2} />
        </Form.Item>

        <Collapse 
          ghost 
          activeKey={advancedActive} 
          onChange={setAdvancedActive}
          items={[
            {
              key: 'advanced',
              label: <Space><SettingOutlined /> {t('opencode.connectivity.moreParams')}</Space>,
              children: (
                <>
                  <Row gutter={24} style={{ marginBottom: 16 }}>
                    <Col span={8}>
                      <Form.Item 
                        label={t('opencode.connectivity.temperature')} 
                        name="temperature" 
                        style={{ marginBottom: 0 }}
                        labelCol={{ span: 10 }}
                        wrapperCol={{ span: 14 }}
                      >
                        <InputNumber min={0} max={2} step={0.1} style={{ width: '100%' }} />
                      </Form.Item>
                    </Col>
                    <Col span={9}>
                      <Form.Item 
                        label={t('opencode.connectivity.maxTokens')} 
                        name="maxTokens" 
                        style={{ marginBottom: 0 }}
                        labelCol={{ span: 11 }}
                        wrapperCol={{ span: 13 }}
                      >
                        <InputNumber min={1} step={100} style={{ width: '100%' }} />
                      </Form.Item>
                    </Col>
                    <Col span={7}>
                      <Form.Item 
                        label={t('opencode.connectivity.stream')} 
                        name="stream" 
                        valuePropName="checked" 
                        style={{ marginBottom: 0 }}
                        labelCol={{ span: 10 }}
                        wrapperCol={{ span: 14 }}
                      >
                        <Switch />
                      </Form.Item>
                    </Col>
                  </Row>

                  <Form.Item
                    label={
                      <span>
                        {t('opencode.connectivity.customHeaders')}
                        <Tooltip title={t('opencode.connectivity.customHeadersHint')}>
                          <InfoCircleOutlined style={{ marginLeft: 3 }} />
                        </Tooltip>
                      </span>
                    }
                  >
                    <JsonEditor
                      value={headersJson}
                      onChange={(val, valid) => { setHeadersJson(val); setHeadersValid(valid); }}
                      mode="text"
                      height={150}
                      placeholder="{}"
                    />
                  </Form.Item>

                  <Form.Item
                    label={
                      <span>
                        {t('opencode.connectivity.customBody')}
                        <Tooltip title={t('opencode.connectivity.customBodyHint')}>
                          <InfoCircleOutlined style={{ marginLeft: 2 }} />
                        </Tooltip>
                      </span>
                    }
                  >
                    <JsonEditor
                      value={bodyJson}
                      onChange={(val, valid) => { setBodyJson(val); setBodyValid(valid); }}
                      mode="text"
                      height={150}
                      placeholder="{}"
                    />
                  </Form.Item>
                </>
              )
            }
          ]}
        />

        <Typography.Title level={5} style={{ marginTop: 16, marginBottom: 8 }}>
          {t('opencode.connectivity.results')}
        </Typography.Title>
        <Table
          dataSource={results}
          columns={columns}
          pagination={false}
          size="small"
          scroll={{ y: 300 }}
        />
        {isTestCompleted && hasFailedModels && onRemoveModels && (
          <div style={{ marginTop: 12, display: 'flex', justifyContent: 'flex-end' }}>
            <Popconfirm
              title={t('opencode.connectivity.removeConfirmTitle')}
              description={t('opencode.connectivity.removeConfirmDesc', { count: selectedModelIds.length })}
              onConfirm={handleRemoveModels}
              okText={t('common.confirm')}
              cancelText={t('common.cancel')}
              disabled={selectedModelIds.length === 0}
            >
              <Button
                danger
                icon={<DeleteOutlined />}
                loading={removing}
                disabled={selectedModelIds.length === 0}
              >
                {t('opencode.connectivity.removeSelected', { count: selectedModelIds.length })}
              </Button>
            </Popconfirm>
          </div>
        )}
      </Form>

      <Modal
        title={t('opencode.connectivity.detailsTitle', { modelId: selectedResult?.modelId || '' })}
        open={detailsModalOpen}
        onCancel={() => setDetailsModalOpen(false)}
        footer={[
          <Button key="close" onClick={() => setDetailsModalOpen(false)}>
            {t('common.close')}
          </Button>
        ]}
        width={800}
      >
        {selectedResult && (
          <JsonEditor
            value={{
              request: {
                url: selectedResult.requestUrl,
                headers: selectedResult.requestHeaders,
                body: selectedResult.requestBody,
              },
              response: {
                headers: selectedResult.responseHeaders,
                body: selectedResult.responseBody,
              }
            }}
            readOnly={true}
            height={500}
            mode="text"
          />
        )}
      </Modal>
    </Modal>
  );
};

export default ConnectivityTestModal;
