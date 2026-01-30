import React from 'react';
import { Modal, Button, Space } from 'antd';
import { PlusOutlined, UserOutlined, ImportOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useSkillsStore } from '../stores/skillsStore';
import { useSkills } from '../hooks/useSkills';
import { SkillsList } from './SkillsList';
import { AddSkillModal } from './modals/AddSkillModal';
import { ImportModal } from './modals/ImportModal';
import { SkillsSettingsModal } from './modals/SkillsSettingsModal';
import { DeleteConfirmModal } from './modals/DeleteConfirmModal';
import { NewToolsModal } from './modals/NewToolsModal';
import styles from './SkillsModal.module.less';

interface SkillsModalProps {
  open?: boolean;
  onClose?: () => void;
}

export const SkillsModal: React.FC<SkillsModalProps> = ({ open, onClose }) => {
  const { t } = useTranslation();
  const {
    isModalOpen,
    setModalOpen,
    isAddModalOpen,
    setAddModalOpen,
    isImportModalOpen,
    setImportModalOpen,
    isSettingsModalOpen,
    setSettingsModalOpen,
    isNewToolsModalOpen,
    onboardingPlan,
    loading,
  } = useSkillsStore();

  // Use props if provided, otherwise use store state
  const isOpen = open !== undefined ? open : isModalOpen;
  const handleClose = () => {
    if (onClose) {
      onClose();
    } else {
      setModalOpen(false);
    }
  };

  const {
    skills,
    getInstalledTools,
    getAllTools,
    formatRelative,
    getGithubInfo,
    getSkillSourceLabel,
    toggleToolSync,
    updateSkill,
    deleteSkill,
    refresh,
  } = useSkills();

  const [deleteSkillId, setDeleteSkillId] = React.useState<string | null>(null);
  const [actionLoading, setActionLoading] = React.useState(false);

  const installedTools = getInstalledTools();
  const allTools = getAllTools();
  const skillToDelete = deleteSkillId
    ? skills.find((s) => s.id === deleteSkillId)
    : null;

  const discoveredCount = onboardingPlan?.total_skills_found || 0;

  const handleToggleTool = async (skill: typeof skills[0], toolId: string) => {
    setActionLoading(true);
    try {
      await toggleToolSync(skill, toolId);
    } catch (error) {
      console.error('Failed to toggle sync:', error);
    } finally {
      setActionLoading(false);
    }
  };

  const handleUpdate = async (skill: typeof skills[0]) => {
    setActionLoading(true);
    try {
      await updateSkill(skill);
    } catch (error) {
      console.error('Failed to update skill:', error);
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = (skillId: string) => {
    setDeleteSkillId(skillId);
  };

  const confirmDelete = async () => {
    if (!deleteSkillId) return;
    setActionLoading(true);
    try {
      await deleteSkill(deleteSkillId);
      setDeleteSkillId(null);
    } catch (error) {
      console.error('Failed to delete skill:', error);
    } finally {
      setActionLoading(false);
    }
  };

  return (
    <>
      <Modal
        title={t('skills.title')}
        open={isOpen}
        onCancel={handleClose}
        footer={null}
        width={900}
        className={styles.skillsModal}
        destroyOnClose
      >
        <div className={styles.header}>
          <Space>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={() => setAddModalOpen(true)}
            >
              {t('skills.newSkill')}
            </Button>
            {discoveredCount > 0 && (
              <Button icon={<ImportOutlined />} onClick={() => setImportModalOpen(true)}>
                {t('skills.reviewImport')} ({discoveredCount})
              </Button>
            )}
          </Space>
          <Button
            icon={<UserOutlined />}
            onClick={() => setSettingsModalOpen(true)}
          >
            {t('skills.settings')}
          </Button>
        </div>

        <div className={styles.content}>
          <SkillsList
            skills={skills}
            installedTools={installedTools}
            loading={loading || actionLoading}
            getGithubInfo={getGithubInfo}
            getSkillSourceLabel={getSkillSourceLabel}
            formatRelative={formatRelative}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
            onToggleTool={handleToggleTool}
          />
        </div>
      </Modal>

      <AddSkillModal
        open={isAddModalOpen}
        onClose={() => setAddModalOpen(false)}
        allTools={allTools}
        onSuccess={() => {
          setAddModalOpen(false);
          refresh();
        }}
      />

      <ImportModal
        open={isImportModalOpen}
        onClose={() => setImportModalOpen(false)}
        onSuccess={() => {
          setImportModalOpen(false);
          refresh();
        }}
      />

      <SkillsSettingsModal
        open={isSettingsModalOpen}
        onClose={() => setSettingsModalOpen(false)}
      />

      <DeleteConfirmModal
        open={!!deleteSkillId}
        skillName={skillToDelete?.name || ''}
        onClose={() => setDeleteSkillId(null)}
        onConfirm={confirmDelete}
        loading={actionLoading}
      />

      <NewToolsModal
        open={isNewToolsModalOpen}
      />
    </>
  );
};
