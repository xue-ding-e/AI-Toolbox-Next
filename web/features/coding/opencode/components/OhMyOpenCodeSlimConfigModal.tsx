import React from 'react';
import { Modal, Form, Input, Button, Typography, Collapse, Select, message, Divider } from 'antd';
import { PlusOutlined, DeleteOutlined, SwapOutlined, ImportOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { SLIM_AGENT_TYPES, getSlimAgentDescriptionKey, getSlimAgentDisplayNameKey, type OhMyOpenCodeSlimAgents, type SlimAgentType } from '@/types/ohMyOpenCodeSlim';
import JsonEditor from '@/components/common/JsonEditor';
import ImportJsonConfigModal, { type ImportedConfigData } from './ImportJsonConfigModal';
import OhMyOpenCodeSlimCouncilForm, { buildSlimCouncilConfig, parseSlimCouncilFormValues } from './OhMyOpenCodeSlimCouncilForm';
import { buildSlimAgentsFromFormValues } from './ohMyOpenCodeSlimFormUtils';
import styles from './OhMyOpenCodeSlimConfigModal.module.less';

const { Text } = Typography;

interface OhMyOpenCodeSlimConfigModalProps {
  open: boolean;
  isEdit: boolean;
  initialValues?: {
    id?: string;
    name: string;
    agents?: OhMyOpenCodeSlimAgents;
    council?: Record<string, unknown> | null;
    otherFields?: Record<string, unknown>;
  };
  modelOptions: Array<
    | { label: string; value: string; disabled?: boolean }
    | { label: string; options: { label: string; value: string; disabled?: boolean }[] }
  >;
  /** Map of model ID to its variant keys */
  modelVariantsMap?: Record<string, string[]>;
  onCancel: () => void;
  onSuccess: (values: OhMyOpenCodeSlimConfigFormValues) => void;
}

export interface OhMyOpenCodeSlimConfigFormValues {
  id?: string;
  name: string;
  agents: OhMyOpenCodeSlimAgents;
  council?: Record<string, unknown>;
  otherFields?: Record<string, unknown>;
}

const OhMyOpenCodeSlimConfigModal: React.FC<OhMyOpenCodeSlimConfigModalProps> = ({
  open,
  isEdit,
  initialValues,
  modelOptions,
  modelVariantsMap = {},
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);

  // Custom agents (user-defined)
  const [customAgents, setCustomAgents] = React.useState<string[]>([]);
  const [newAgentKey, setNewAgentKey] = React.useState('');
  const [showAddAgent, setShowAddAgent] = React.useState(false);
  const [showBatchReplace, setShowBatchReplace] = React.useState(false);
  const [showImportJson, setShowImportJson] = React.useState(false);

  // Batch replace model state
  const [batchReplaceFromModel, setBatchReplaceFromModel] = React.useState<string | undefined>(undefined);
  const [batchReplaceToModel, setBatchReplaceToModel] = React.useState<string | undefined>(undefined);
  const [batchReplaceFromVariant, setBatchReplaceFromVariant] = React.useState<string | undefined>(undefined);
  const [batchReplaceToVariant, setBatchReplaceToVariant] = React.useState<string | undefined>(undefined);

  const fromModelVariants = React.useMemo(
    () => (batchReplaceFromModel ? modelVariantsMap[batchReplaceFromModel] ?? [] : []),
    [batchReplaceFromModel, modelVariantsMap]
  );

  const toModelVariants = React.useMemo(
    () => (batchReplaceToModel ? modelVariantsMap[batchReplaceToModel] ?? [] : []),
    [batchReplaceToModel, modelVariantsMap]
  );

  // Store otherFields - keep both raw string and parsed value for submit-time validation
  const otherFieldsRef = React.useRef<Record<string, unknown>>({});
  const otherFieldsRawRef = React.useRef<string>('');
  const councilOtherFieldsValidRef = React.useRef(true);

  // Track if modal has been initialized
  const initializedRef = React.useRef(false);

  const labelCol = 6;
  const wrapperCol = 18;

  // Built-in agent keys
  const builtInAgentKeys = React.useMemo(() => [...SLIM_AGENT_TYPES], []);

  // Initialize form values when modal opens
  React.useEffect(() => {
    if (!open) {
      initializedRef.current = false;
      return;
    }

    if (initializedRef.current) {
      return;
    }

    initializedRef.current = true;

    // Always reset form and refs first to prevent stale values from previous edits
    form.resetFields();
    otherFieldsRef.current = {};
    otherFieldsRawRef.current = '';

    if (initialValues) {
      const councilFormValues = parseSlimCouncilFormValues(initialValues.council ?? null);
      // Build form values with nested agent paths
      const formValues: Record<string, unknown> = {
        id: initialValues.id,
        name: initialValues.name,
        ...councilFormValues,
      };

      const detectedCustomAgents: string[] = [];
      const builtInAgentKeySet = new Set<string>(builtInAgentKeys);

      // Set agent models (built-in + custom)
      if (initialValues.agents) {
        Object.entries(initialValues.agents).forEach(([agentType, agent]) => {
          if (agent?.model) {
            formValues[`agent_${agentType}_model`] = agent.model;
          }
          if (typeof agent?.variant === 'string' && agent.variant) {
            formValues[`agent_${agentType}_variant`] = agent.variant;
          }

          // Track custom agents
          if (!builtInAgentKeySet.has(agentType)) {
            detectedCustomAgents.push(agentType);
          }
        });
      }

      setCustomAgents(detectedCustomAgents);

      form.setFieldsValue(formValues);
      otherFieldsRef.current = initialValues.otherFields || {};
      otherFieldsRawRef.current = initialValues.otherFields && Object.keys(initialValues.otherFields).length > 0
        ? JSON.stringify(initialValues.otherFields, null, 2)
        : '';
    } else {
      setCustomAgents([]);
    }

    setShowAddAgent(false);
    setShowBatchReplace(false);
    setShowImportJson(false);
    setNewAgentKey('');
    setBatchReplaceFromModel(undefined);
    setBatchReplaceToModel(undefined);
    setBatchReplaceFromVariant(undefined);
    setBatchReplaceToVariant(undefined);
    councilOtherFieldsValidRef.current = true;
  }, [open, initialValues, form, builtInAgentKeys]);

  React.useEffect(() => {
    if (!batchReplaceFromModel) {
      setBatchReplaceFromVariant(undefined);
      return;
    }
    if (batchReplaceFromVariant && !fromModelVariants.includes(batchReplaceFromVariant)) {
      setBatchReplaceFromVariant(undefined);
    }
  }, [batchReplaceFromModel, batchReplaceFromVariant, fromModelVariants]);

  React.useEffect(() => {
    if (!batchReplaceToModel) {
      setBatchReplaceToVariant(undefined);
      return;
    }
    if (batchReplaceToVariant && !toModelVariants.includes(batchReplaceToVariant)) {
      setBatchReplaceToVariant(undefined);
    }
  }, [batchReplaceToModel, batchReplaceToVariant, toModelVariants]);

  const handleBatchReplaceModel = () => {
    const fromModel = batchReplaceFromModel;
    const toModel = batchReplaceToModel;

    if (!fromModel || !toModel) {
      message.warning(t('opencode.ohMyOpenCode.batchReplaceRequired'));
      return;
    }

    const sourceVariants = modelVariantsMap[fromModel] ?? [];
    if (batchReplaceFromVariant && !sourceVariants.includes(batchReplaceFromVariant)) {
      message.warning(t('opencode.ohMyOpenCode.batchReplaceInvalidFromVariant'));
      return;
    }

    const targetVariants = modelVariantsMap[toModel] ?? [];
    if (batchReplaceToVariant && !targetVariants.includes(batchReplaceToVariant)) {
      message.warning(t('opencode.ohMyOpenCode.batchReplaceInvalidToVariant'));
      return;
    }

    if (fromModel === toModel) {
      message.warning(t('opencode.ohMyOpenCode.batchReplaceSameModel'));
      return;
    }

    const allAgentKeys = [...builtInAgentKeys, ...customAgents];
    const modelFieldNames = allAgentKeys.map((agentType) => `agent_${agentType}_model`);

    const values = form.getFieldsValue(true) as Record<string, unknown>;
    const updateValues: Record<string, unknown> = {};

    let replacedCount = 0;
    let clearedVariantCount = 0;
    const hasTargetVariants = targetVariants.length > 0;

    modelFieldNames.forEach((modelFieldName) => {
      if (values[modelFieldName] !== fromModel) {
        return;
      }

      const variantFieldName = modelFieldName.replace('_model', '_variant');
      const variantValue = values[variantFieldName];

      if (batchReplaceFromVariant) {
        if (typeof variantValue !== 'string' || variantValue !== batchReplaceFromVariant) {
          return;
        }
      }

      updateValues[modelFieldName] = toModel;
      replacedCount += 1;

      if (batchReplaceToVariant) {
        updateValues[variantFieldName] = batchReplaceToVariant;
        return;
      }

      if (typeof variantValue === 'string' && variantValue) {
        if (!hasTargetVariants || !targetVariants.includes(variantValue)) {
          updateValues[variantFieldName] = undefined;
          clearedVariantCount += 1;
        }
      }
    });

    if (replacedCount === 0) {
      message.warning(t('opencode.ohMyOpenCode.batchReplaceNoMatch'));
      return;
    }

    form.setFieldsValue(updateValues);

    if (clearedVariantCount > 0) {
      message.success(t('opencode.ohMyOpenCode.batchReplaceSuccessWithVariantReset', {
        count: replacedCount,
        variantCount: clearedVariantCount,
      }));
      return;
    }

    message.success(t('opencode.ohMyOpenCode.batchReplaceSuccess', { count: replacedCount }));
  };

  const handleImportJson = (data: ImportedConfigData, _mode: 'core' | 'full') => {
    const updateValues: Record<string, string | undefined> = {};
    const builtInAgentKeySet = new Set<string>(builtInAgentKeys);
    const newCustomAgents: string[] = [];
    let agentCount = 0;

    // Process agents
    if (data.agents) {
      Object.entries(data.agents).forEach(([agentType, agentConfig]) => {
        if (!agentConfig || typeof agentConfig !== 'object') return;

        // Detect custom agents
        if (!builtInAgentKeySet.has(agentType) && !customAgents.includes(agentType) && !newCustomAgents.includes(agentType)) {
          newCustomAgents.push(agentType);
        }

        // Set model field
        if (typeof agentConfig.model === 'string' && agentConfig.model) {
          updateValues[`agent_${agentType}_model`] = agentConfig.model;
        }

        // Set variant field
        if (typeof agentConfig.variant === 'string' && agentConfig.variant) {
          updateValues[`agent_${agentType}_variant`] = agentConfig.variant;
        }

        agentCount++;
      });
    }

    // Process otherFields
    if (data.otherFields && Object.keys(data.otherFields).length > 0) {
      const importedOtherFields = { ...data.otherFields };
      const importedCouncil = importedOtherFields.council;

      if (importedCouncil && typeof importedCouncil === 'object' && !Array.isArray(importedCouncil)) {
        form.setFieldsValue(parseSlimCouncilFormValues(importedCouncil as Record<string, unknown>));
        delete importedOtherFields.council;
      }

      otherFieldsRef.current = importedOtherFields;
      otherFieldsRawRef.current = Object.keys(importedOtherFields).length > 0
        ? JSON.stringify(importedOtherFields, null, 2)
        : '';
    }

    // Add custom agents
    if (newCustomAgents.length > 0) {
      setCustomAgents(prev => [...prev, ...newCustomAgents]);
    }

    // Apply form values
    form.setFieldsValue(updateValues);

    message.success(t('opencode.ohMyOpenCode.importFromJsonSuccess', { agentCount, categoryPart: '' }));
    setShowImportJson(false);
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      if (!councilOtherFieldsValidRef.current) {
        message.error(t('opencode.ohMyOpenCode.invalidJson'));
        setLoading(false);
        return;
      }

      // Validate otherFields JSON at submit time
      const rawContent = otherFieldsRawRef.current.trim();
      let parsedOtherFields: Record<string, unknown> = {};
      if (rawContent !== '') {
        try {
          parsedOtherFields = JSON.parse(rawContent);
          if (typeof parsedOtherFields !== 'object' || parsedOtherFields === null || Array.isArray(parsedOtherFields)) {
            message.error(t('opencode.ohMyOpenCode.invalidJson'));
            setLoading(false);
            return;
          }
        } catch {
          message.error(t('opencode.ohMyOpenCode.invalidJson'));
          setLoading(false);
          return;
        }
      }

      delete parsedOtherFields.council;

      const agents = buildSlimAgentsFromFormValues({
        builtInAgentKeys,
        customAgents,
        formValues: values as Record<string, unknown>,
        initialAgents: initialValues?.agents,
      });

      const result: OhMyOpenCodeSlimConfigFormValues = {
        name: values.name,
        agents,
        council: undefined,
        otherFields: Object.keys(parsedOtherFields).length > 0
          ? parsedOtherFields
          : undefined,
      };

      const councilBuildResult = buildSlimCouncilConfig(values as Record<string, unknown>, t);
      if (councilBuildResult.errorMessage) {
        message.error(councilBuildResult.errorMessage);
        setLoading(false);
        return;
      }
      result.council = councilBuildResult.council ?? undefined;

      // Include id when editing
      if (isEdit && values.id) {
        result.id = values.id;
      }

      onSuccess(result);
      form.resetFields();
    } catch (error) {
      console.error('Form validation error:', error);
    } finally {
      setLoading(false);
    }
  };

  // Handle adding custom agent
  const handleAddCustomAgent = () => {
    const key = newAgentKey.trim();
    if (!key) {
      message.warning(t('opencode.ohMyOpenCode.customAgentKeyRequired'));
      return;
    }
    // Check for duplicates
    const allKeys = [...builtInAgentKeys, ...customAgents];
    if (allKeys.includes(key)) {
      message.warning(t('opencode.ohMyOpenCode.customAgentKeyDuplicate'));
      return;
    }
    setCustomAgents(prev => [...prev, key]);
    setNewAgentKey('');
    setShowAddAgent(false);
  };

  // Handle removing custom agent
  const handleRemoveCustomAgent = (agentKey: string) => {
    setCustomAgents(prev => prev.filter(k => k !== agentKey));
    // Clear form fields
    form.setFieldValue(`agent_${agentKey}_model`, undefined);
    form.setFieldValue(`agent_${agentKey}_variant`, undefined);
  };

  // Render built-in agent item
  const renderBuiltInAgentItem = (agentType: SlimAgentType) => (
    <Form.Item
      key={agentType}
      label={t(getSlimAgentDisplayNameKey(agentType))}
      tooltip={t(getSlimAgentDescriptionKey(agentType))}
    >
      <Form.Item
        noStyle
        shouldUpdate={(prevValues, currentValues) =>
          prevValues[`agent_${agentType}_model`] !== currentValues[`agent_${agentType}_model`] ||
          prevValues[`agent_${agentType}_variant`] !== currentValues[`agent_${agentType}_variant`]
        }
      >
        {({ getFieldValue }) => {
          const selectedModel = getFieldValue(`agent_${agentType}_model`);
          const currentVariant = getFieldValue(`agent_${agentType}_variant`);
          const mapVariants = selectedModel ? modelVariantsMap[selectedModel] ?? [] : [];
          const hasVariants = mapVariants.length > 0 || (typeof currentVariant === 'string' && currentVariant);
          const variantOptions = [...mapVariants];
          if (typeof currentVariant === 'string' && currentVariant && !variantOptions.includes(currentVariant)) {
            variantOptions.unshift(currentVariant);
          }

          return (
            <div className={styles.compactFieldRow}>
              <Form.Item name={`agent_${agentType}_model`} noStyle>
                <Select
                  placeholder={t('opencode.ohMyOpenCode.selectModel')}
                  options={modelOptions}
                  allowClear
                  showSearch
                  optionFilterProp="label"
                  className={styles.compactModelSelect}
                  onChange={(newModel) => {
                    const newVariants = newModel ? modelVariantsMap[newModel] ?? [] : [];
                    if (newVariants.length === 0 || (currentVariant && !newVariants.includes(currentVariant))) {
                      form.setFieldValue(`agent_${agentType}_variant`, undefined);
                    }
                  }}
                />
              </Form.Item>
              {hasVariants && (
                <Form.Item name={`agent_${agentType}_variant`} noStyle>
                  <Select
                    placeholder="variant"
                    options={variantOptions.map((v) => ({ label: v, value: v }))}
                    allowClear
                    className={styles.variantSelect}
                  />
                </Form.Item>
              )}
            </div>
          );
        }}
      </Form.Item>
    </Form.Item>
  );

  // Render custom agent item (with delete button)
  const renderCustomAgentItem = (agentType: string) => (
    <Form.Item
      key={agentType}
      label={<span className={styles.customAgentLabel}>{agentType}</span>}
      tooltip={t('opencode.ohMyOpenCode.customAgentTooltip')}
    >
      <Form.Item
        noStyle
        shouldUpdate={(prevValues, currentValues) =>
          prevValues[`agent_${agentType}_model`] !== currentValues[`agent_${agentType}_model`] ||
          prevValues[`agent_${agentType}_variant`] !== currentValues[`agent_${agentType}_variant`]
        }
      >
        {({ getFieldValue }) => {
          const selectedModel = getFieldValue(`agent_${agentType}_model`);
          const currentVariant = getFieldValue(`agent_${agentType}_variant`);
          const mapVariants = selectedModel ? modelVariantsMap[selectedModel] ?? [] : [];
          const hasVariants = mapVariants.length > 0 || (typeof currentVariant === 'string' && currentVariant);
          const variantOptions = [...mapVariants];
          if (typeof currentVariant === 'string' && currentVariant && !variantOptions.includes(currentVariant)) {
            variantOptions.unshift(currentVariant);
          }

          return (
            <div className={styles.compactFieldRow}>
              <Form.Item name={`agent_${agentType}_model`} noStyle>
                <Select
                  placeholder={t('opencode.ohMyOpenCode.selectModel')}
                  options={modelOptions}
                  allowClear
                  showSearch
                  optionFilterProp="label"
                  className={styles.compactModelSelect}
                  onChange={(newModel) => {
                    const newVariants = newModel ? modelVariantsMap[newModel] ?? [] : [];
                    if (newVariants.length === 0 || (currentVariant && !newVariants.includes(currentVariant))) {
                      form.setFieldValue(`agent_${agentType}_variant`, undefined);
                    }
                  }}
                />
              </Form.Item>
              {hasVariants && (
                <Form.Item name={`agent_${agentType}_variant`} noStyle>
                  <Select
                    placeholder="variant"
                    options={variantOptions.map((v) => ({ label: v, value: v }))}
                    allowClear
                    className={styles.variantSelect}
                  />
                </Form.Item>
              )}
              <Button
                icon={<DeleteOutlined />}
                onClick={() => handleRemoveCustomAgent(agentType)}
                danger
                title={t('common.delete')}
                className={styles.removeButton}
              />
            </div>
          );
        }}
      </Form.Item>
    </Form.Item>
  );

  const agentsSectionLabel = (
    <div className={styles.sectionLabel}>
      <div className={styles.sectionLabelMain}>
        <span className={styles.sectionTitle}>{t('opencode.ohMyOpenCode.agentModels')}</span>
      </div>
      <span className={styles.sectionHint}>{t('opencode.ohMyOpenCode.agentModelsHint')}</span>
    </div>
  );

  const otherFieldsSectionLabel = (
    <div className={styles.sectionLabel}>
      <div className={styles.sectionLabelMain}>
        <span className={styles.sectionTitle}>{t('opencode.ohMyOpenCode.otherFields')}</span>
      </div>
      <span className={styles.sectionHint}>{t('opencode.ohMyOpenCodeSlim.otherFieldsHint')}</span>
    </div>
  );

  const batchReplaceButtonClassName = showBatchReplace
    ? `${styles.actionButton} ${styles.actionButtonActive}`
    : styles.actionButton;

  const handleCancelAddCustomAgent = () => {
    setShowAddAgent(false);
    setNewAgentKey('');
  };

  const otherFieldsValue = otherFieldsRef.current && Object.keys(otherFieldsRef.current).length > 0
    ? otherFieldsRef.current
    : undefined;

  const otherFieldsTable = (
    <div className={styles.optionTableWrap}>
      <table className={styles.optionTable}>
        <thead>
          <tr>
            <th>{t('opencode.ohMyOpenCodeSlim.optionName')}</th>
            <th>{t('opencode.ohMyOpenCodeSlim.optionType')}</th>
            <th>{t('opencode.ohMyOpenCodeSlim.optionDefault')}</th>
            <th>{t('opencode.ohMyOpenCodeSlim.optionDesc')}</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>tmux.enabled</td>
            <td>boolean</td>
            <td>false</td>
            <td>{t('opencode.ohMyOpenCodeSlim.tmuxEnabledDesc')}</td>
          </tr>
          <tr>
            <td>tmux.layout</td>
            <td>string</td>
            <td>"main-vertical"</td>
            <td>{t('opencode.ohMyOpenCodeSlim.tmuxLayoutDesc')}</td>
          </tr>
          <tr>
            <td>disabled_mcps</td>
            <td>string[]</td>
            <td>[]</td>
            <td>{t('opencode.ohMyOpenCodeSlim.disabledMcpsDesc')}</td>
          </tr>
        </tbody>
      </table>
    </div>
  );

  const actionsCardClassName = `${styles.sectionCard} ${styles.actionsCard}`;

  const agentsSectionContent = (
    <div className={styles.contentSection}>
      {SLIM_AGENT_TYPES.map(renderBuiltInAgentItem)}

      {customAgents.length > 0 && (
        <>
          <Divider className={styles.sectionDivider}>{t('opencode.ohMyOpenCode.customAgents')}</Divider>
          {customAgents.map(renderCustomAgentItem)}
        </>
      )}

      {showAddAgent ? (
        <div className={styles.addCustomRow}>
          <Input
            placeholder={t('opencode.ohMyOpenCode.customAgentKeyPlaceholder')}
            value={newAgentKey}
            onChange={(e) => setNewAgentKey(e.target.value)}
            onPressEnter={handleAddCustomAgent}
            className={styles.addCustomInput}
          />
          <Button type="primary" onClick={handleAddCustomAgent} className={styles.addCustomAction}>
            {t('common.confirm')}
          </Button>
          <Button onClick={handleCancelAddCustomAgent} className={styles.addCustomAction}>
            {t('common.cancel')}
          </Button>
        </div>
      ) : (
        <Button
          type="dashed"
          icon={<PlusOutlined />}
          onClick={() => setShowAddAgent(true)}
          className={styles.fullWidthAddButton}
        >
          {t('opencode.ohMyOpenCode.addCustomAgent')}
        </Button>
      )}
    </div>
  );

  const otherFieldsSectionContent = (
    <div className={styles.contentSection}>
      {otherFieldsTable}
      <Form.Item labelCol={{ span: 24 }} wrapperCol={{ span: 24 }} className={styles.jsonEditorItem}>
        <JsonEditor
          value={otherFieldsValue}
          onChange={(value) => {
            if (value === null || value === undefined) {
              otherFieldsRawRef.current = '';
            } else if (typeof value === 'string') {
              otherFieldsRawRef.current = value;
            } else {
              otherFieldsRawRef.current = JSON.stringify(value, null, 2);
            }
          }}
          height={200}
          minHeight={150}
          maxHeight={400}
          resizable
          mode="text"
          placeholder={`{
  "tmux": {
    "enabled": true,
    "layout": "main-vertical",
    "main_pane_size": 60
  }
}`}
        />
      </Form.Item>
    </div>
  );

  const councilSectionWrapperClassName = styles.sectionSpacer;

  return (
    <Modal
      className={styles.modal}
      title={isEdit
        ? t('opencode.ohMyOpenCodeSlim.editConfig')
        : t('opencode.ohMyOpenCodeSlim.addConfig')}
      open={open}
      onCancel={onCancel}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="submit" type="primary" loading={loading} onClick={handleSubmit}>
          {t('common.save')}
        </Button>,
      ]}
      width={800}
    >
      <div className={styles.content}>
        <Form
          className={styles.form}
          form={form}
          layout="horizontal"
          labelCol={{ span: labelCol }}
          wrapperCol={{ span: wrapperCol }}
        >
          <Form.Item name="id" hidden>
            <Input />
          </Form.Item>

          <div className={styles.scrollArea}>
            <div className={styles.sectionCard}>
              <Form.Item
                className={styles.nameItem}
                label={t('opencode.ohMyOpenCode.configName')}
                name="name"
                rules={[{ required: true, message: t('opencode.ohMyOpenCode.configNamePlaceholder') }]}
              >
                <Input placeholder={t('opencode.ohMyOpenCode.configNamePlaceholder')} />
              </Form.Item>
            </div>

            <div className={actionsCardClassName}>
              <div className={styles.actionsToolbar}>
                <div className={styles.actionsGroup}>
                  <Button
                    icon={<ImportOutlined />}
                    onClick={() => setShowImportJson(true)}
                    className={styles.actionButton}
                  >
                    {t('opencode.ohMyOpenCode.importFromJson')}
                  </Button>
                  {isEdit && (
                    <Button
                      icon={<SwapOutlined />}
                      onClick={() => setShowBatchReplace(!showBatchReplace)}
                      className={batchReplaceButtonClassName}
                    >
                      {t('opencode.ohMyOpenCode.batchReplaceModel')}
                    </Button>
                  )}
                </div>
              </div>

              {isEdit && showBatchReplace && (
                <div className={styles.batchPanel}>
                  <Text type="secondary" className={styles.helperText}>
                    {t('opencode.ohMyOpenCode.batchReplaceHint')}
                  </Text>
                  <div className={styles.batchFlow}>
                    <div className={`${styles.batchGroup} ${styles.batchGroupFrom}`}>
                      <div className={styles.batchGroupHeader}>
                        <Text className={`${styles.batchGroupTag} ${styles.batchGroupTagFrom}`}>
                          {t('opencode.ohMyOpenCode.batchReplaceFromTitle')}
                        </Text>
                        <Text type="secondary" className={styles.batchGroupHint}>
                          {t('opencode.ohMyOpenCode.batchReplaceFromHint')}
                        </Text>
                      </div>
                      <div className={styles.batchGroupFields}>
                        <div className={styles.batchField}>
                          <Text className={styles.batchLabel}>{t('opencode.ohMyOpenCode.batchReplaceFromPlaceholder')}</Text>
                          <Select
                            value={batchReplaceFromModel}
                            placeholder={t('opencode.ohMyOpenCode.batchReplaceFromPlaceholder')}
                            options={modelOptions}
                            allowClear
                            showSearch
                            optionFilterProp="label"
                            className={styles.batchSelect}
                            onChange={(value) => {
                              setBatchReplaceFromModel(value);
                              setBatchReplaceFromVariant(undefined);
                            }}
                          />
                        </div>
                        <div className={styles.batchField}>
                          <Text className={styles.batchLabel}>{t('opencode.ohMyOpenCode.batchReplaceFromVariantPlaceholder')}</Text>
                          <Select
                            value={batchReplaceFromVariant}
                            placeholder={t('opencode.ohMyOpenCode.batchReplaceFromVariantPlaceholder')}
                            options={fromModelVariants.map((v) => ({ label: v, value: v }))}
                            allowClear
                            disabled={!batchReplaceFromModel || fromModelVariants.length === 0}
                            className={styles.batchSelect}
                            onChange={(value) => setBatchReplaceFromVariant(value)}
                          />
                        </div>
                      </div>
                    </div>

                    <div className={styles.batchArrow}>
                      <span className={styles.batchArrowIcon}>
                        <SwapOutlined />
                      </span>
                    </div>

                    <div className={`${styles.batchGroup} ${styles.batchGroupTo}`}>
                      <div className={styles.batchGroupHeader}>
                        <Text className={`${styles.batchGroupTag} ${styles.batchGroupTagTo}`}>
                          {t('opencode.ohMyOpenCode.batchReplaceToTitle')}
                        </Text>
                        <Text type="secondary" className={styles.batchGroupHint}>
                          {t('opencode.ohMyOpenCode.batchReplaceToHint')}
                        </Text>
                      </div>
                      <div className={styles.batchGroupFields}>
                        <div className={styles.batchField}>
                          <Text className={styles.batchLabel}>{t('opencode.ohMyOpenCode.batchReplaceToPlaceholder')}</Text>
                          <Select
                            value={batchReplaceToModel}
                            placeholder={t('opencode.ohMyOpenCode.batchReplaceToPlaceholder')}
                            options={modelOptions}
                            allowClear
                            showSearch
                            optionFilterProp="label"
                            className={styles.batchSelect}
                            onChange={(value) => {
                              setBatchReplaceToModel(value);
                              setBatchReplaceToVariant(undefined);
                            }}
                          />
                        </div>
                        <div className={styles.batchField}>
                          <Text className={styles.batchLabel}>{t('opencode.ohMyOpenCode.batchReplaceToVariantPlaceholder')}</Text>
                          <Select
                            value={batchReplaceToVariant}
                            placeholder={t('opencode.ohMyOpenCode.batchReplaceToVariantPlaceholder')}
                            options={toModelVariants.map((v) => ({ label: v, value: v }))}
                            allowClear
                            disabled={!batchReplaceToModel || toModelVariants.length === 0}
                            className={styles.batchSelect}
                            onChange={(value) => setBatchReplaceToVariant(value)}
                          />
                        </div>
                      </div>
                    </div>
                  </div>
                  <div className={styles.batchActionRow}>
                    <Text type="secondary" className={styles.batchActionHint}>
                      {t('opencode.ohMyOpenCode.batchReplaceActionHint')}
                    </Text>
                    <Button
                      type="primary"
                      icon={<SwapOutlined />}
                      onClick={handleBatchReplaceModel}
                      className={styles.batchActionButton}
                    >
                      {t('opencode.ohMyOpenCode.batchReplaceAction')}
                    </Button>
                  </div>
                </div>
              )}
            </div>

            <Collapse
              className={styles.sectionCollapse}
              defaultActiveKey={['agents']}
              ghost
              items={[{ key: 'agents', label: agentsSectionLabel, children: agentsSectionContent }]}
            />

            <div className={councilSectionWrapperClassName}>
              <OhMyOpenCodeSlimCouncilForm
                form={form}
                modelOptions={modelOptions}
                modelVariantsMap={modelVariantsMap}
                councilOtherFieldsValidRef={councilOtherFieldsValidRef}
              />
            </div>

            <Collapse
              className={styles.sectionCollapse}
              defaultActiveKey={[]}
              ghost
              items={[{ key: 'other', label: otherFieldsSectionLabel, children: otherFieldsSectionContent }]}
            />
          </div>
        </Form>
      </div>

      <ImportJsonConfigModal
        open={showImportJson}
        onCancel={() => setShowImportJson(false)}
        onImport={handleImportJson}
        variant="omos"
      />
    </Modal>
  );
};

export default OhMyOpenCodeSlimConfigModal;
