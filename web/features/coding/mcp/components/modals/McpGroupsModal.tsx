import React from 'react';
import { Form, Input, InputNumber, message, Modal, Button, Empty } from 'antd';
import { PlusOutlined, EditOutlined, DeleteOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import * as api from '../../services/mcpApi';
import type { McpGroupRecord } from '../../types';
import styles from './McpGroupsModal.module.less';

interface McpGroupsModalProps {
  open: boolean;
  groups: McpGroupRecord[];
  onClose: () => void;
  onSuccess: () => void;
}

interface GroupFormValues {
  name: string;
  note?: string;
  sortIndex?: number;
}

// Managed-group modal mirrors SkillGroupsModal one-to-one. Group membership is
// still by each server's `user_group` text; the backend keeps that text glued
// to renames and clears it when a group is deleted.
export const McpGroupsModal: React.FC<McpGroupsModalProps> = ({
  open,
  groups,
  onClose,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<GroupFormValues>();
  const [editingGroup, setEditingGroup] = React.useState<McpGroupRecord | null>(null);
  const [saving, setSaving] = React.useState(false);

  const sortedGroups = React.useMemo(
    () =>
      [...groups].sort((a, b) => {
        const sortDiff = a.sort_index - b.sort_index;
        if (sortDiff !== 0) return sortDiff;
        return a.name.localeCompare(b.name);
      }),
    [groups],
  );

  const startEdit = (group?: McpGroupRecord) => {
    setEditingGroup(group ?? null);
    form.setFieldsValue({
      name: group?.name ?? '',
      note: group?.note ?? '',
      sortIndex: group?.sort_index ?? groups.length,
    });
  };

  React.useEffect(() => {
    if (open) startEdit();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const handleSubmit = async (values: GroupFormValues) => {
    setSaving(true);
    try {
      await api.saveMcpGroup(
        values.name,
        values.note?.trim() || null,
        values.sortIndex ?? editingGroup?.sort_index ?? groups.length,
        editingGroup?.id,
      );
      message.success(t('mcp.groups.saveSuccess'));
      startEdit();
      onSuccess();
    } catch (error) {
      message.error(String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = (group: McpGroupRecord) => {
    Modal.confirm({
      title: t('mcp.groups.deleteTitle'),
      content: t('mcp.groups.deleteContent', { name: group.name }),
      okText: t('common.delete'),
      okButtonProps: { danger: true },
      cancelText: t('common.cancel'),
      onOk: async () => {
        await api.deleteMcpGroup(group.id);
        message.success(t('mcp.groups.deleteSuccess'));
        onSuccess();
      },
    });
  };

  return (
    <Modal
      open={open}
      title={t('mcp.groups.title')}
      onCancel={onClose}
      footer={null}
      width={980}
      destroyOnHidden
      className={styles.modal}
    >
      <div className={styles.content}>
        <section className={styles.sectionCard}>
          <div className={styles.panelHeader}>
            <div>
              <div className={styles.sectionEyebrow}>{t('mcp.groups.listEyebrow')}</div>
              <div className={styles.sectionTitle}>{t('mcp.groups.listTitle')}</div>
              <p className={styles.sectionDescription}>{t('mcp.groups.listDescription')}</p>
            </div>
            <div className={styles.groupCount}>{t('mcp.groups.count', { count: sortedGroups.length })}</div>
          </div>

          {sortedGroups.length === 0 ? (
            <div className={styles.emptyState}>
              <Empty
                image={Empty.PRESENTED_IMAGE_SIMPLE}
                description={
                  <div className={styles.emptyCopy}>
                    <strong>{t('mcp.groups.emptyTitle')}</strong>
                    <span>{t('mcp.groups.empty')}</span>
                  </div>
                }
              />
              <Button type="primary" icon={<PlusOutlined />} onClick={() => startEdit()}>
                {t('mcp.groups.createFirst')}
              </Button>
            </div>
          ) : (
            <div className={styles.groupList}>
              {sortedGroups.map((group, index) => {
                const isActive = editingGroup?.id === group.id;

                return (
                  <div
                    key={group.id}
                    role="button"
                    tabIndex={0}
                    className={isActive ? `${styles.groupRow} ${styles.groupRowActive}` : styles.groupRow}
                    onClick={() => startEdit(group)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        startEdit(group);
                      }
                    }}
                  >
                    <div className={styles.groupOrder}>{String(index + 1).padStart(2, '0')}</div>
                    <div className={styles.groupMain}>
                      <div className={styles.groupNameRow}>
                        <strong>{group.name}</strong>
                        <span className={styles.groupMeta}>{t('mcp.groups.sortValue', { value: group.sort_index })}</span>
                      </div>
                      <span className={styles.groupNote}>
                        {group.note?.trim() || t('mcp.groups.noteEmpty')}
                      </span>
                    </div>
                    <div className={styles.groupActions}>
                      <Button
                        size="small"
                        danger
                        icon={<DeleteOutlined />}
                        onClick={(event) => {
                          event.stopPropagation();
                          handleDelete(group);
                        }}
                      >
                        {t('common.delete')}
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <section className={styles.sectionCard}>
          <div className={styles.panelHeader}>
            <div>
              <div className={styles.sectionTitle}>
                {editingGroup ? t('mcp.groups.editTitle', { name: editingGroup.name }) : t('mcp.groups.createTitle')}
              </div>
              <p className={styles.sectionDescription}>
                {editingGroup ? t('mcp.groups.editDescription') : t('mcp.groups.createDescription')}
              </p>
            </div>
            {editingGroup ? (
              <Button onClick={() => startEdit()}>{t('mcp.groups.newAction')}</Button>
            ) : null}
          </div>

          <Form
            form={form}
            layout="horizontal"
            labelCol={{ flex: '108px' }}
            wrapperCol={{ flex: 'auto' }}
            onFinish={handleSubmit}
            className={styles.form}
          >
            <Form.Item label={t('mcp.groups.name')} name="name" rules={[{ required: true, message: t('mcp.groups.nameRequired') }]}>
              <Input placeholder={t('mcp.groups.namePlaceholder')} />
            </Form.Item>

            <Form.Item label={t('mcp.groups.note')} name="note">
              <Input.TextArea rows={4} placeholder={t('mcp.groups.notePlaceholder')} />
            </Form.Item>

            <Form.Item label={t('mcp.groups.sortOrder')} name="sortIndex">
              <InputNumber min={0} precision={0} className={styles.sortInput} placeholder="0" />
            </Form.Item>

            <div className={styles.formActions}>
              <Button onClick={() => startEdit()}>{editingGroup ? t('common.cancel') : t('common.reset')}</Button>
              <Button type="primary" htmlType="submit" loading={saving} icon={editingGroup ? <EditOutlined /> : <PlusOutlined />}>
                {editingGroup ? t('common.save') : t('mcp.groups.create')}
              </Button>
            </div>
          </Form>
        </section>
      </div>
    </Modal>
  );
};

export default McpGroupsModal;
