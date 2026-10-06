import React from 'react';
import { Form } from 'antd';
import { useTranslation } from 'react-i18next';
import BillingConfigCollapse from '@/features/coding/shared/providerBilling/BillingConfigCollapse';
import type { BillingConfigState } from '@/features/coding/shared/providerBilling/billingConfigUtils';
import CustomHeadersCollapse from '@/features/coding/shared/providerHeaders/CustomHeadersCollapse';
import type { CustomHeadersState } from '@/features/coding/shared/providerHeaders/customHeadersUtils';
import ModelRewritesCollapse from '@/features/coding/shared/providerModelRewrites/ModelRewritesCollapse';
import type { ModelRewritesState } from '@/features/coding/shared/providerModelRewrites/modelRewritesUtils';
import ProviderNotesCollapse from '@/features/coding/shared/providerConfig/ProviderNotesCollapse';

/**
 * Form-level wrapper column shared by every provider form section.
 * Sections are full-width rows; the label column belongs to the top fields.
 */
const PROVIDER_SECTION_WRAPPER_COL = { span: 24 };

export interface ProviderFormSectionsProps {
  /**
   * Whether the provider-specific sections are editable. Official-mode
   * providers keep only notes; everything below is hidden when false.
   */
  editable: boolean;

  /**
   * Advanced-settings block. Owned by the caller because the editor differs per
   * CLI (`JsonEditor` for Claude-style JSON settings, `TomlEditorFormItem` for
   * Codex `config.toml`), and it carries its own validation rules.
   */
  advancedSettings?: React.ReactNode;

  /**
   * Model-mapping block (Claude-style role → upstream model table). Codex
   * intentionally has none: its catalog lives on the provider card's model
   * list, so the mapping table is not rendered there. Callers that own one
   * pass it; the rest leave it undefined.
   */
  modelMapping?: React.ReactNode;

  /**
   * Billing section. Some flows (the import tab) deliberately omit it: the
   * imported provider has no pricing to edit yet. Defaults to following
   * `editable`.
   */
  showBilling?: boolean;
  billing: BillingConfigState;
  onBillingChange: (value: BillingConfigState) => void;

  customHeaders: CustomHeadersState;
  onCustomHeadersChange: (value: CustomHeadersState) => void;

  modelRewrites: ModelRewritesState;
  onModelRewritesChange: (value: ModelRewritesState) => void;

  /** i18n prefix for the notes section labels. */
  i18nPrefix: string;
  notesRows?: number;
  notesResetKey: string;
}

/**
 * Shared section layout for the provider edit modals.
 *
 * Locks the order every provider form uses after its top fields (category /
 * name / base URL / API key):
 *
 *   [model mapping]  ← Claude only, Codex keeps its catalog on the card
 *   [advanced settings]  ← caller-owned editor: JSON (Claude) vs TOML (Codex)
 *   [billing]
 *   [custom headers]
 *   [model rewrites]
 *   [notes]
 *
 * The top field block stays in each form: its fields, validation and
 * category-selection side effects differ enough per CLI that sharing it would
 * cost more than it saves.
 */
const ProviderFormSections: React.FC<ProviderFormSectionsProps> = ({
  editable,
  advancedSettings,
  modelMapping,
  showBilling = editable,
  billing,
  onBillingChange,
  customHeaders,
  onCustomHeadersChange,
  modelRewrites,
  onModelRewritesChange,
  i18nPrefix,
  notesRows = 3,
  notesResetKey,
}) => {
  const { t } = useTranslation();

  return (
    <>
      {modelMapping}

      {editable && advancedSettings}

      {editable && showBilling && (
        <Form.Item wrapperCol={PROVIDER_SECTION_WRAPPER_COL}>
          <BillingConfigCollapse value={billing} onChange={onBillingChange} />
        </Form.Item>
      )}

      {editable && (
        <Form.Item wrapperCol={PROVIDER_SECTION_WRAPPER_COL}>
          <CustomHeadersCollapse value={customHeaders} onChange={onCustomHeadersChange} />
        </Form.Item>
      )}

      {editable && (
        <Form.Item wrapperCol={PROVIDER_SECTION_WRAPPER_COL}>
          <ModelRewritesCollapse value={modelRewrites} onChange={onModelRewritesChange} />
        </Form.Item>
      )}

      <Form.Item name="notes" wrapperCol={PROVIDER_SECTION_WRAPPER_COL}>
        <ProviderNotesCollapse
          title={t(`${i18nPrefix}.provider.notes`)}
          placeholder={t(`${i18nPrefix}.provider.notesPlaceholder`)}
          rows={notesRows}
          resetKey={notesResetKey}
        />
      </Form.Item>
    </>
  );
};

export default ProviderFormSections;
