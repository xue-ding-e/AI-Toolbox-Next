import React from 'react';
import { App, Button, Collapse, Empty, Form, Input, Modal, Select, Space, Tooltip, Typography } from 'antd';
import {
  ApiOutlined,
  CloudDownloadOutlined,
  DownOutlined,
  PlusOutlined,
  RightOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import ProviderCard from '@/components/common/ProviderCard';
import type {
  ModelDisplayData,
  ProviderDisplayData,
  ProviderConnectivityStatusItem,
} from '@/components/common/ProviderCard/types';
import ModelFormModal, { type ModelFormValues } from '@/components/common/ModelFormModal';
import FetchModelsModal from '@/components/common/FetchModelsModal';
import type { FetchModelsApplyResult } from '@/components/common/FetchModelsModal/types';
import ProviderConnectivityTestModal from '@/features/coding/shared/providerConnectivity/ProviderConnectivityTestModal';
import {
  buildProviderConnectivityBatchTarget,
  runProviderConnectivityBatch,
} from '@/features/coding/shared/providerConnectivity/batchTest';
import {
  ProviderSearchEmpty,
  ProviderSearchInput,
  ProviderSortDropdown,
  PROVIDER_SORT_MODES_BASIC,
  filterProviderItems,
  sortProviderItems,
  useProviderListSort,
} from '@/features/coding/shared/providerList';
import JsonEditor from '@/components/common/JsonEditor';
import {
  deleteOmoNativeProvider,
  listOmoNativeProviders,
  saveOmoNativeProvider,
} from '@/services/omoNativeApi';
import type { OmoNativeProvider } from '@/types/omoNative';
import {
  OMO_NATIVE_API_OPTIONS,
  asRecord,
  getNumberField,
  getOmoNativeModelEntries,
  getStringField,
  isRecordEmpty,
  omoNativeApiToNpm,
  omoNativeProviderToConnectivityInfo,
  omoNativeProviderToDisplayData,
  type OmoNativeModelEntry,
} from '../utils/omoNativeProviders';

const { Text } = Typography;

/**
 * Native 的自定义 provider 存在引擎的 `models.json`（`providers` 键）里，密钥存在
 * `auth.json`。UI 与 OMP/Pi 同级：共享 ProviderCard + 共享表单弹窗 + 共享连通性测试。
 *
 * `models.json` 没有对外发布的 schema，字段形状反推自上游 `convertProvider`：
 * provider 级 `name` / `baseUrl` / `api` / `headers`，模型级 `id` / `name` /
 * `reasoning` / `input` / `contextWindow` / `maxTokens`。写入按 provider key 局部更新，
 * 未知字段原样保留。
 */

interface ProviderModalState {
  provider?: OmoNativeProvider;
  copy?: boolean;
}

const OmoNativeProvidersSection: React.FC = () => {
  const { t } = useTranslation();
  const { message } = App.useApp();

  const [providers, setProviders] = React.useState<OmoNativeProvider[]>([]);
  const [providerModal, setProviderModal] = React.useState<ProviderModalState | null>(null);
  const [providerForm] = Form.useForm();
  const [providerConfigJson, setProviderConfigJson] = React.useState<Record<string, unknown>>({});
  const [providerHeadersJson, setProviderHeadersJson] = React.useState<Record<string, unknown>>({});
  const [providerHeadersJsonValid, setProviderHeadersJsonValid] = React.useState(true);
  const [providerAdvancedExpanded, setProviderAdvancedExpanded] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [modelModal, setModelModal] = React.useState<{
    provider: OmoNativeProvider;
    modelId?: string;
  } | null>(null);
  const [fetchModelsProvider, setFetchModelsProvider] = React.useState<OmoNativeProvider | null>(null);
  const [connectivityProvider, setConnectivityProvider] = React.useState<OmoNativeProvider | null>(null);
  const [connectivityStatuses, setConnectivityStatuses] = React.useState<
    Record<string, ProviderConnectivityStatusItem>
  >({});
  const [batchTesting, setBatchTesting] = React.useState(false);
  const [providerKeyword, setProviderKeyword] = React.useState('');

  const loadProviders = React.useCallback(async () => {
    try {
      setProviders(await listOmoNativeProviders());
    } catch (error) {
      console.error('Failed to load OmO Native providers:', error);
      message.error(t('common.error'));
    }
  }, [t, message]);

  React.useEffect(() => {
    void loadProviders();
  }, [loadProviders]);

  // 内建 provider 由引擎自带，没有 models.json 条目可改；只有自定义的能编辑/删除。
  const customProviders = React.useMemo(
    () => providers.filter((provider) => provider.custom),
    [providers],
  );

  const { sortMode, setSortMode, lastUsedAt } = useProviderListSort('omo_native');
  const visibleProviders = React.useMemo(
    () =>
      sortProviderItems(
        filterProviderItems(customProviders, providerKeyword, (provider) => [
          provider.key,
          getStringField(provider.config, 'name'),
          getStringField(provider.config, 'baseUrl'),
          ...getOmoNativeModelEntries(provider.config).map((entry) => entry.id),
        ]),
        sortMode,
        { name: (provider) => getStringField(provider.config, 'name') || provider.key },
        (provider) => lastUsedAt(provider.key),
      ),
    [customProviders, providerKeyword, sortMode, lastUsedAt],
  );

  const openProviderModal = (provider?: OmoNativeProvider, options?: { copy?: boolean }) => {
    const isCopy = options?.copy === true;
    const nextConfig = provider ? asRecord(provider.config) : {};
    setProviderModal({ provider: isCopy ? undefined : provider, copy: isCopy });
    setProviderConfigJson(nextConfig);
    setProviderHeadersJson(asRecord(nextConfig.headers));
    setProviderHeadersJsonValid(true);
    setProviderAdvancedExpanded(false);
    providerForm.setFieldsValue({
      providerKey: isCopy && provider ? `${provider.key}_copy` : provider?.key,
      displayName: getStringField(nextConfig, 'name'),
      api: getStringField(nextConfig, 'api') || undefined,
      baseUrl: getStringField(nextConfig, 'baseUrl'),
      apiKey: '',
    });
  };

  const handleSaveProvider = async () => {
    if (!providerModal || !providerHeadersJsonValid) return;
    const values = await providerForm.validateFields();
    const providerKey = (values.providerKey as string | undefined)?.trim();
    if (!providerKey) {
      message.error(t('omoNative.providers.providerKeyRequired'));
      return;
    }

    setSaving(true);
    try {
      const nextConfig: Record<string, unknown> = { ...providerConfigJson };
      const setOptionalString = (key: string, value?: string) => {
        const trimmed = value?.trim();
        if (trimmed) {
          nextConfig[key] = trimmed;
        } else {
          delete nextConfig[key];
        }
      };
      setOptionalString('name', values.displayName);
      setOptionalString('api', values.api);
      setOptionalString('baseUrl', values.baseUrl);
      if (isRecordEmpty(providerHeadersJson)) {
        delete nextConfig.headers;
      } else {
        nextConfig.headers = providerHeadersJson;
      }

      await saveOmoNativeProvider({
        key: providerKey,
        config: nextConfig,
        apiKey: (values.apiKey as string | undefined)?.trim() || undefined,
      });
      message.success(t('common.success'));
      setProviderModal(null);
      await loadProviders();
    } catch (error) {
      console.error('Failed to save OmO Native provider:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteProvider = (provider: OmoNativeProvider) => {
    Modal.confirm({
      title: t('omoNative.providers.deleteConfirmTitle', { name: provider.key }),
      content: t('omoNative.providers.deleteConfirmContent'),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      onOk: async () => {
        try {
          await deleteOmoNativeProvider(provider.key, true);
          message.success(t('common.success'));
          await loadProviders();
        } catch (error) {
          console.error('Failed to delete OmO Native provider:', error);
          message.error(t('common.error'));
        }
      },
    });
  };

  /** 模型改动一律「读整份 provider 配置 → 改 models → 按 key 写回」，未知字段不动。 */
  const persistModels = async (
    provider: OmoNativeProvider,
    nextModels: Record<string, unknown>[],
  ) => {
    const nextConfig = { ...asRecord(provider.config) };
    if (nextModels.length > 0) {
      nextConfig.models = nextModels;
    } else {
      delete nextConfig.models;
    }
    await saveOmoNativeProvider({ key: provider.key, config: nextConfig });
    await loadProviders();
  };

  const handleSaveModel = async (values: ModelFormValues) => {
    if (!modelModal) return;
    const provider = modelModal.provider;
    const entries = getOmoNativeModelEntries(provider.config);
    const nextModel: Record<string, unknown> = { id: values.id };
    if (values.name?.trim()) nextModel.name = values.name.trim();
    if (values.api) nextModel.api = values.api;
    if (typeof values.reasoning === 'boolean') nextModel.reasoning = values.reasoning;
    if (values.inputTypes?.trim()) nextModel.input = values.inputTypes.trim();
    if (typeof values.contextLimit === 'number') nextModel.contextWindow = values.contextLimit;
    if (typeof values.outputLimit === 'number') nextModel.maxTokens = values.outputLimit;
    if (values.thinking?.trim()) {
      try {
        nextModel.thinking = JSON.parse(values.thinking);
      } catch {
        message.error(t('omoNative.providers.invalidJson'));
        return;
      }
    }

    const isEdit = Boolean(modelModal.modelId);
    if (!isEdit && entries.some((entry) => entry.id === values.id)) {
      message.error(t('omoNative.providers.modelIdExists'));
      return;
    }

    const nextEntries: OmoNativeModelEntry[] = entries.map((entry) =>
      isEdit && entry.id === modelModal.modelId ? { id: values.id, model: nextModel } : entry,
    );
    if (!isEdit) {
      nextEntries.push({ id: values.id, model: nextModel });
    }

    try {
      await persistModels(
        provider,
        nextEntries.map((entry) => entry.model),
      );
      message.success(t('common.success'));
      setModelModal(null);
    } catch (error) {
      console.error('Failed to save OmO Native model:', error);
      message.error(t('common.error'));
    }
  };

  const handleDeleteModel = async (provider: OmoNativeProvider, modelId: string) => {
    const nextModels = getOmoNativeModelEntries(provider.config)
      .filter((entry) => entry.id !== modelId)
      .map((entry) => entry.model);
    try {
      await persistModels(provider, nextModels);
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to delete OmO Native model:', error);
      message.error(t('common.error'));
    }
  };

  const handleFetchModelsSuccess = async (result: FetchModelsApplyResult) => {
    if (!fetchModelsProvider) return;
    const provider = fetchModelsProvider;
    const entries = getOmoNativeModelEntries(provider.config);
    const existingIds = new Set(entries.map((entry) => entry.id));
    for (const model of result.selectedModels) {
      if (existingIds.has(model.id)) continue;
      entries.push({ id: model.id, model: { id: model.id, name: model.name } });
    }
    try {
      await persistModels(
        provider,
        entries.map((entry) => entry.model),
      );
      message.success(t('common.success'));
      setFetchModelsProvider(null);
    } catch (error) {
      console.error('Failed to apply fetched models:', error);
      message.error(t('common.error'));
    }
  };

  const handleBatchTestProviders = async () => {
    const eligible = visibleProviders
      .map((provider) => {
        const modelIds = getOmoNativeModelEntries(provider.config).map((entry) => entry.id);
        const baseUrl = getStringField(provider.config, 'baseUrl');
        if (!baseUrl || modelIds.length === 0) return null;
        return buildProviderConnectivityBatchTarget(
          {
            providerId: provider.key,
            providerName: getStringField(provider.config, 'name') || provider.key,
            providerConfig: {
              npm: omoNativeApiToNpm(getStringField(provider.config, 'api')),
              options: {
                baseURL: baseUrl,
                headers: asRecord(provider.config.headers),
              },
            },
            configValueMode: 'omp' as const,
            modelIds,
          },
          {
            requireBaseUrl: true,
            requireApiKey: false,
            errorMessages: {
              missingBaseUrl: t('common.baseUrlMissing'),
              missingApiKey: t('common.apiKeyMissing'),
              missingModel: t('common.modelMissing'),
            },
          },
        );
      })
      .filter((target): target is NonNullable<typeof target> => target !== null);

    if (eligible.length === 0) {
      message.warning(t('omoNative.providers.noTestableProvider'));
      return;
    }

    setConnectivityStatuses(
      Object.fromEntries(eligible.map((target) => [target.providerId, { status: 'running' as const }])),
    );
    setBatchTesting(true);
    try {
      await runProviderConnectivityBatch(eligible, (providerId, status) => {
        const nextStatus: ProviderConnectivityStatusItem =
          status.status === 'success'
            ? {
                ...status,
                tooltipMessage:
                  status.totalMs !== undefined
                    ? t('common.connectivityBatchSuccessWithTiming', {
                        model: status.modelId || t('common.notSet'),
                        totalMs: status.totalMs,
                      })
                    : t('common.connectivityBatchSuccess', {
                        model: status.modelId || t('common.notSet'),
                      }),
              }
            : status;
        setConnectivityStatuses((previous) => ({ ...previous, [providerId]: nextStatus }));
      });
    } catch (error) {
      console.error('Failed to batch test OmO Native providers:', error);
      message.error(t('common.error'));
    } finally {
      setBatchTesting(false);
    }
  };

  const renderProvider = (provider: OmoNativeProvider) => {
    const entries = getOmoNativeModelEntries(provider.config);
    const modelDisplayList: ModelDisplayData[] = entries.map((entry) => ({
      id: entry.id,
      name: getStringField(entry.model, 'name') || entry.id,
    }));
    const hasBaseUrl = Boolean(getStringField(provider.config, 'baseUrl'));
    const hasModels = modelDisplayList.length > 0;
    const canTest = hasBaseUrl && hasModels;
    const missingReason = !hasBaseUrl ? t('common.baseUrlMissing') : t('common.modelMissing');

    const providerDisplay: ProviderDisplayData = omoNativeProviderToDisplayData(provider, {
      fallbackBaseUrl: t('omoNative.providers.modelsJsonSource'),
    });

    return (
      <ProviderCard
        key={provider.key}
        provider={providerDisplay}
        models={modelDisplayList}
        onEdit={() => openProviderModal(provider)}
        onCopy={() => openProviderModal(provider, { copy: true })}
        onDelete={() => handleDeleteProvider(provider)}
        deleteConfirm={false}
        connectivityStatus={canTest ? connectivityStatuses[provider.key] : undefined}
        extraActions={
          <Space size={0}>
            <Tooltip title={canTest ? '' : missingReason}>
              <span>
                <Button
                  size="small"
                  type="text"
                  style={{ fontSize: 12 }}
                  disabled={!canTest}
                  onClick={() => setConnectivityProvider(provider)}
                >
                  <ApiOutlined style={{ marginRight: 4 }} />
                  {t('ohMyPi.connectivity.button')}
                </Button>
              </span>
            </Tooltip>
            <Tooltip title={hasBaseUrl ? '' : t('common.baseUrlMissing')}>
              <span>
                <Button
                  size="small"
                  type="text"
                  style={{ fontSize: 12 }}
                  disabled={!hasBaseUrl}
                  onClick={() => setFetchModelsProvider(provider)}
                >
                  <CloudDownloadOutlined style={{ marginRight: 4 }} />
                  {t('ohMyPi.fetchModels.button')}
                </Button>
              </span>
            </Tooltip>
          </Space>
        }
        onAddModel={() => setModelModal({ provider })}
        onEditModel={(modelId) => setModelModal({ provider, modelId })}
        onDeleteModel={(modelId) => handleDeleteModel(provider, modelId)}
      />
    );
  };

  const builtinCount = providers.length - customProviders.length;

  return (
    <>
      <Collapse
        bordered={false}
        defaultActiveKey={['providers']}
        items={[
          {
            key: 'providers',
            label: (
              <Space>
                <ApiOutlined />
                <Text strong>{t('omoNative.providers.title')}</Text>
                {builtinCount > 0 && (
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {t('omoNative.providers.builtinCount', { count: builtinCount })}
                  </Text>
                )}
              </Space>
            ),
            extra: (
              <Space onClick={(event) => event.stopPropagation()}>
                <ProviderSearchInput value={providerKeyword} onChange={setProviderKeyword} />
                <ProviderSortDropdown
                  mode={sortMode}
                  modes={PROVIDER_SORT_MODES_BASIC}
                  onChange={setSortMode}
                />
                <Button
                  type="link"
                  size="small"
                  style={{ fontSize: 12 }}
                  icon={<ApiOutlined />}
                  loading={batchTesting}
                  onClick={handleBatchTestProviders}
                >
                  {t('common.batchTest')}
                </Button>
                <Button
                  type="link"
                  size="small"
                  style={{ fontSize: 12 }}
                  icon={<PlusOutlined />}
                  onClick={() => openProviderModal()}
                >
                  {t('omoNative.providers.addSupplier')}
                </Button>
              </Space>
            ),
            children: (
              <div>
                <div
                  style={{
                    fontSize: 12,
                    color: 'var(--color-text-tertiary)',
                    borderLeft: '2px solid var(--color-border)',
                    paddingLeft: 8,
                    marginBottom: 12,
                  }}
                >
                  <div>{t('omoNative.providers.sectionHint')}</div>
                  <div>{t('omoNative.providers.builtinHint')}</div>
                </div>
                {visibleProviders.length ? (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                    {visibleProviders.map(renderProvider)}
                  </div>
                ) : customProviders.length ? (
                  <ProviderSearchEmpty />
                ) : (
                  <Empty description={t('omoNative.providers.emptyText')} />
                )}
              </div>
            ),
          },
        ]}
      />

      <Modal
        title={
          providerModal?.provider
            ? t('omoNative.providers.editSupplierTitle', { name: providerModal.provider.key })
            : t('omoNative.providers.addSupplierTitle')
        }
        open={!!providerModal}
        width={860}
        confirmLoading={saving}
        onCancel={() => setProviderModal(null)}
        onOk={handleSaveProvider}
        destroyOnHidden
      >
        <Form form={providerForm} layout="vertical">
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
              gap: 12,
              marginBottom: 12,
            }}
          >
            <Form.Item
              label={t('omoNative.providers.providerKey')}
              name="providerKey"
              rules={[{ required: true, message: t('omoNative.providers.providerKeyRequired') }]}
            >
              <Input disabled={!!providerModal?.provider} placeholder="axonhub-chat" />
            </Form.Item>
            <Form.Item label={t('omoNative.providers.displayName')} name="displayName">
              <Input placeholder={t('omoNative.providers.displayNamePlaceholder')} />
            </Form.Item>
          </div>

          <div style={{ marginBottom: 12 }}>
            <Text strong>{t('omoNative.providers.configSection')}</Text>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
                gap: 12,
                marginTop: 8,
              }}
            >
              <Form.Item label={t('omoNative.providers.apiType')} name="api">
                <Select allowClear showSearch options={OMO_NATIVE_API_OPTIONS} />
              </Form.Item>
              <Form.Item label={t('omoNative.providers.baseUrl')} name="baseUrl">
                <Input placeholder="https://api.example.com/v1" />
              </Form.Item>
              <Form.Item
                label={t('omoNative.providers.providerApiKey')}
                name="apiKey"
                tooltip={t('omoNative.providers.apiKeyHint')}
              >
                <Input.Password
                  autoComplete="off"
                  placeholder={providerModal?.provider ? '••••••' : ''}
                />
              </Form.Item>
            </div>
          </div>

          <div style={{ marginBottom: 8 }}>
            <Button
              type="link"
              style={{ padding: 0 }}
              onClick={() => setProviderAdvancedExpanded((value) => !value)}
            >
              {providerAdvancedExpanded ? <DownOutlined /> : <RightOutlined />}
              <span style={{ marginLeft: 4 }}>{t('common.advancedSettings')}</span>
            </Button>
          </div>
          {providerAdvancedExpanded && (
            <div>
              <Text type="secondary">{t('omoNative.providers.headersJson')}</Text>
              <JsonEditor
                value={isRecordEmpty(providerHeadersJson) ? undefined : providerHeadersJson}
                height={160}
                onChange={(value, isValid) => {
                  if (isValid) setProviderHeadersJson(asRecord(value));
                  setProviderHeadersJsonValid(isValid);
                }}
              />
            </div>
          )}
        </Form>
      </Modal>

      <ModelFormModal
        open={!!modelModal}
        width={700}
        isEdit={!!modelModal?.modelId}
        initialValues={
          modelModal?.modelId
            ? (() => {
                const entry = getOmoNativeModelEntries(modelModal.provider.config).find(
                  (item) => item.id === modelModal.modelId,
                );
                if (!entry) return undefined;
                return {
                  id: entry.id,
                  name: getStringField(entry.model, 'name'),
                  api: getStringField(entry.model, 'api'),
                  reasoning:
                    typeof entry.model.reasoning === 'boolean' ? entry.model.reasoning : undefined,
                  inputTypes: typeof entry.model.input === 'string' ? entry.model.input : undefined,
                  contextLimit: getNumberField(entry.model, 'contextWindow'),
                  outputLimit: getNumberField(entry.model, 'maxTokens'),
                  thinking:
                    entry.model.thinking && typeof entry.model.thinking === 'object'
                      ? JSON.stringify(entry.model.thinking)
                      : undefined,
                };
              })()
            : undefined
        }
        existingIds={
          modelModal && !modelModal.modelId
            ? getOmoNativeModelEntries(modelModal.provider.config).map((entry) => entry.id)
            : []
        }
        showOptions={false}
        showVariants={false}
        showModalities={false}
        showInputTypes
        showApi
        apiOptions={OMO_NATIVE_API_OPTIONS}
        showReasoning
        showOmpThinking
        limitRequired={false}
        nameRequired={false}
        toolName="OMO Native"
        onCancel={() => setModelModal(null)}
        onSuccess={handleSaveModel}
      />

      {fetchModelsProvider && (
        <FetchModelsModal
          open={!!fetchModelsProvider}
          providerId={fetchModelsProvider.key}
          providerName={
            getStringField(fetchModelsProvider.config, 'name') || fetchModelsProvider.key
          }
          baseUrl={getStringField(fetchModelsProvider.config, 'baseUrl')}
          headers={asRecord(fetchModelsProvider.config.headers) as Record<string, string>}
          sdkType={omoNativeApiToNpm(getStringField(fetchModelsProvider.config, 'api'))}
          configValueMode="omp"
          existingModelIds={getOmoNativeModelEntries(fetchModelsProvider.config).map(
            (entry) => entry.id,
          )}
          onCancel={() => setFetchModelsProvider(null)}
          onSuccess={handleFetchModelsSuccess}
        />
      )}

      <ProviderConnectivityTestModal
        open={!!connectivityProvider}
        connectivityInfo={
          connectivityProvider
            ? omoNativeProviderToConnectivityInfo(connectivityProvider)
            : null
        }
        onCancel={() => setConnectivityProvider(null)}
      />
    </>
  );
};

export default OmoNativeProvidersSection;
