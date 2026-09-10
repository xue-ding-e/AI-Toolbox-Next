import React from 'react';
import { Modal } from 'antd';
import { useTranslation } from 'react-i18next';
import { reconcileProviderSelection } from './providerBatchOperations';

/**
 * Shared batch-selection state for provider lists.
 *
 * Mirrors the Skills/MCP batch pattern (SkillsPage `selectedIds` +
 * `handleSelectChange` + `handleSelectAllFiltered`), extracted so every coding
 * tab's provider list can offer select-all / batch-delete without duplicating
 * the Set bookkeeping and confirm dialog.
 *
 * Selection is deliberately scoped to the *visible (filtered)* list passed via
 * `allIds`, matching Skills' `handleSelectAllFiltered` semantics.
 */
export function useProviderBatchSelection<K extends string>(opts: {
  /** Visible provider ids that the owning page permits deleting. */
  allIds: K[];
  /** Return true only after the whole batch succeeds; false preserves selection. */
  onBatchDelete: (ids: K[]) => Promise<boolean>;
}) {
  const { allIds, onBatchDelete } = opts;
  const { t } = useTranslation();
  const [selectionMode, setSelectionMode] = React.useState(false);
  const [selectedIds, setSelectedIds] = React.useState<Set<K>>(new Set());
  const selectableIds = React.useMemo(() => new Set(allIds), [allIds]);
  const currentSelectedIds = React.useMemo(
    () => reconcileProviderSelection(selectedIds, selectableIds),
    [selectedIds, selectableIds],
  );
  const latestDeleteOptions = React.useRef({ selectableIds, onBatchDelete });

  React.useEffect(() => {
    latestDeleteOptions.current = { selectableIds, onBatchDelete };
  }, [selectableIds, onBatchDelete]);

  // Drop ids that are no longer in the visible list so stale selections from a
  // previous filter/search don't leak into counts or batch actions.
  React.useEffect(() => {
    if (selectedIds.size === 0) {
      return;
    }
    setSelectedIds((previous) => reconcileProviderSelection(previous, selectableIds));
  }, [selectableIds, selectedIds.size]);

  const isSelectable = React.useCallback((id: K) => selectableIds.has(id), [selectableIds]);

  const toggleSelect = React.useCallback((id: K, checked: boolean) => {
    if (checked && !selectableIds.has(id)) return;
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (checked) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  }, [selectableIds]);

  const selectAllFiltered = React.useCallback((checked: boolean) => {
    setSelectedIds(checked ? new Set(allIds) : new Set());
  }, [allIds]);

  const clearSelection = React.useCallback(() => {
    setSelectedIds(new Set());
  }, []);

  const enterSelection = React.useCallback(() => {
    setSelectionMode(true);
  }, []);

  const exitSelection = React.useCallback(() => {
    setSelectionMode(false);
    setSelectedIds(new Set());
  }, []);

  const selectedArray = React.useMemo(() => [...currentSelectedIds], [currentSelectedIds]);
  const hasSelection = selectedArray.length > 0;
  const visibleCount = allIds.length;
  const isAllSelected = visibleCount > 0 && selectedArray.length === visibleCount
    ? allIds.every((id) => currentSelectedIds.has(id))
    : false;
  const indeterminate = hasSelection && !isAllSelected;

  const batchDelete = React.useCallback(() => {
    if (selectedArray.length === 0) {
      return;
    }
    const idsToDelete = [...selectedArray];
    Modal.confirm({
      title: t('common.batch.deleteConfirmTitle'),
      content: t('common.batch.deleteConfirmContent', { count: idsToDelete.length }),
      okText: t('common.batch.delete'),
      okType: 'danger',
      cancelText: t('common.cancel'),
      onOk: async () => {
        // A default/provider can change while the confirmation is open.
        const latestOptions = latestDeleteOptions.current;
        const deletableIds = idsToDelete.filter((id) => latestOptions.selectableIds.has(id));
        if (deletableIds.length > 0 && await latestOptions.onBatchDelete(deletableIds)) {
          exitSelection();
        }
      },
    });
  }, [selectedArray, exitSelection, t]);

  return {
    selectionMode,
    selectedIds: currentSelectedIds,
    selectedArray,
    hasSelection,
    isAllSelected,
    indeterminate,
    enterSelection,
    exitSelection,
    clearSelection,
    isSelectable,
    toggleSelect,
    selectAllFiltered,
    batchDelete,
  };
}

export type ProviderBatchSelection<K extends string> = ReturnType<
  typeof useProviderBatchSelection<K>
>;
