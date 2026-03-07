import React from 'react';
import { Modal, Checkbox, Button, Empty, Spin, Typography, Tag, Input } from 'antd';
import { ApiOutlined, CloudServerOutlined, AppstoreOutlined } from '@ant-design/icons';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { ImportExternalProvidersModalProps } from './types';
import styles from './index.module.less';

const { Text } = Typography;

const currencyFormatter = new Intl.NumberFormat(undefined, {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

function ImportExternalProvidersModal<TConfig>({
  open,
  title,
  loading,
  items,
  existingProviderIds,
  emptyDescription,
  cancelText,
  importButtonText,
  selectAllText,
  deselectAllText,
  existingTagText,
  noApiKeyTagText,
  disabledTagText,
  balanceLabelText,
  profileLabel,
  siteTypeLabel,
  loadingTokenText,
  tokenResolvedText,
  searchPlaceholder,
  onCancel,
  onImport,
  onResolveToken,
}: ImportExternalProvidersModalProps<TConfig>) {
  const [selectedIds, setSelectedIds] = React.useState<Set<string>>(new Set());
  const [resolvingIds, setResolvingIds] = React.useState<Set<string>>(new Set());
  const [resolvedIds, setResolvedIds] = React.useState<Set<string>>(new Set());
  const [failedIds, setFailedIds] = React.useState<Set<string>>(new Set());
  const [searchText, setSearchText] = React.useState('');

  React.useEffect(() => {
    if (open) {
      setSelectedIds(new Set());
      setResolvingIds(new Set());
      setResolvedIds(new Set());
      setFailedIds(new Set());
      setSearchText('');
    }
  }, [open]);

  React.useEffect(() => {
    if (!onResolveToken) {
      return;
    }

    const unresolvedSelectedIds = items
      .filter((item) => selectedIds.has(item.providerId))
      .filter((item) => !item.hasApiKey)
      .filter((item) => !resolvedIds.has(item.providerId))
      .map((item) => item.providerId)
      .filter((providerId) => !resolvingIds.has(providerId));

    if (unresolvedSelectedIds.length === 0) {
      return;
    }

    unresolvedSelectedIds.forEach((providerId) => {
      setResolvingIds((prev) => new Set(prev).add(providerId));
      onResolveToken(providerId)
        .then((success) => {
          if (!success) {
            setSelectedIds((prev) => {
              const next = new Set(prev);
              next.delete(providerId);
              return next;
            });
            setFailedIds((prev) => new Set(prev).add(providerId));
          } else {
            setFailedIds((prev) => {
              const next = new Set(prev);
              next.delete(providerId);
              return next;
            });
          }
        })
        .finally(() => {
          setResolvingIds((prev) => {
            const next = new Set(prev);
            next.delete(providerId);
            return next;
          });
          setResolvedIds((prev) => new Set(prev).add(providerId));
        });
    });
  }, [items, onResolveToken, resolvedIds, resolvingIds, selectedIds]);

  const filteredItems = React.useMemo(() => {
    const keyword = searchText.trim().toLowerCase();
    const matchedItems = !keyword
      ? items
      : items.filter((item) => {
          const name = item.name.toLowerCase();
          const domain = (() => {
            if (!item.baseUrl) {
              return '';
            }
            try {
              return new URL(item.baseUrl).host.toLowerCase();
            } catch {
              return item.baseUrl.toLowerCase();
            }
          })();

          return name.includes(keyword) || domain.includes(keyword);
        });

    return matchedItems
      .map((item, index) => ({ item, index }))
      .sort((left, right) => {
        const leftDisabled = left.item.isDisabled ? 1 : 0;
        const rightDisabled = right.item.isDisabled ? 1 : 0;
        if (leftDisabled !== rightDisabled) {
          return leftDisabled - rightDisabled;
        }
        return left.index - right.index;
      })
      .map(({ item }) => item);
  }, [items, searchText]);

  const filteredImportableItems = React.useMemo(
    () =>
      filteredItems.filter(
        (item) =>
          !existingProviderIds.includes(item.providerId) &&
          !item.isDisabled &&
          !failedIds.has(item.providerId)
      ),
    [existingProviderIds, failedIds, filteredItems]
  );

  const isAllSelected =
    filteredImportableItems.length > 0 &&
    filteredImportableItems.every((item) => selectedIds.has(item.providerId));

  const selectedVisibleCount = filteredImportableItems.filter((item) =>
    selectedIds.has(item.providerId)
  ).length;

  const importableCount = Array.from(selectedIds).filter(
    (id) =>
      items.some(
        (item) =>
          item.providerId === id &&
          !existingProviderIds.includes(id) &&
          !item.isDisabled &&
          !failedIds.has(id)
      )
  ).length;

  const formatBalance = React.useCallback((value: number) => currencyFormatter.format(value), []);

  const handleToggle = (providerId: string, checked: boolean) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (checked) {
        next.add(providerId);
      } else {
        next.delete(providerId);
      }
      return next;
    });
  };

  const handleImport = () => {
    const selected = items.filter((item) => selectedIds.has(item.providerId));
    if (selected.length === 0) {
      return;
    }
    onImport(selected);
  };

  return (
    <Modal
      title={title}
      open={open}
      onCancel={onCancel}
      width={760}
      className={styles.modal}
      destroyOnClose
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {cancelText}
        </Button>,
        <Button key="import" type="primary" onClick={handleImport} disabled={importableCount === 0}>
          {importButtonText} ({importableCount})
        </Button>,
      ]}
    >
      <Spin spinning={loading}>
        {items.length === 0 && !loading ? (
          <Empty description={emptyDescription} />
        ) : (
          <div>
            <div className={styles.toolbar}>
              <Checkbox
                checked={isAllSelected}
                indeterminate={selectedVisibleCount > 0 && !isAllSelected}
                onChange={(e) => {
                  if (e.target.checked) {
                    setSelectedIds((prev) => {
                      const next = new Set(prev);
                      filteredImportableItems.forEach((item) => next.add(item.providerId));
                      return next;
                    });
                  } else {
                    setSelectedIds((prev) => {
                      const next = new Set(prev);
                      filteredImportableItems.forEach((item) => next.delete(item.providerId));
                      return next;
                    });
                  }
                }}
              >
                {isAllSelected ? deselectAllText : selectAllText}
              </Checkbox>
              <div className={styles.toolbarRight}>
                <Text className={styles.summary}>
                  {filteredItems.length} / {items.length}
                </Text>
                <Input
                  allowClear
                  size="small"
                  className={styles.searchInput}
                  placeholder={searchPlaceholder}
                  value={searchText}
                  onChange={(e) => setSearchText(e.target.value)}
                />
              </div>
            </div>
            <div className={styles.container}>
              {filteredItems.map((item) => {
                const isExisting = existingProviderIds.includes(item.providerId);
                const isSelected = selectedIds.has(item.providerId);
                const isFailed = failedIds.has(item.providerId);
                const isDisabled = isExisting || isFailed || !!item.isDisabled;
                const balanceText =
                  typeof item.balanceUsd === 'number'
                    ? `$${formatBalance(item.balanceUsd)}`
                    : typeof item.balanceCny === 'number'
                      ? `¥${formatBalance(item.balanceCny)}`
                      : null;
                return (
                  <div
                    key={item.providerId}
                    className={`${styles.card} ${isDisabled ? styles.existing : ''} ${isSelected && !isDisabled ? styles.selected : ''}`}
                    onClick={() => {
                      if (!isDisabled) {
                        handleToggle(item.providerId, !isSelected);
                      }
                    }}
                  >
                    <div className={styles.cardHeader}>
                      <Checkbox
                        checked={isSelected}
                        disabled={isDisabled}
                        onClick={(e) => e.stopPropagation()}
                        onChange={(e) => handleToggle(item.providerId, e.target.checked)}
                      />
                      <div className={styles.titleArea}>
                        <Text strong className={styles.title}>
                          {item.name}
                        </Text>
                        {balanceText && (
                          <Tag className={styles.tag}>
                            {balanceLabelText}: <span className={styles.balanceValue}>{balanceText}</span>
                          </Tag>
                        )}
                        <Tag className={styles.tag}>{item.providerId}</Tag>
                        {item.secondaryLabel && <Tag className={styles.tag}>{item.secondaryLabel}</Tag>}
                        {isExisting && <Tag className={styles.tag}>{existingTagText}</Tag>}
                        {item.isDisabled && <Tag className={styles.tag}>{disabledTagText}</Tag>}
                        {isFailed && <Tag color="warning" className={styles.tag}>{noApiKeyTagText}</Tag>}
                      </div>
                    </div>
                    <div className={styles.cardBody}>
                      {(item.baseUrl || item.siteType || item.sourceProfileName) && (
                        <div className={styles.infoRow}>
                          {item.baseUrl && (
                            <div className={styles.infoItem}>
                              <CloudServerOutlined className={styles.icon} />
                              <a
                                href={item.baseUrl}
                                className={styles.infoLink}
                                title={item.baseUrl}
                                onClick={(event) => {
                                  event.preventDefault();
                                  event.stopPropagation();
                                  void openUrl(item.baseUrl!);
                                }}
                              >
                                {item.baseUrl}
                              </a>
                            </div>
                          )}
                          {item.siteType && (
                            <div className={styles.infoItem}>
                              <ApiOutlined className={styles.icon} />
                              <Text className={styles.infoText}>
                                {siteTypeLabel}: {item.siteType}
                              </Text>
                            </div>
                          )}
                          {item.sourceProfileName && (
                            <div className={styles.infoItem}>
                              <AppstoreOutlined className={styles.icon} />
                              <Text className={styles.infoText}>
                                {profileLabel}: {item.sourceProfileName}
                              </Text>
                            </div>
                          )}
                        </div>
                      )}
                      {isSelected && (
                        <div className={styles.tokenRow}>
                          {resolvingIds.has(item.providerId) && (
                            <Tag color="processing" className={styles.tag}>
                              {loadingTokenText}
                            </Tag>
                          )}
                          {!resolvingIds.has(item.providerId) && item.hasApiKey && (
                            <Tag color="success" className={styles.tag}>
                              {tokenResolvedText}
                              {item.apiKeyPreview ? `: ${item.apiKeyPreview}` : ''}
                            </Tag>
                          )}
                          {!resolvingIds.has(item.providerId) &&
                            !item.hasApiKey &&
                            resolvedIds.has(item.providerId) && (
                              <Tag color="warning" className={styles.tag}>
                                {noApiKeyTagText}
                              </Tag>
                            )}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </Spin>
    </Modal>
  );
}

export default ImportExternalProvidersModal;
