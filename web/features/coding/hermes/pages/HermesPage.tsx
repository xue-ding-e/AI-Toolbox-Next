import React from 'react';
import {
  Button,
  Collapse,
  Empty,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Spin,
  Tooltip,
  Typography,
  message,
} from 'antd';
import {
  ApiOutlined,
  DatabaseOutlined,
  DownOutlined,
  EditOutlined,
  EllipsisOutlined,
  EyeOutlined,
  FileTextOutlined,
  FolderOpenOutlined,
  GlobalOutlined,
  PlusOutlined,
  QuestionCircleOutlined,
  ReloadOutlined,
  RightOutlined,
  RobotOutlined,
  SettingOutlined,
  ThunderboltOutlined,
  ToolOutlined,
} from '@ant-design/icons';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';
import JsonPreviewModal from '@/components/common/JsonPreviewModal';
import ProviderCard from '@/components/common/ProviderCard';
import type {
  ModelDisplayData,
  ProviderConnectivityStatusItem,
  ProviderDisplayData,
} from '@/components/common/ProviderCard/types';
import SectionSidebarLayout, {
  type SidebarSectionMarker,
} from '@/components/layout/SectionSidebarLayout/SectionSidebarLayout';
import { SessionManagerPanel } from '@/features/coding/shared/sessionManager';
import SidebarSettingsModal from '@/components/common/SidebarSettingsModal';
import { TRAY_CONFIG_REFRESH_EVENT } from '@/constants/configEvents';
import ProviderConnectivityTestModal from '@/features/coding/shared/providerConnectivity/ProviderConnectivityTestModal';
import {
  buildProviderConnectivityBatchTarget,
  runProviderConnectivityBatch,
} from '@/features/coding/shared/providerConnectivity/batchTest';
import RootDirectoryModal from '@/features/coding/shared/RootDirectoryModal';
import useRootDirectoryConfig from '@/features/coding/shared/useRootDirectoryConfig';
import { GlobalPromptSettings } from '@/features/coding/shared/prompt';
import HermesMemoryPanel from '../components/HermesMemoryPanel';
import { refreshTrayMenu } from '@/services/appApi';
import {
  deleteHermesRuntimeProvider,
  getHermesSettingsConfig,
  launchHermesDashboard,
  openHermesWebUi,
  readHermesRuntimeConfig,
  saveHermesModelSettings,
  saveHermesModelsProvider,
  saveHermesOtherSettings,
  saveHermesSettingsConfig,
} from '@/services/hermesApi';
import { hermesPromptApi } from '@/services/hermesPromptApi';
import type {
  HermesRuntimeConfig,
  HermesRuntimeProviderView,
} from '@/types/hermes';
import { useSettingsStore } from '@/stores';

import JsonEditor from '@/components/common/JsonEditor';
import ModelFormModal from '@/components/common/ModelFormModal';
import type { ModelFormValues } from '@/components/common/ModelFormModal';
import {
  asRecord,
  buildHermesConnectivityProvider,
  getNumberField,
  getProviderModelRecords,
  getStringField,
  isRecordEmpty,
  maskCredential,
  setOptionalStringField,
} from '../utils/hermesUtils';
import styles from './HermesPage.module.less';

const { Title, Text, Link } = Typography;

interface HermesProviderModalState {
  provider?: HermesRuntimeProviderView;
}

interface HermesModelModalState {
  provider: HermesRuntimeProviderView;
  modelId?: string;
}

/** Shape accepted by the shared root-directory hook (maps to Hermes `configDir`). */
interface HermesCommonConfigLike {
  config?: string;
  rootDir?: string | null;
}

const SIDEBAR_ICON_BY_SECTION_ID: Record<string, React.ReactNode> = {
  'hermes-model-settings': <RobotOutlined />,
  'hermes-providers': <DatabaseOutlined />,
  'hermes-global-prompt': <FileTextOutlined />,
  'hermes-memory': <EditOutlined />,
  'hermes-other-configuration': <ToolOutlined />,
};

const HERMES_API_MODE_OPTIONS = [
  'anthropic',
  'openai',
  'openai-completions',
  'openai-responses',
  'google',
  'gemini',
  'custom',
].map((value) => ({ value, label: value }));

