import React from 'react';
import { Button, Collapse, Empty, Space, Spin, Typography } from 'antd';
import {
  AppstoreOutlined,
  CheckSquareOutlined,
  DatabaseOutlined,
  PlusOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import ProviderBatchToolbar from './providerList/ProviderBatchToolbar';
import ProviderSearchEmpty from './providerList/ProviderSearchEmpty';
import ProviderSearchInput from './providerList/ProviderSearchInput';
import ProviderSortDropdown from './providerList/ProviderSortDropdown';
import type { ProviderSortMode } from './providerList/sortProviders';
import type { ProviderBatchSelection } from './providerList/useProviderBatchSelection';

const { Text } = Typography;

export interface ProviderListSectionProps {
  /** i18n prefix for `provider.title` / `commonConfigButton` / `addProvider`. */
  i18nPrefix: string;
  /** Sidebar section anchor id, e.g. `codex-providers`. */
  sectionId: string;

  collapsed: boolean;
  onCollapsedChange: (collapsed: boolean) => void;
  loading?: boolean;

  /** Provider count; drives the empty state. */
  providerCount: number;
  /** Count after the search filter; drives the search-empty state. */
  visibleCount: number;

  /** Provider-level batch selection state (see `useProviderBatchSelection`). */
  batch: ProviderBatchSelection<string>;
  /** Ids eligible for batch selection (already filtered by the caller). */
  batchSelectableIds: string[];

  keyword: string;
  onKeywordChange: (keyword: string) => void;
  sortMode: ProviderSortMode;
  sortModes: readonly ProviderSortMode[];
  onSortModeChange: (mode: ProviderSortMode) => void;

  onBatchTest?: () => void;
  batchTesting?: boolean;
  onOpenCommonConfig?: () => void;
  onAddProvider: () => void;

  /** Extra nodes rendered in the section header label, next to the title (e.g. Gateway chips). */
  headerExtra?: React.ReactNode;

  /**
   * Empty-state text. Defaults to `${i18nPrefix}.provider.emptyText`, but most
   * pages keep their `emptyText` at the top level (`<tool>.emptyText`) rather
   * than under `provider` — pass `t('<tool>.emptyText')` for those instead of
   * duplicating the string into a second key.
   */
  emptyText?: React.ReactNode;

  /** Hint block under the toolbar. Callers pass their own wording. */
  hint?: React.ReactNode;
  /** Rendered above the list (below the hint), e.g. the provider cards. */
  children: React.ReactNode;
  /** Import buttons rendered below the list (CC Switch / All API Hub / favorites). */
  footer?: React.ReactNode;
}

/**
 * Shared shell for the "provider list" section on every coding tab.
 *
 * Locks the skeleton the pages had each re-implemented: the Collapse section
 * header with the batch/search/sort/batch-test/common-config/add-provider
 * toolbar, the hint block, the empty and search-empty states, and the import
 * footer. Tool-specific wording and actions are passed in as props or slots so
 * a new CLI inherits the layout instead of copying it.
 */
const ProviderListSection: React.FC<ProviderListSectionProps> = ({
  i18nPrefix,
  sectionId,
  collapsed,
  onCollapsedChange,
  loading = false,
  providerCount,
  visibleCount,
  batch,
  batchSelectableIds,
  keyword,
  onKeywordChange,
  sortMode,
  sortModes,
  onSortModeChange,
  onBatchTest,
  batchTesting = false,
  onOpenCommonConfig,
  onAddProvider,
  headerExtra,
  emptyText,
  hint,
  children,
  footer,
}) => {
  const { t } = useTranslation();

  const toolbarButtonStyle: React.CSSProperties = { fontSize: 12 };

  return (
    <div id={sectionId} data-sidebar-section="true" data-sidebar-title={t(`${i18nPrefix}.provider.title`)}>
      <Collapse
        style={{ marginBottom: 16 }}
        activeKey={collapsed ? [] : ['providers']}
        onChange={(keys) => onCollapsedChange(!keys.includes('providers'))}
        items={[
          {
            key: 'providers',
            label: (
              <Space size={8} wrap>
                <Text strong>
                  <DatabaseOutlined style={{ marginRight: 8 }} />
                  {t(`${i18nPrefix}.provider.title`)}
                </Text>
                {headerExtra}
              </Space>
            ),
            extra: (
              <Space size={4} wrap>
                <Button
                  type="link"
                  size="small"
                  style={{ ...toolbarButtonStyle, display: 'inline-flex', alignItems: 'center', gap: 4 }}
                  onClick={(event) => {
                    event.stopPropagation();
                    if (batch.selectionMode) {
                      batch.exitSelection();
                    } else {
                      batch.enterSelection();
                    }
                  }}
                >
                  <CheckSquareOutlined style={{ fontSize: 13, lineHeight: 1 }} />
                  <span>
                    {batch.selectionMode ? t('common.batch.exit') : t('common.batch.manage')}
                  </span>
                </Button>
                {batch.selectionMode && (
                  <ProviderBatchToolbar
                    hasSelection={batch.hasSelection}
                    visibleCount={batchSelectableIds.length}
                    isAllSelected={batch.isAllSelected}
                    indeterminate={batch.indeterminate}
                    onSelectAll={batch.selectAllFiltered}
                    onBatchDelete={batch.batchDelete}
                    disabled={loading}
                  />
                )}
                <ProviderSearchInput value={keyword} onChange={onKeywordChange} />
                <ProviderSortDropdown mode={sortMode} modes={sortModes} onChange={onSortModeChange} />
                {onBatchTest && (
                  <Button
                    type="link"
                    size="small"
                    style={toolbarButtonStyle}
                    icon={<ThunderboltOutlined />}
                    loading={batchTesting}
                    onClick={(event) => {
                      event.stopPropagation();
                      onBatchTest();
                    }}
                  >
                    {t('common.batchTest')}
                  </Button>
                )}
                {onOpenCommonConfig && (
                  <Button
                    type="link"
                    size="small"
                    style={toolbarButtonStyle}
                    icon={<AppstoreOutlined />}
                    onClick={(event) => {
                      event.stopPropagation();
                      onOpenCommonConfig();
                    }}
                  >
                    {t(`${i18nPrefix}.commonConfigButton`)}
                  </Button>
                )}
                <Button
                  type="link"
                  size="small"
                  style={toolbarButtonStyle}
                  icon={<PlusOutlined />}
                  onClick={(event) => {
                    event.stopPropagation();
                    onAddProvider();
                  }}
                >
                  {t(`${i18nPrefix}.addProvider`)}
                </Button>
              </Space>
            ),
            children: (
              <Spin spinning={loading}>
                {hint}
                {providerCount === 0 ? (
                  <Empty
                    description={emptyText ?? t(`${i18nPrefix}.provider.emptyText`)}
                    style={{ marginTop: 40 }}
                  />
                ) : visibleCount === 0 ? (
                  <ProviderSearchEmpty />
                ) : (
                  children
                )}
                {footer && <div style={{ marginTop: 12 }}>{footer}</div>}
              </Spin>
            ),
          },
        ]}
      />
    </div>
  );
};

export default ProviderListSection;
