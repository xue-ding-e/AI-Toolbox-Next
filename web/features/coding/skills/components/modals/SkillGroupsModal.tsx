import React from 'react';
import { Form, Input, InputNumber, message, Modal, Button, Empty, Space } from 'antd';
import { PlusOutlined, EditOutlined, DeleteOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import * as api from '../../services/skillsApi';
import type { SkillGroupRecord } from '../../types';
import styles from './SkillGroupsModal.module.less';

interface SkillGroupsModalProps {
  open: boolean;
  groups: SkillGroupRecord[];
  onClose: () => void;
  onSuccess: () => void;
}

interface GroupFormValues {
  name: string;
  note?: string;
  sortIndex?: number;
}

export const SkillGroupsModal: React.FC<SkillGroupsModalProps> = ({
  open,
  groups,
  onClose,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<GroupFormValues>();
  const [editingGroup, setEditingGroup] = React.useState<SkillGroupRecord | null>(null);
  const [saving, setSaving] = React.useState(false);

  const startEdit = (group?: SkillGroupRecord) => {
    setEditingGroup(group ?? null);
    form.setFieldsValue({
      name: group?.name ?? '',
      note: group?.note ?? '',
      sortIndex: group?.sort_index ?? groups.length,
    });
  };

  React.useEffect(() => {
    if (open) startEdit();
  }, [open]);

  const handleSubmit = async (values: GroupFormValues) => {
    setSaving(true);
    try {
      await api.saveSkillGroup(
        values.name,
        values.note?.trim() || null,
        values.sortIndex ?? editingGroup?.sort_index ?? groups.length,
        editingGroup?.id,
      );
      message.success(t('skills.groups.saveSuccess'));
      startEdit();
      onSuccess();
    } catch (error) {
      message.error(String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = (group: SkillGroupRecord) => {
    Modal.confirm({
      title: t('skills.groups.deleteTitle'),
      content: t('skills.groups.deleteContent', { name: group.name }),
      okText: t('common.delete'),
      okButtonProps: { danger: true },
      cancelText: t('common.cancel'),
      onOk: async () => {
        await api.deleteSkillGroup(group.id);
        message.success(t('skills.groups.deleteSuccess'));
        onSuccess();
      },
    });
  };

  return (
    <Modal
      open={open}
      title={t('skills.groups.title')}
      onCancel={onClose}
      footer={null}
      destroyOnHidden
      className={styles.modal}
    >
      <div className={styles.content}>
        <section className={styles.sectionCard}>
          <div className={styles.sectionTitle}>
            {editingGroup ? t('skills.groups.editTitle', { name: editingGroup.name }) : t('skills.groups.createTitle')}
          </div>
          <Form form={form} layout="horizontal" labelCol={{ span: 5 }} wrapperCol={{ span: 19 }} onFinish={handleSubmit}>
            <Form.Item label={t('skills.groups.name')} name="name" rules={[{ required: true, message: t('skills.groups.nameRequired') }]}>
              <Input placeholder={t('skills.groups.namePlaceholder')} />
            </Form.Item>
            <Form.Item label={t('skills.groups.note')} name="note">
              <Input.TextArea rows={2} placeholder={t('skills.groups.notePlaceholder')} />
            </Form.Item>
            <Form.Item label={t('skills.groups.sortOrder')} name="sortIndex">
              <InputNumber min={0} precision={0} className={styles.sortInput} />
            </Form.Item>
            <div className={styles.formActions}>
              {editingGroup && (
                <Button onClick={() => startEdit()}>{t('common.cancel')}</Button>
              )}
              <Button type="primary" htmlType="submit" loading={saving} icon={editingGroup ? <EditOutlined /> : <PlusOutlined />}>
                {editingGroup ? t('common.save') : t('skills.groups.create')}
              </Button>
            </div>
          </Form>
        </section>

        <section className={styles.groupList}>
          {groups.length === 0 ? (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={t('skills.groups.empty')} />
          ) : (
            groups.map((group) => (
              <div key={group.id} className={styles.groupRow}>
                <div className={styles.groupMain}>
                  <strong>{group.name}</strong>
                  {group.note && <span className={styles.groupNote}>{group.note}</span>}
                </div>
                <Space size="small">
                  <Button size="small" icon={<EditOutlined />} onClick={() => startEdit(group)} />
                  <Button size="small" danger icon={<DeleteOutlined />} onClick={() => handleDelete(group)} />
                </Space>
              </div>
            ))
          )}
        </section>
      </div>
    </Modal>
  );
};
