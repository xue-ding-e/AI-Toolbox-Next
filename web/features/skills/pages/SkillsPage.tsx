import React from 'react';
import { Typography, Button, Space, Modal, message } from 'antd';
import { PlusOutlined, UserOutlined, ImportOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useSkillsStore } from '../stores/skillsStore';
import { useSkills } from '../hooks/useSkills';
import { SkillsList } from '../components/SkillsList';
import { AddSkillModal } from '../components/modals/AddSkillModal';
import { ImportModal } from '../components/modals/ImportModal';
import { SkillsSettingsModal } from '../components/modals/SkillsSettingsModal';
import { DeleteConfirmModal } from '../components/modals/DeleteConfirmModal';
import { NewToolsModal } from '../components/modals/NewToolsModal';
import { formatGitError, isGitError } from '../utils/gitErrorParser';
import styles from './SkillsPage.module.less';

const { Title } = Typography;

const SkillsPage: React.FC = () => {
  const { t } = useTranslation();
  const {
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

  // Initialize data on mount
  React.useEffect(() => {
    refresh();
  }, []);

  const installedTools = getInstalledTools();
  const allTools = getAllTools();
  const skillToDelete = deleteSkillId
    ? skills.find((s) => s.id === deleteSkillId)
    : null;

  const discoveredCount = onboardingPlan?.total_skills_found || 0;

  const showGitError = (errMsg: string) => {
    if (isGitError(errMsg)) {
      Modal.error({
        title: t('common.error'),
        content: (
          <div style={{ whiteSpace: 'pre-wrap', maxHeight: '400px', overflow: 'auto' }}>
            {formatGitError(errMsg, t)}
          </div>
        ),
        width: 600,
      });
    } else {
      message.error(errMsg);
    }
  };

  const handleToggleTool = async (skill: typeof skills[0], toolId: string) => {
    setActionLoading(true);
    try {
      await toggleToolSync(skill, toolId);
    } catch (error) {
      showGitError(String(error));
    } finally {
      setActionLoading(false);
    }
  };

  const handleUpdate = async (skill: typeof skills[0]) => {
    setActionLoading(true);
    try {
      await updateSkill(skill);
    } catch (error) {
      showGitError(String(error));
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
      showGitError(String(error));
    } finally {
      setActionLoading(false);
    }
  };

  return (
    <div className={styles.skillsPage}>
      <div className={styles.pageHeader}>
        <Title level={3} style={{ margin: 0 }}>
          {t('skills.title')}
        </Title>
        <Button
          type="text"
          icon={<UserOutlined />}
          onClick={() => setSettingsModalOpen(true)}
        >
          {t('skills.settings')}
        </Button>
      </div>

      <div className={styles.toolbar}>
        <Space size="middle">
          <Button
            type="link"
            icon={<PlusOutlined />}
            onClick={() => setAddModalOpen(true)}
          >
            {t('skills.addSkill')}
          </Button>
          {discoveredCount > 0 && (
            <Button
              type="text"
              icon={<ImportOutlined />}
              onClick={() => setImportModalOpen(true)}
            >
              {t('skills.importExisting')} ({discoveredCount})
            </Button>
          )}
        </Space>
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
    </div>
  );
};

export default SkillsPage;
