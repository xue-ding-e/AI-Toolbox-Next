import React from 'react';
import { Modal, Checkbox, Button, Empty, Spin, Typography, Tag, Popconfirm, message } from 'antd';
import { DeleteOutlined, ApiOutlined, CloudServerOutlined, AppstoreOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { listFavoriteProviders, deleteFavoriteProvider, type OpenCodeFavoriteProvider } from '@/services/opencodeApi';
import type { ImportProviderModalProps } from './types';
import styles from './index.module.less';

const { Text } = Typography;

/**
 * Modal component for importing favorite providers
 */
const ImportProviderModal: React.FC<ImportProviderModalProps> = ({
  open,
  onClose,
  onImport,
  existingProviderIds,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [providers, setProviders] = React.useState<OpenCodeFavoriteProvider[]>([]);
  const [selectedIds, setSelectedIds] = React.useState<Set<string>>(new Set());
  const [deletingId, setDeletingId] = React.useState<string | null>(null);

  // Load favorite providers when modal opens
  const loadProviders = React.useCallback(async () => {
    setLoading(true);
    try {
      const data = await listFavoriteProviders();
      setProviders(data);
      // Default to no selection - user can use select all button
      setSelectedIds(new Set<string>());
    } catch (error) {
      console.error('Failed to load favorite providers:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (open) {
      loadProviders();
    }
  }, [open, loadProviders]);

  // Handle selection change
  const handleSelectionChange = (providerId: string, selected: boolean) => {
    setSelectedIds((prev) => {
      const newSet = new Set(prev);
      if (selected) {
        newSet.add(providerId);
      } else {
        newSet.delete(providerId);
      }
      return newSet;
    });
  };

  // Handle select all (only non-existing providers)
  const handleSelectAll = () => {
    const newSelectedIds = new Set<string>();
    providers.forEach((p) => {
      if (!existingProviderIds.includes(p.providerId)) {
        newSelectedIds.add(p.providerId);
      }
    });
    setSelectedIds(newSelectedIds);
  };

  // Handle deselect all
  const handleDeselectAll = () => {
    setSelectedIds(new Set<string>());
  };

  // Check if all importable providers are selected
  const importableProviders = providers.filter((p) => !existingProviderIds.includes(p.providerId));
  const isAllSelected = importableProviders.length > 0 &&
    importableProviders.every((p) => selectedIds.has(p.providerId));

  // Handle delete favorite provider
  const handleDelete = async (providerId: string) => {
    setDeletingId(providerId);
    try {
      await deleteFavoriteProvider(providerId);
      setProviders((prev) => prev.filter((p) => p.providerId !== providerId));
      setSelectedIds((prev) => {
        const newSet = new Set(prev);
        newSet.delete(providerId);
        return newSet;
      });
      message.success(t('opencode.provider.favoriteDeleted'));
    } catch (error) {
      console.error('Failed to delete favorite provider:', error);
      message.error(t('common.error'));
    } finally {
      setDeletingId(null);
    }
  };

  // Handle import
  const handleImport = () => {
    const selectedProviders = providers.filter((p) => selectedIds.has(p.providerId));
    if (selectedProviders.length === 0) {
      return;
    }
    onImport(selectedProviders);
  };

  // Count of importable providers (selected and not existing)
  const importableCount = Array.from(selectedIds).filter(
    (id) => !existingProviderIds.includes(id)
  ).length;

  // Sort providers: importable first, then existing
  const sortedProviders = React.useMemo(() => {
    return [...providers].sort((a, b) => {
      const aExists = existingProviderIds.includes(a.providerId);
      const bExists = existingProviderIds.includes(b.providerId);
      if (aExists === bExists) return 0;
      return aExists ? 1 : -1;
    });
  }, [providers, existingProviderIds]);

  return (
    <Modal
      title={t('opencode.provider.importModalTitle')}
      open={open}
      onCancel={onClose}
      width={800}
      className={styles.modal}
      footer={[
        <Button key="cancel" onClick={onClose}>
          {t('common.cancel')}
        </Button>,
        <Button
          key="import"
          type="primary"
          onClick={handleImport}
          disabled={importableCount === 0}
        >
          {t('opencode.provider.importSelected')} ({importableCount})
        </Button>,
      ]}
    >
      <Spin spinning={loading}>
        {providers.length === 0 && !loading ? (
          <Empty description={t('opencode.provider.noFavoriteProviders')} />
        ) : (
          <div>
            <div className={styles.toolbar}>
              <Checkbox
                checked={isAllSelected}
                indeterminate={selectedIds.size > 0 && !isAllSelected}
                onChange={(e) => e.target.checked ? handleSelectAll() : handleDeselectAll()}
              >
                {isAllSelected
                  ? t('opencode.provider.deselectAllProviders')
                  : t('opencode.provider.selectAllProviders')}
              </Checkbox>
            </div>
            <div className={styles.container}>
            {sortedProviders.map((provider) => {
              const isExisting = existingProviderIds.includes(provider.providerId);
              const isSelected = selectedIds.has(provider.providerId);
              const modelCount = Object.keys(provider.providerConfig.models || {}).length;

              return (
                <div
                  key={provider.providerId}
                  className={`${styles.card} ${isExisting ? styles.existing : ''} ${isSelected && !isExisting ? styles.selected : ''}`}
                  onClick={() => !isExisting && handleSelectionChange(provider.providerId, !isSelected)}
                >
                  <div className={styles.cardHeader}>
                    <Checkbox
                      checked={isSelected}
                      disabled={isExisting}
                      onClick={(e) => e.stopPropagation()}
                      onChange={(e) => handleSelectionChange(provider.providerId, e.target.checked)}
                    />
                    <div className={styles.titleArea}>
                      <Text strong className={styles.title}>
                        {provider.providerConfig.name || provider.providerId}
                      </Text>
                      {isExisting && (
                        <Tag className={styles.existsTag}>{t('opencode.provider.providerExists')}</Tag>
                      )}
                    </div>
                    <Popconfirm
                      title={t('opencode.provider.confirmDeleteFavorite')}
                      onConfirm={(e) => {
                        e?.stopPropagation();
                        handleDelete(provider.providerId);
                      }}
                      onCancel={(e) => e?.stopPropagation()}
                      okText={t('common.confirm')}
                      cancelText={t('common.cancel')}
                    >
                      <Button
                        type="text"
                        size="small"
                        className={styles.deleteBtn}
                        icon={<DeleteOutlined />}
                        loading={deletingId === provider.providerId}
                        onClick={(e) => e.stopPropagation()}
                      />
                    </Popconfirm>
                  </div>

                  <div className={styles.cardBody}>
                    <div className={styles.infoRow}>
                      <div className={styles.infoItem}>
                        <ApiOutlined className={styles.icon} />
                        <Text className={styles.infoText} ellipsis>
                          {provider.npm || '@ai-sdk/openai-compatible'}
                        </Text>
                      </div>
                      {provider.baseUrl && (
                        <div className={styles.infoItem}>
                          <CloudServerOutlined className={styles.icon} />
                          <Text className={styles.infoText} ellipsis title={provider.baseUrl}>
                            {provider.baseUrl}
                          </Text>
                        </div>
                      )}
                    </div>
                    <div className={styles.modelsRow}>
                      <AppstoreOutlined className={styles.icon} />
                      <div className={styles.modelTags}>
                        {Object.keys(provider.providerConfig.models || {}).map((modelId) => (
                          <Tag key={modelId} className={styles.modelTag}>{modelId}</Tag>
                        ))}
                        {modelCount === 0 && (
                          <Text type="secondary" className={styles.noModels}>0</Text>
                        )}
                      </div>
                    </div>
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
};

export default ImportProviderModal;
