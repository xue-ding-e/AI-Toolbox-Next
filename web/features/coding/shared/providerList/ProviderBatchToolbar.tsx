import React from 'react';
import { Button } from 'antd';
import { DeleteOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { ManagementCheckbox } from '../management';

interface ProviderBatchToolbarProps {
  /** Whether any provider is currently selected. */
  hasSelection: boolean;
  /** Visible (filtered) provider count in the current view. */
  visibleCount: number;
  isAllSelected: boolean;
  indeterminate: boolean;
  onSelectAll: (checked: boolean) => void;
  onBatchDelete: () => void;
  /** Disable all batch actions (e.g. while loading). */
  disabled?: boolean;
}

/**
 * Provider-list batch controls shown only while in selection mode: a select-all
 * checkbox, a delete button, and a trailing vertical divider that separates the
 * batch group from the rest of the header actions.
 *
 * The toggle button that enters/exits selection mode is rendered by the page
 * itself (as an antd link button matching the page's other header buttons);
 * this component only owns the selection-mode controls. All clicks call
 * `stopPropagation` so they don't bubble into a surrounding Collapse header and
 * collapse the provider list.
 */
const ProviderBatchToolbar: React.FC<ProviderBatchToolbarProps> = ({
  hasSelection,
  visibleCount,
  isAllSelected,
  indeterminate,
  onSelectAll,
  onBatchDelete,
  disabled = false,
}) => {
  const { t } = useTranslation();
  const stop = (e: React.MouseEvent) => e.stopPropagation();

  return (
    <>
      <span onClick={stop} style={{ display: 'inline-flex', alignItems: 'center' }}>
        <ManagementCheckbox
          checked={isAllSelected}
          indeterminate={indeterminate}
          ariaLabel={t('common.batch.selectAll')}
          onChange={onSelectAll}
          onClick={stop}
          disabled={visibleCount === 0 || disabled}
          style={{ width: 13, height: 13 }}
        />
      </span>
      <Button
        type="link"
        size="small"
        danger
        icon={<DeleteOutlined />}
        disabled={!hasSelection || disabled}
        onClick={(e) => {
          e.stopPropagation();
          onBatchDelete();
        }}
        style={{ fontSize: 12 }}
      >
        {t('common.batch.delete')}
      </Button>
      <span
        aria-hidden="true"
        onClick={stop}
        style={{
          display: 'inline-block',
          width: 1,
          height: 18,
          background: 'var(--color-border)',
          margin: '0 4px',
          alignSelf: 'center',
        }}
      />
    </>
  );
};

export default ProviderBatchToolbar;