const HermesPage: React.FC = () => {
  const { t } = useTranslation();
  const { sidebarHiddenByPage, setSidebarHidden } = useSettingsStore();
  const [loading, setLoading] = React.useState(true);
  const [saving, setSaving] = React.useState(false);
  const [runtimeConfig, setRuntimeConfig] = React.useState<HermesRuntimeConfig | null>(null);
  const [modelForm] = Form.useForm();
  const [providerModalForm] = Form.useForm();
  const [providerModal, setProviderModal] = React.useState<HermesProviderModalState | null>(null);
  const [providerJson, setProviderJson] = React.useState<Record<string, unknown>>({});
  const [providerJsonValid, setProviderJsonValid] = React.useState(true);
  const [providerAdvancedExpanded, setProviderAdvancedExpanded] = React.useState(false);
  const [modelModal, setModelModal] = React.useState<HermesModelModalState | null>(null);
  const [connectivityProviderId, setConnectivityProviderId] = React.useState<string | null>(null);
  const [connectivityModalOpen, setConnectivityModalOpen] = React.useState(false);
  const [connectivityStatuses, setConnectivityStatuses] = React.useState<Record<string, ProviderConnectivityStatusItem>>({});
  const [batchTestingProviders, setBatchTestingProviders] = React.useState(false);
  const [otherSettings, setOtherSettings] = React.useState<Record<string, unknown>>({});
  const [otherSettingsValid, setOtherSettingsValid] = React.useState(true);
  const [previewModalOpen, setPreviewModalOpen] = React.useState(false);
  const [settingsModalOpen, setSettingsModalOpen] = React.useState(false);
  const modelSettingsSaveSeqRef = React.useRef(0);
  const sidebarHidden = sidebarHiddenByPage.hermes;

  const sidebarSections = React.useMemo<SidebarSectionMarker[]>(() => [
    {
      id: 'hermes-model-settings',
      title: t('hermes.modelSettings.title', { defaultValue: 'Model Settings' }),
      order: 1,
    },
    {
      id: 'hermes-providers',
      title: t('hermes.provider.title', { defaultValue: 'Providers' }),
      order: 2,
    },
    {
      id: 'hermes-global-prompt',
      title: t('hermes.prompt.title', { defaultValue: 'Global Prompt' }),
      order: 3,
    },
    {
      id: 'hermes-memory',
      title: t('hermes.memory.title', { defaultValue: 'Memory' }),
      order: 4,
    },
    {
      id: 'hermes-other-configuration',
      title: t('hermes.otherConfig.title', { defaultValue: 'Other Configuration' }),
      order: 5,
    },
    {
      id: 'hermes-session-manager',
      title: t('sessionManager.title'),
      order: 6,
    },
  ], [t]);

  const loadConfig = React.useCallback(async (silent = false) => {
    if (!silent) {
      setLoading(true);
    }
    try {
      const config = await readHermesRuntimeConfig();
      setRuntimeConfig(config);
      setOtherSettings(config.otherSettings || {});
      modelForm.setFieldsValue({
        defaultProvider: config.modelSettings.defaultProvider || undefined,
        defaultModel: config.modelSettings.defaultModel || undefined,
      });
    } catch (error) {
      console.error('Failed to load Hermes runtime config:', error);
      message.error(t('common.error'));
    } finally {
      if (!silent) {
        setLoading(false);
      }
    }
  }, [modelForm, t]);

  React.useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  React.useEffect(() => {
    const handleTrayConfigRefresh = (event: Event) => {
      event.preventDefault();
      void loadConfig(true);
    };

    window.addEventListener(TRAY_CONFIG_REFRESH_EVENT, handleTrayConfigRefresh);
    return () => {
      window.removeEventListener(TRAY_CONFIG_REFRESH_EVENT, handleTrayConfigRefresh);
    };
  }, [loadConfig]);

  // Root config-dir editing via the shared RootDirectoryModal. Hermes persists a
  // `configDir` in the DB (id "common"); the shared hook uses `rootDir` naming,
  // so we map between the two shapes.
  const {
    rootDirectoryModalOpen,
    setRootDirectoryModalOpen,
    getRootDirectoryModalProps,
    handleSaveRootDirectory,
    handleResetRootDirectory,
  } = useRootDirectoryConfig<HermesCommonConfigLike>({
    t,
    translationKeyPrefix: 'hermes',
    defaultConfig: '{}',
    loadConfig,
    getCommonConfig: async (): Promise<HermesCommonConfigLike | null> => {
      const config = await getHermesSettingsConfig();
      return { config: config?.configDir ?? '', rootDir: config?.configDir ?? null };
    },
    saveCommonConfig: async (input) => {
      if (input.clearRootDir || !input.rootDir) {
        await saveHermesSettingsConfig({ clearConfigDir: true });
        return;
      }
      await saveHermesSettingsConfig({ configDir: input.rootDir });
    },
  });

  const providerOptions = React.useMemo(() => {
    const options = new Map<string, string>();
    runtimeConfig?.providers.forEach((provider) => {
      options.set(provider.providerKey, `${provider.displayName} (${provider.providerKey})`);
    });
    runtimeConfig?.builtinProviders.forEach((provider) => {
      if (!options.has(provider.key)) {
        options.set(provider.key, `${provider.name} (${provider.key})`);
      }
    });
    const current = runtimeConfig?.modelSettings.defaultProvider;
    if (current && !options.has(current)) {
      options.set(current, current);
    }
    return Array.from(options.entries()).map(([value, label]) => ({ value, label }));
  }, [runtimeConfig]);

  const selectedProviderKey = Form.useWatch('defaultProvider', modelForm);
  const selectedDefaultModel = Form.useWatch('defaultModel', modelForm);
  const selectedProvider = runtimeConfig?.providers.find(
    (provider) => provider.providerKey === selectedProviderKey,
  );
  const modelOptions = React.useMemo(() => {
    const options = new Set<string>();
    selectedProvider?.modelIds?.forEach((modelId) => options.add(modelId));
    const current = selectedDefaultModel || runtimeConfig?.modelSettings.defaultModel;
    if (current) {
      options.add(current);
    }
    return Array.from(options).map((modelId) => ({ value: modelId, label: modelId }));
  }, [runtimeConfig?.modelSettings.defaultModel, selectedDefaultModel, selectedProvider?.modelIds]);

  const hermesProviders = React.useMemo(
    () => runtimeConfig?.providers ?? [],
    [runtimeConfig?.providers],
  );

  const connectivityInfo = React.useMemo(() => {
    if (!connectivityProviderId) {
      return null;
    }
    const provider = hermesProviders.find((item) => item.providerKey === connectivityProviderId);
    if (!provider) {
      return null;
    }
    return {
      providerId: provider.providerKey,
      providerName: provider.displayName,
      providerConfig: buildHermesConnectivityProvider(provider),
      modelIds: provider.modelIds ?? [],
    };
  }, [connectivityProviderId, hermesProviders]);

  const handleModelSettingsChange = async (
    changedValues: Record<string, unknown>,
    allValues: Record<string, unknown>,
  ) => {
    if (!('defaultProvider' in changedValues) && !('defaultModel' in changedValues)) {
      return;
    }
    if (!runtimeConfig) {
      return;
    }

    const nextProvider = (allValues.defaultProvider ?? '') as string;
    const nextModel = (allValues.defaultModel ?? '') as string;
    const currentSettings = runtimeConfig.modelSettings;
    if (
      (currentSettings.defaultProvider ?? '') === nextProvider
      && (currentSettings.defaultModel ?? '') === nextModel
    ) {
      return;
    }

    const saveSeq = modelSettingsSaveSeqRef.current + 1;
    modelSettingsSaveSeqRef.current = saveSeq;
    setSaving(true);
    try {
      const nextConfig = await saveHermesModelSettings({
        defaultProvider: nextProvider,
        defaultModel: nextModel,
      });
      if (modelSettingsSaveSeqRef.current === saveSeq) {
        setRuntimeConfig(nextConfig);
        setOtherSettings(nextConfig.otherSettings || {});
      }
      await refreshTrayMenu();
    } catch (error) {
      console.error('Failed to save Hermes model settings:', error);
      if (modelSettingsSaveSeqRef.current === saveSeq) {
        message.error(t('common.error'));
      }
    } finally {
      if (modelSettingsSaveSeqRef.current === saveSeq) {
        setSaving(false);
      }
    }
  };

  // Initialize the inlined provider edit/save modal whenever it opens.
  React.useEffect(() => {
    if (!providerModal) {
      return;
    }
    const nextProviderJson = providerModal.provider?.provider
      ? asRecord(providerModal.provider.provider)
      : {};
    setProviderJson(nextProviderJson);
    setProviderJsonValid(true);
    setProviderAdvancedExpanded(false);
    providerModalForm.setFieldsValue({
      providerKey: providerModal.provider?.providerKey,
      apiMode: providerModal.provider?.apiMode || getStringField(nextProviderJson, 'api_mode'),
      baseUrl: getStringField(nextProviderJson, 'base_url')
        || getStringField(nextProviderJson, 'baseUrl'),
      providerApiKey: getStringField(nextProviderJson, 'api_key')
        || getStringField(nextProviderJson, 'apiKey'),
    });
  }, [providerModal, providerModalForm]);

  const handleSaveProviderModal = async () => {
    if (!providerJsonValid) {
      message.error(t('hermes.invalidJson', { defaultValue: 'Invalid JSON.' }));
      return;
    }
    const values = await providerModalForm.validateFields();
    const providerKey = values.providerKey?.trim();
    if (!providerKey) {
      message.error(t('hermes.provider.providerKeyRequired', { defaultValue: 'Provider name is required.' }));
      return;
    }

    const nextProviderJson = { ...providerJson };
    setOptionalStringField(nextProviderJson, 'api_mode', values.apiMode);
    setOptionalStringField(nextProviderJson, 'base_url', values.baseUrl);
    setOptionalStringField(nextProviderJson, 'api_key', values.providerApiKey);
    await handleSaveProvider({ providerKey, provider: nextProviderJson });
  };

  const handleSaveProvider = async (value: { providerKey: string; provider: Record<string, unknown> }) => {
    setSaving(true);
    try {
      const nextConfig = await saveHermesModelsProvider({
        providerKey: value.providerKey,
        provider: value.provider,
      });
      setRuntimeConfig(nextConfig);
      setOtherSettings(nextConfig.otherSettings || {});
      setProviderModal(null);
      await refreshTrayMenu();
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to save Hermes provider:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteProvider = (provider: HermesRuntimeProviderView) => {
    Modal.confirm({
      title: t('hermes.provider.deleteConfirmTitle', { defaultValue: 'Delete provider?' }),
      content: t('hermes.provider.deleteConfirmContent', {
        defaultValue: 'Remove "{{name}}" from custom_providers in config.yaml?',
        name: provider.displayName,
      }),
      okButtonProps: { danger: true },
      onOk: async () => {
        setSaving(true);
        try {
          const nextConfig = await deleteHermesRuntimeProvider(provider.providerKey);
          setRuntimeConfig(nextConfig);
          setOtherSettings(nextConfig.otherSettings || {});
          await refreshTrayMenu();
          message.success(t('common.success'));
          // The remove only ever touches custom_providers. If the same key is a
          // read-only Hermes built-in (providers: dict), a card legitimately
          // remains — make that clear so users don't think the delete failed.
          const readOnlyRemaining = nextConfig.providers.find(
            (item) => item.providerKey === provider.providerKey && item.isReadOnly,
          );
          if (readOnlyRemaining) {
            message.info(
              t('hermes.provider.readOnlyRemains', {
                name: readOnlyRemaining.displayName || readOnlyRemaining.providerKey,
              }),
            );
          }
        } catch (error) {
          console.error('Failed to delete Hermes provider:', error);
          message.error(t('common.error'));
        } finally {
          setSaving(false);
        }
      },
    });
  };

  const handleSaveModel = async (values: ModelFormValues) => {
    if (!modelModal) {
      return;
    }
    const modelId = values.id?.trim();
    if (!modelId) {
      message.error(t('hermes.model.idRequired', { defaultValue: 'Model ID is required.' }));
      return;
    }

    const currentProvider = runtimeConfig?.providers.find(
      (provider) => provider.providerKey === modelModal.provider.providerKey,
    ) ?? modelModal.provider;
    const existingModels = getProviderModelRecords(currentProvider.provider);
    const duplicateModel = existingModels.some((entry) => (
      entry.id === modelId && entry.id !== modelModal.modelId
    ));
    if (duplicateModel) {
      message.error(t('hermes.model.idExists', { defaultValue: 'Model ID already exists.' }));
      return;
    }

    const nextModel = { ...(existingModels.find((entry) => entry.id === modelModal.modelId)?.model ?? {}) };
    setOptionalStringField(nextModel, 'id', modelId);
    setOptionalStringField(nextModel, 'name', values.name);
    if (typeof values.contextLimit === 'number') {
      nextModel.context_length = values.contextLimit;
    } else {
      delete nextModel.context_length;
    }
    if (typeof values.outputLimit === 'number') {
      nextModel.max_tokens = values.outputLimit;
    } else {
      delete nextModel.max_tokens;
    }
    if (typeof values.reasoning === 'boolean') {
      nextModel.reasoning = values.reasoning;
    } else {
      delete nextModel.reasoning;
    }

    let modelWasReplaced = false;
    const nextModels = existingModels.map((entry) => {
      if (entry.id === modelModal.modelId) {
        modelWasReplaced = true;
        return nextModel;
      }
      return entry.model;
    });
    if (!modelWasReplaced) {
      nextModels.push(nextModel);
    }

    setSaving(true);
    try {
      const nextProviderConfig = {
        ...(currentProvider.provider ?? {}),
        models: nextModels,
      };
      const nextConfig = await saveHermesModelsProvider({
        providerKey: currentProvider.providerKey,
        provider: nextProviderConfig,
      });
      setRuntimeConfig(nextConfig);
      setOtherSettings(nextConfig.otherSettings || {});
      setModelModal(null);
      await refreshTrayMenu();
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to save Hermes model:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  const handleSetDefaultModel = async (provider: HermesRuntimeProviderView, modelId: string) => {
    setSaving(true);
    try {
      const nextConfig = await saveHermesModelSettings({
        defaultProvider: provider.providerKey,
        defaultModel: modelId,
      });
      setRuntimeConfig(nextConfig);
      setOtherSettings(nextConfig.otherSettings || {});
      modelForm.setFieldsValue({
        defaultProvider: provider.providerKey,
        defaultModel: modelId,
      });
      await refreshTrayMenu();
      message.success(t('hermes.model.setAsDefaultSuccess', { defaultValue: 'Set as default.' }));
    } catch (error) {
      console.error('Failed to set Hermes default model:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  const handleOpenConnectivityTest = (providerKey: string) => {
    setConnectivityProviderId(providerKey);
    setConnectivityModalOpen(true);
  };

  const handleRemoveConnectivityModels = React.useCallback(async (modelIdsToRemove: string[]) => {
    if (!connectivityProviderId || modelIdsToRemove.length === 0) {
      return;
    }
    const provider = hermesProviders.find((item) => item.providerKey === connectivityProviderId);
    if (!provider || provider.isReadOnly) {
      return;
    }
    const selectedModelIdSet = new Set(modelIdsToRemove);
    const nextModels = getProviderModelRecords(provider.provider)
      .filter((entry) => !selectedModelIdSet.has(entry.id))
      .map((entry) => entry.model);

    setSaving(true);
    try {
      const nextProviderConfig = { ...(provider.provider ?? {}), models: nextModels };
      const nextConfig = await saveHermesModelsProvider({
        providerKey: provider.providerKey,
        provider: nextProviderConfig,
      });
      setRuntimeConfig(nextConfig);
      setOtherSettings(nextConfig.otherSettings || {});
    } catch (error) {
      console.error('Failed to remove Hermes models from connectivity test:', error);
      throw error;
    } finally {
      setSaving(false);
    }
  }, [connectivityProviderId, hermesProviders]);

  const handleBatchTestProviders = React.useCallback(async () => {
    const targets = hermesProviders.map((provider) => (
      buildProviderConnectivityBatchTarget(
        {
          providerId: provider.providerKey,
          providerName: provider.displayName,
          providerConfig: buildHermesConnectivityProvider(provider),
          modelIds: provider.modelIds ?? [],
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
      )
    ));

    setConnectivityStatuses(
      Object.fromEntries(hermesProviders.map((provider) => [
        provider.providerKey,
        { status: 'running' as const },
      ])),
    );
    setBatchTestingProviders(true);

    try {
      await runProviderConnectivityBatch(targets, (providerKey, status) => {
        const nextStatus = status.status === 'success'
          ? {
              ...status,
              tooltipMessage: status.totalMs !== undefined
                ? t('common.connectivityBatchSuccessWithTiming', {
                    model: status.modelId || t('common.notSet'),
                    totalMs: status.totalMs,
                  })
                : t('common.connectivityBatchSuccess', {
                    model: status.modelId || t('common.notSet'),
                  }),
            }
          : status;
        setConnectivityStatuses((previousStatuses) => ({
          ...previousStatuses,
          [providerKey]: nextStatus,
        }));
      });
    } catch (error) {
      console.error('Failed to batch test Hermes providers:', error);
      message.error(t('common.error'));
    } finally {
      setBatchTestingProviders(false);
    }
  }, [hermesProviders, t]);

  const handleOtherSettingsBlur = async (value: unknown, isValid: boolean) => {
    if (!isValid || !otherSettingsValid) {
      message.error(t('hermes.invalidJson', { defaultValue: 'Invalid JSON.' }));
      return;
    }
    const nextOtherSettings = value && typeof value === 'object' && !Array.isArray(value)
      ? value as Record<string, unknown>
      : {};
    setSaving(true);
    try {
      const nextConfig = await saveHermesOtherSettings(nextOtherSettings);
      setRuntimeConfig(nextConfig);
      setOtherSettings(nextConfig.otherSettings || {});
      await refreshTrayMenu();
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to save Hermes other settings:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  const handleOpenRootFolder = async () => {
    if (runtimeConfig?.rootPathInfo.path) {
      await revealItemInDir(runtimeConfig.rootPathInfo.path);
    }
  };

  const handleRefreshConfig = () => {
    void loadConfig(true);
    void refreshTrayMenu();
  };

  const handleOpenWebUi = async () => {
    try {
      await openHermesWebUi();
    } catch {
      Modal.confirm({
        title: t('hermes.openWebUi', { defaultValue: 'Open Web UI' }),
        content: t('hermes.openWebUiOffline', {
          defaultValue: 'Hermes Web UI is not running. Launch the dashboard service?',
        }),
        okText: t('hermes.launchDashboard', { defaultValue: 'Launch Dashboard' }),
        onOk: async () => {
          try {
            await launchHermesDashboard();
            message.success(
              t('hermes.dashboardLaunched', {
                defaultValue: 'Hermes dashboard launched — retry "Open Web UI" shortly.',
              })
            );
          } catch {
            message.error(t('common.error'));
          }
        },
      });
    }
  };

  // Derived values for the inlined shared ModelFormModal (mirrors the old HermesModelModal).
  const modelModalProvider = modelModal?.provider ?? hermesProviders[0];
  const modelModalRecord = modelModal?.modelId
    ? getProviderModelRecords(modelModalProvider?.provider).find(
      (entry) => entry.id === modelModal.modelId,
    )?.model
    : undefined;
  const modelModalExistingIds = getProviderModelRecords(modelModalProvider?.provider).map((entry) => entry.id);
  const modelModalRecordSafe = asRecord(modelModalRecord);
  const modelModalInitialValues = {
    id: modelModal?.modelId ?? getStringField(modelModalRecordSafe, 'id'),
    name: getStringField(modelModalRecordSafe, 'name'),
    reasoning: typeof modelModalRecordSafe.reasoning === 'boolean' ? modelModalRecordSafe.reasoning : undefined,
    contextLimit: getNumberField(modelModalRecordSafe, 'context_length'),
    outputLimit: getNumberField(modelModalRecordSafe, 'max_tokens'),
  };

  const renderProvider = (provider: HermesRuntimeProviderView) => {
    const providerConfig = asRecord(provider.provider);
    const credentialPreview = maskCredential(provider.credential);
    const baseUrl = getStringField(providerConfig, 'base_url')
      || getStringField(providerConfig, 'baseUrl');
    const hasModelIds = (provider.modelIds?.length ?? 0) > 0;
    const connectivityTooltip = !baseUrl
      ? t('common.baseUrlMissing')
      : !hasModelIds
        ? t('common.modelMissing')
        : '';
    const isReadOnly = provider.isReadOnly;

    const providerDisplay: ProviderDisplayData = {
      id: provider.providerKey,
      name: provider.displayName,
      sdkName: provider.apiMode || 'hermes',
      baseUrl: baseUrl || credentialPreview || `${provider.providerKey} (${t('hermes.provider.readOnly', { defaultValue: 'read-only' })})`,
    };
    const modelDisplayList: ModelDisplayData[] = getProviderModelRecords(provider.provider).map((entry) => ({
      id: entry.id,
      name: getStringField(entry.model, 'name') || entry.id,
      isPrimary: provider.isDefault && runtimeConfig?.modelSettings.defaultModel === entry.id,
    }));

    return (
      <ProviderCard
        key={provider.providerKey}
        provider={providerDisplay}
        models={modelDisplayList}
        onEdit={isReadOnly ? undefined : () => setProviderModal({ provider })}
        onCopy={isReadOnly ? undefined : () => setProviderModal({ provider: undefined })}
        onDelete={isReadOnly ? undefined : () => handleDeleteProvider(provider)}
        deleteConfirm={false}
        connectivityStatus={connectivityStatuses[provider.providerKey]}
        extraActions={
          <Tooltip title={connectivityTooltip}>
            <span>
              <Button
                size="small"
                type="text"
                style={{ fontSize: 12 }}
                onClick={() => handleOpenConnectivityTest(provider.providerKey)}
                disabled={!baseUrl || !hasModelIds}
              >
                <ApiOutlined style={{ marginRight: 4 }} />
                {t('hermes.connectivity.button', { defaultValue: 'Test' })}
              </Button>
            </span>
          </Tooltip>
        }
        onAddModel={isReadOnly ? undefined : () => setModelModal({ provider })}
        onEditModel={isReadOnly ? undefined : (modelId) => setModelModal({ provider, modelId })}
        onDeleteModel={undefined}
        onSetPrimaryModel={isReadOnly ? undefined : (modelId) => handleSetDefaultModel(provider, modelId)}
        i18nPrefix="pi"
      />
    );
  };

  return (
    <Spin spinning={loading}>
      <SectionSidebarLayout
        sidebarTitle={t('hermes.title', { defaultValue: 'Hermes' })}
        sidebarHidden={sidebarHidden}
        sections={sidebarSections}
        markerAttr="data-hermes-sidebar-section"
        getIcon={(id) => SIDEBAR_ICON_BY_SECTION_ID[id] ?? null}
      >
        <div className={styles.pageContent}>
          <div className={styles.pageHeader}>
            <div>
              <div className={styles.titleRow}>
                <Title level={4} className={styles.pageTitle}>
                  {t('hermes.title', { defaultValue: 'Hermes' })}
                </Title>
                <Link
                  type="secondary"
                  className={styles.headerLink}
                  onClick={(event) => {
                    event.stopPropagation();
                    setPreviewModalOpen(true);
                  }}
                >
                  <EyeOutlined /> {t('common.previewConfig')}
                </Link>
              </div>
              <Space className={styles.pathToolbar} wrap>
                <Text type="secondary" className={styles.pathLabel}>
                  {t('hermes.configPath', { defaultValue: 'Config path' })}:
                </Text>
                <Text code className={styles.pathText}>
                  {runtimeConfig?.rootPathInfo.path}
                </Text>
                <Button
                  type="text"
                  size="small"
                  icon={<EditOutlined />}
                  onClick={() => setRootDirectoryModalOpen(true)}
                  className={styles.textAction}
                >
                  {t('hermes.rootPathSource.customize', { defaultValue: 'Customize' })}
                </Button>
                <Button
                  type="text"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  onClick={handleOpenRootFolder}
                  className={styles.textAction}
                >
                  {t('hermes.openFolder', { defaultValue: 'Open folder' })}
                </Button>
                <Button
                  type="text"
                  size="small"
                  icon={<ReloadOutlined />}
                  onClick={handleRefreshConfig}
                  className={styles.textAction}
                >
                  {t('hermes.refreshConfig', { defaultValue: 'Refresh' })}
                </Button>
                <Button
                  type="text"
                  size="small"
                  icon={<GlobalOutlined />}
                  onClick={handleOpenWebUi}
                  className={styles.textAction}
                >
                  {t('hermes.openWebUi', { defaultValue: 'Open Web UI' })}
                </Button>
              </Space>
            </div>
            <Button type="text" icon={<EllipsisOutlined />} onClick={() => setSettingsModalOpen(true)}>
              {t('common.moreOptions')}
            </Button>
          </div>
          <div className={styles.pageHint}>
            {t('hermes.pageHint', {
              defaultValue: 'Hermes reads a single config.yaml; provider facts come from the runtime file. Built-in (read-only) providers are managed by the providers: dict.',
            })}
          </div>

          <div
            id="hermes-model-settings"
            className={styles.hermesSection}
            data-hermes-sidebar-section="true"
            data-sidebar-title={t('hermes.modelSettings.title', { defaultValue: 'Model Settings' })}
          >
            <div className={styles.modelCard}>
              <Title level={5} className={styles.modelCardTitle}>
                <RobotOutlined style={{ marginRight: 8 }} />
                {t('hermes.modelSettings.title', { defaultValue: 'Model Settings' })}
              </Title>
              <div className={styles.modelCardContent}>
                <Form
                  form={modelForm}
                  layout="vertical"
                  onValuesChange={handleModelSettingsChange}
                >
                  <div className={styles.modelSettingsGrid}>
                    <Form.Item label={t('hermes.modelSettings.defaultProvider', { defaultValue: 'Default provider' })} name="defaultProvider">
                      <Select
                        allowClear
                        showSearch
                        options={providerOptions}
                        placeholder={t('hermes.modelSettings.defaultProviderPlaceholder', { defaultValue: 'Select a provider' })}
                      />
                    </Form.Item>
                    <Form.Item label={t('hermes.modelSettings.defaultModel', { defaultValue: 'Default model' })} name="defaultModel">
                      <Select
                        allowClear
                        showSearch
                        options={modelOptions}
                        placeholder={t('hermes.modelSettings.defaultModelPlaceholder', { defaultValue: 'Select a model' })}
                      />
                    </Form.Item>
                  </div>
                </Form>
              </div>
            </div>
          </div>

          <div
            id="hermes-providers"
            className={styles.hermesSection}
            data-hermes-sidebar-section="true"
            data-sidebar-title={t('hermes.provider.title', { defaultValue: 'Providers' })}
          >
            <Collapse
              className={styles.collapseCard}
              items={[
                {
                  key: 'providers',
                  label: (
                    <Space>
                      <ApiOutlined />
                      <Text strong>{t('hermes.provider.title', { defaultValue: 'Providers' })}</Text>
                    </Space>
                  ),
                  extra: (
                    <Space onClick={(event) => event.stopPropagation()}>
                      <Button
                        type="link"
                        size="small"
                        icon={<ThunderboltOutlined />}
                        loading={batchTestingProviders}
                        onClick={handleBatchTestProviders}
                      >
                        {t('common.batchTest')}
                      </Button>
                      <Button
                        type="link"
                        size="small"
                        icon={<PlusOutlined />}
                        onClick={() => setProviderModal({})}
                      >
                        {t('hermes.provider.addSupplier', { defaultValue: 'Add provider' })}
                      </Button>
                    </Space>
                  ),
                  children: runtimeConfig?.providers.length
                    ? (
                        <div className={styles.providerList}>
                          {runtimeConfig.providers.map(renderProvider)}
                        </div>
                      )
                    : <Empty description={t('hermes.provider.emptyText', { defaultValue: 'No providers configured.' })} />,
                },
              ]}
            />
          </div>

          <div
            id="hermes-global-prompt"
            className={`${styles.hermesSection} ${styles.promptSection}`}
            data-hermes-sidebar-section="true"
            data-sidebar-title={t('hermes.prompt.title', { defaultValue: 'Global Prompt' })}
          >
            <GlobalPromptSettings
              translationKeyPrefix="hermes.prompt"
              service={hermesPromptApi}
              collapseKey="hermes-prompt"
              onUpdated={async () => {
                await loadConfig(true);
                await refreshTrayMenu();
              }}
            />
          </div>

          <div
            id="hermes-memory"
            className={styles.hermesSection}
            data-hermes-sidebar-section="true"
            data-sidebar-title={t('hermes.memory.title', { defaultValue: 'Memory' })}
          >
            <HermesMemoryPanel />
          </div>

          <div
            id="hermes-other-configuration"
            className={styles.hermesSection}
            data-hermes-sidebar-section="true"
            data-sidebar-title={t('hermes.otherConfig.title', { defaultValue: 'Other Configuration' })}
          >
            <Collapse
              style={{ marginBottom: 0 }}
              items={[
                {
                  key: 'other',
                  label: (
                    <Space>
                      <SettingOutlined />
                      <Text strong>
                        {t('hermes.otherConfig.title', { defaultValue: 'Other Configuration' })}
                      </Text>
                    </Space>
                  ),
                  children: (
                    <Form.Item
                      help={
                        <span>
                          <Text type="secondary">
                            {t('hermes.otherConfig.hint', {
                              defaultValue: 'Top-level config.yaml keys not managed by this page (agent, etc.).',
                            })}
                          </Text>
                          ，<span style={{ color: 'var(--ant-color-primary)' }}>{t('hermes.otherConfig.autoSaveHint', { defaultValue: 'Auto-saves on blur. Keys removed in the editor are kept on disk.' })}</span>
                        </span>
                      }
                      style={{ marginBottom: 0 }}
                    >
                      <JsonEditor
                        value={otherSettings}
                        height={260}
                        onChange={(nextValue, nextIsValid) => {
                          setOtherSettings(
                            nextValue && typeof nextValue === 'object' && !Array.isArray(nextValue)
                              ? nextValue as Record<string, unknown>
                              : {},
                          );
                          setOtherSettingsValid(nextIsValid);
                        }}
                        onBlur={handleOtherSettingsBlur}
                      />
                    </Form.Item>
                  ),
                },
              ]}
            />
          </div>

          <div
            id="hermes-session-manager"
            className={styles.hermesSection}
            data-hermes-sidebar-section="true"
            data-sidebar-title={t('sessionManager.title')}
          >
            <SessionManagerPanel tool="hermes" />
          </div>
        </div>

        <RootDirectoryModal
          open={rootDirectoryModalOpen}
          {...getRootDirectoryModalProps(runtimeConfig?.rootPathInfo || null)}
          onCancel={() => setRootDirectoryModalOpen(false)}
          onSubmit={handleSaveRootDirectory}
          onReset={handleResetRootDirectory}
        />

        <Modal
          title={providerModal?.provider
            ? `${t('hermes.provider.editTitle', { defaultValue: 'Edit Provider' })}: ${providerModal.provider.displayName}`
            : t('hermes.provider.addTitle', { defaultValue: 'Add Provider' })}
          open={!!providerModal}
          width={720}
          confirmLoading={saving}
          onCancel={() => setProviderModal(null)}
          onOk={handleSaveProviderModal}
          destroyOnHidden
        >
          <Form form={providerModalForm} layout="vertical" className={styles.providerForm}>
            <div className={styles.modalSection}>
              <Text strong>{t('hermes.provider.basicSection', { defaultValue: 'Basic' })}</Text>
              <div className={styles.modalGrid}>
                <Form.Item
                  label={t('hermes.provider.providerKey', { defaultValue: 'Provider name' })}
                  name="providerKey"
                  rules={[{ required: true, message: t('hermes.provider.providerKeyRequired', { defaultValue: 'Provider name is required.' }) }]}
                >
                  <Input
                    disabled={!!providerModal?.provider}
                    placeholder={t('hermes.provider.providerKeyPlaceholder', { defaultValue: 'e.g. anthropic' })}
                  />
                </Form.Item>
                <Form.Item label={t('hermes.provider.apiMode', { defaultValue: 'API mode' })} name="apiMode">
                  <Select
                    allowClear
                    showSearch
                    options={HERMES_API_MODE_OPTIONS}
                    placeholder={t('hermes.provider.apiModePlaceholder', { defaultValue: 'anthropic / openai / ...' })}
                  />
                </Form.Item>
                <Form.Item label={t('hermes.provider.baseUrl', { defaultValue: 'Base URL' })} name="baseUrl">
                  <Input placeholder="https://api.anthropic.com" />
                </Form.Item>
                <Form.Item
                  label={t('hermes.provider.apiKey', { defaultValue: 'API key' })}
                  name="providerApiKey"
                >
                  <Input.Password autoComplete="off" />
                </Form.Item>
              </div>
            </div>

            <div className={styles.advancedToggle}>
              <Button
                type="link"
                onClick={() => setProviderAdvancedExpanded(!providerAdvancedExpanded)}
                className={styles.advancedToggleButton}
              >
                {providerAdvancedExpanded ? <DownOutlined /> : <RightOutlined />}
                <span>{t('common.advancedSettings', { defaultValue: 'Advanced settings' })}</span>
                <Tooltip title={t('hermes.provider.advancedHint', { defaultValue: 'Full provider config that is written to config.yaml (name is set from the provider name).' })}>
                  <QuestionCircleOutlined style={{ color: 'var(--color-text-tertiary)' }} />
                </Tooltip>
              </Button>
            </div>
            {providerAdvancedExpanded && (
              <div className={styles.modalSection}>
                <div className={styles.advancedEditor}>
                  <Text type="secondary">
                    <FileTextOutlined style={{ marginRight: 4 }} />
                    {t('hermes.provider.configJson', { defaultValue: 'Provider config (JSON)' })}
                  </Text>
                  <JsonEditor
                    value={isRecordEmpty(providerJson) ? undefined : providerJson}
                    height={240}
                    mode="text"
                    onChange={(value, isValid) => {
                      if (isValid) {
                        setProviderJson(asRecord(value));
                      }
                      setProviderJsonValid(isValid);
                    }}
                  />
                </div>
              </div>
            )}
          </Form>
        </Modal>

        <ModelFormModal
          open={!!modelModal}
          width={560}
          isEdit={!!modelModal?.modelId}
          initialValues={modelModalInitialValues}
          existingIds={modelModal?.modelId ? [] : modelModalExistingIds}
          showOptions={false}
          showVariants={false}
          showModalities={false}
          showInputTypes={false}
          showApi={false}
          showReasoning
          showThinkingLevelMap={false}
          showOmpThinking={false}
          showCompat={false}
          showCost={false}
          limitRequired={false}
          nameRequired={false}
          onCancel={() => setModelModal(null)}
          onSuccess={handleSaveModel}
          onDuplicateId={() => message.error(t('hermes.model.idExists', { defaultValue: 'Model ID already exists.' }))}
          i18nPrefix="pi"
        />

        <ProviderConnectivityTestModal
          open={connectivityModalOpen}
          connectivityInfo={connectivityInfo}
          removableModelIds={connectivityInfo?.modelIds}
          onRemoveModels={handleRemoveConnectivityModels}
          onCancel={() => setConnectivityModalOpen(false)}
        />

        <JsonPreviewModal
          open={previewModalOpen}
          onClose={() => setPreviewModalOpen(false)}
          title={t('hermes.preview.title', { defaultValue: 'Preview config' })}
          data={runtimeConfig}
        />

        <SidebarSettingsModal
          open={settingsModalOpen}
          onClose={() => setSettingsModalOpen(false)}
          sidebarVisible={!sidebarHidden}
          onSidebarVisibleChange={async (visible) => {
            await setSidebarHidden('hermes', !visible);
          }}
        />
      </SectionSidebarLayout>
    </Spin>
  );
};

export default HermesPage;
