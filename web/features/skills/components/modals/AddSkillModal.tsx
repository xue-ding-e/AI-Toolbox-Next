import React from 'react';
import { Modal, Tabs, Input, Button, Checkbox, Space, message, Spin, Dropdown, AutoComplete } from 'antd';
import { FolderOutlined, GithubOutlined, DownOutlined } from '@ant-design/icons';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import * as api from '../../services/skillsApi';
import type { ToolOption, GitSkillCandidate } from '../../types';
import { GitPickModal } from './GitPickModal';
import { formatGitError, isGitError } from '../../utils/gitErrorParser';
import styles from './AddSkillModal.module.less';

interface AddSkillModalProps {
  open: boolean;
  onClose: () => void;
  allTools: ToolOption[];
  onSuccess: () => void;
}

export const AddSkillModal: React.FC<AddSkillModalProps> = ({
  open: isOpen,
  onClose,
  allTools,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = React.useState<'local' | 'git'>('local');
  const [localPath, setLocalPath] = React.useState('');
  const [gitUrl, setGitUrl] = React.useState('');
  const [gitBranch, setGitBranch] = React.useState('');
  const [selectedTools, setSelectedTools] = React.useState<string[]>([]);
  const [loading, setLoading] = React.useState(false);

  // Branch options for AutoComplete
  const branchOptions = [
    { value: 'main' },
    { value: 'master' },
  ];

  // Git pick modal state
  const [gitCandidates, setGitCandidates] = React.useState<GitSkillCandidate[]>([]);
  const [showGitPick, setShowGitPick] = React.useState(false);

  // Split tools: show installed + selected uninstalled in main list
  const visibleTools = React.useMemo(() => {
    return allTools.filter((t) => t.installed || selectedTools.includes(t.id));
  }, [allTools, selectedTools]);

  // Hidden tools: uninstalled and not selected
  const hiddenTools = React.useMemo(() => {
    return allTools.filter((t) => !t.installed && !selectedTools.includes(t.id));
  }, [allTools, selectedTools]);

  // Initialize selected tools with all installed tools
  React.useEffect(() => {
    if (isOpen) {
      const installed = allTools.filter((t) => t.installed).map((t) => t.id);
      setSelectedTools(installed);
    }
  }, [isOpen, allTools]);

  const handleBrowse = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t('skills.selectLocalFolder'),
    });
    if (selected && typeof selected === 'string') {
      setLocalPath(selected);
    }
  };

  const handleToolToggle = (toolId: string) => {
    setSelectedTools((prev) =>
      prev.includes(toolId)
        ? prev.filter((id) => id !== toolId)
        : [...prev, toolId]
    );
  };

  const syncToTools = async (skillId: string, centralPath: string, skillName: string) => {
    for (const toolId of selectedTools) {
      try {
        await api.syncSkillToTool(centralPath, skillId, toolId, skillName);
      } catch (error) {
        const errMsg = String(error);
        if (errMsg.startsWith('TARGET_EXISTS|')) {
          console.warn(`Skipping ${toolId}: target exists`);
        } else {
          console.error(`Failed to sync to ${toolId}:`, error);
        }
      }
    }
  };

  const isSkillExistsError = (errMsg: string) => errMsg.startsWith('SKILL_EXISTS|');

  // Helper function to show error messages
  const showError = (errMsg: string) => {
    if (isGitError(errMsg)) {
      // For git errors, show a modal with detailed info
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

  const doLocalInstall = async (overwrite: boolean) => {
    setLoading(true);
    try {
      const result = await api.installLocalSkill(localPath, overwrite);
      if (selectedTools.length > 0) {
        await syncToTools(result.skill_id, result.central_path, result.name);
      }
      message.success(t('skills.status.localSkillCreated'));
      onSuccess();
      resetForm();
    } catch (error) {
      const errMsg = String(error);
      if (!overwrite && isSkillExistsError(errMsg)) {
        confirmOverwrite(() => doLocalInstall(true));
      } else {
        showError(errMsg);
      }
    } finally {
      setLoading(false);
    }
  };

  const doGitInstall = async (overwrite: boolean) => {
    setLoading(true);
    try {
      const candidates = await api.listGitSkills(gitUrl, gitBranch || undefined);
      if (candidates.length > 1) {
        setGitCandidates(candidates);
        setShowGitPick(true);
        setLoading(false);
        return;
      }

      const result = await api.installGitSkill(gitUrl, gitBranch || undefined, overwrite);
      if (selectedTools.length > 0) {
        await syncToTools(result.skill_id, result.central_path, result.name);
      }
      message.success(t('skills.status.gitSkillCreated'));
      onSuccess();
      resetForm();
    } catch (error) {
      const errMsg = String(error);
      if (!overwrite && isSkillExistsError(errMsg)) {
        confirmOverwrite(() => doGitInstall(true));
      } else if (errMsg.startsWith('MULTI_SKILLS|')) {
        try {
          const candidates = await api.listGitSkills(gitUrl, gitBranch || undefined);
          if (candidates.length > 0) {
            setGitCandidates(candidates);
            setShowGitPick(true);
          } else {
            message.error(t('skills.errors.noSkillsFoundInRepo'));
          }
        } catch (listError) {
          showError(String(listError));
        }
      } else {
        showError(errMsg);
      }
    } finally {
      setLoading(false);
    }
  };

  const confirmOverwrite = (onOk: () => void) => {
    Modal.confirm({
      title: t('skills.overwrite.title'),
      content: t('skills.overwrite.message'),
      okText: t('skills.overwrite.confirm'),
      okType: 'danger',
      cancelText: t('common.cancel'),
      onOk,
    });
  };

  const handleLocalInstall = () => {
    if (!localPath.trim()) {
      message.error(t('skills.errors.requireLocalPath'));
      return;
    }
    doLocalInstall(false);
  };

  const handleGitInstall = () => {
    if (!gitUrl.trim()) {
      message.error(t('skills.errors.requireGitUrl'));
      return;
    }
    doGitInstall(false);
  };

  const handleGitPickConfirm = async (selections: { subpath: string }[]) => {
    setShowGitPick(false);
    setLoading(true);

    try {
      for (const sel of selections) {
        try {
          const result = await api.installGitSelection(gitUrl, sel.subpath, gitBranch || undefined);
          if (selectedTools.length > 0) {
            await syncToTools(result.skill_id, result.central_path, result.name);
          }
        } catch (error) {
          const errMsg = String(error);
          if (isSkillExistsError(errMsg)) {
            // Auto-overwrite for batch selections
            const result = await api.installGitSelection(gitUrl, sel.subpath, gitBranch || undefined, true);
            if (selectedTools.length > 0) {
              await syncToTools(result.skill_id, result.central_path, result.name);
            }
          } else {
            throw error;
          }
        }
      }
      message.success(t('skills.status.selectedSkillsInstalled'));
      onSuccess();
      resetForm();
    } catch (error) {
      showError(String(error));
    } finally {
      setLoading(false);
    }
  };

  const resetForm = () => {
    setLocalPath('');
    setGitUrl('');
    setGitBranch('');
    setGitCandidates([]);
  };

  const handleClose = () => {
    resetForm();
    onClose();
  };

  return (
    <>
      <Modal
        title={t('skills.addSkillTitle')}
        open={isOpen}
        onCancel={handleClose}
        footer={null}
        width={700}
        destroyOnClose
      >
        <Spin spinning={loading}>
          <Tabs
            activeKey={activeTab}
            onChange={(key) => setActiveTab(key as 'local' | 'git')}
            items={[
              {
                key: 'local',
                label: (
                  <span>
                    <FolderOutlined /> {t('skills.localTab')}
                  </span>
                ),
                children: (
                  <div className={styles.tabContent}>
                    <div className={styles.field}>
                      <label>{t('skills.addLocal.pathLabel')}</label>
                      <div className={styles.fieldInput}>
                        <Space.Compact style={{ width: '100%' }}>
                          <Input
                            value={localPath}
                            onChange={(e) => setLocalPath(e.target.value)}
                            placeholder={t('skills.addLocal.pathPlaceholder')}
                          />
                          <Button onClick={handleBrowse}>{t('common.browse')}</Button>
                        </Space.Compact>
                      </div>
                    </div>
                  </div>
                ),
              },
              {
                key: 'git',
                label: (
                  <span>
                    <GithubOutlined /> {t('skills.gitTab')}
                  </span>
                ),
                children: (
                  <div className={styles.tabContent}>
                    <div className={styles.field}>
                      <label>{t('skills.addGit.urlLabel')}</label>
                      <div className={styles.fieldInput}>
                        <Input
                          value={gitUrl}
                          onChange={(e) => setGitUrl(e.target.value)}
                          placeholder={t('skills.addGit.urlPlaceholder')}
                        />
                      </div>
                    </div>
                    <div className={styles.field}>
                      <label>{t('skills.addGit.branchLabel')}</label>
                      <div className={styles.fieldInput}>
                        <AutoComplete
                          value={gitBranch}
                          onChange={setGitBranch}
                          options={branchOptions}
                          placeholder={t('skills.addGit.branchPlaceholder')}
                          style={{ width: '100%' }}
                        />
                      </div>
                    </div>
                  </div>
                ),
              },
            ]}
          />

          <div className={styles.toolsSection}>
            <div className={styles.toolsLabel}>{t('skills.installToTools')}</div>
            <div className={styles.toolsHint}>{t('skills.syncAfterCreate')}</div>
            <div className={styles.toolsGrid}>
              {visibleTools.length > 0 ? (
                visibleTools.map((tool) => (
                  <Checkbox
                    key={tool.id}
                    checked={selectedTools.includes(tool.id)}
                    onChange={() => handleToolToggle(tool.id)}
                  >
                    {tool.label}
                  </Checkbox>
                ))
              ) : (
                <span className={styles.noTools}>{t('skills.noToolsInstalled')}</span>
              )}
              {hiddenTools.length > 0 && (
                <Dropdown
                  trigger={['click']}
                  menu={{
                    items: hiddenTools.map((tool) => ({
                      key: tool.id,
                      label: tool.label,
                      onClick: () => handleToolToggle(tool.id),
                    })),
                  }}
                >
                  <span className={styles.moreButton}>
                    {t('skills.more')} <DownOutlined />
                  </span>
                </Dropdown>
              )}
            </div>
          </div>

          <div className={styles.footer}>
            <Button onClick={handleClose}>{t('common.cancel')}</Button>
            <Button
              type="primary"
              onClick={activeTab === 'local' ? handleLocalInstall : handleGitInstall}
              loading={loading}
            >
              {t('skills.install')}
            </Button>
          </div>
        </Spin>
      </Modal>

      <GitPickModal
        open={showGitPick}
        candidates={gitCandidates}
        onClose={() => setShowGitPick(false)}
        onConfirm={handleGitPickConfirm}
      />
    </>
  );
};
