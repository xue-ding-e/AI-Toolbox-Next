import React from 'react';
import { Typography, Button, Space, Input, Select } from 'antd';
import { PlusOutlined, AppstoreOutlined, ImportOutlined, SearchOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useSkillsStore } from '../stores/skillsStore';
import { useSkillsHub } from '../hooks/useSkillsHub';
import { SkillsList } from '../components/SkillsList';
import { AddSkillModal } from '../components/modals/AddSkillModal';
import { ImportModal } from '../components/modals/ImportModal';
import { SkillsSettingsModal } from '../components/modals/SkillsSettingsModal';
import { DeleteConfirmModal } from '../components/modals/DeleteConfirmModal';
import { NewToolsModal } from '../components/modals/NewToolsModal';
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
    searchQuery,
    setSearchQuery,
    sortMode,
    setSortMode,
  } = useSkillsStore();

  const {
    skills,
    getFilteredSkills,
    getInstalledTools,
    getAllTools,
    formatRelative,
    getGithubInfo,
    getSkillSourceLabel,
    toggleToolSync,
    updateSkill,
    deleteSkill,
    refresh,
  } = useSkillsHub();

  const [deleteSkillId, setDeleteSkillId] = React.useState<string | null>(null);
  const [actionLoading, setActionLoading] = React.useState(false);

  // Initialize data on mount
  React.useEffect(() => {
    refresh();
  }, []);

  const installedTools = getInstalledTools();
  const allTools = getAllTools();
  const filteredSkills = getFilteredSkills();
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
    <div className={styles.skillsPage}>
      <div className={styles.pageHeader}>
        <Title level={3} style={{ margin: 0 }}>
          {t('skills.title')}
        </Title>
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
          <Input
            placeholder={t('skills.searchPlaceholder')}
            prefix={<SearchOutlined />}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            allowClear
            variant="borderless"
            className={styles.searchInput}
          />
          <Select
            value={sortMode}
            onChange={setSortMode}
            variant="borderless"
            options={[
              { value: 'updated', label: t('skills.sortByUpdated') },
              { value: 'name', label: t('skills.sortByName') },
            ]}
            className={styles.sortSelect}
          />
        </Space>
        <Button
          type="text"
          icon={<AppstoreOutlined />}
          onClick={() => setSettingsModalOpen(true)}
        >
          {t('skills.settings')}
        </Button>
      </div>

      <div className={styles.content}>
        <SkillsList
          skills={filteredSkills}
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
