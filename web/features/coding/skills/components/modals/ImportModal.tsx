import React from 'react';
import { Modal, Checkbox, Button, Empty, message, Spin, Tooltip, Dropdown } from 'antd';
import { WarningOutlined, FolderOpenOutlined, PlusOutlined } from '@ant-design/icons';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';
import { useSkillsStore } from '../../stores/skillsStore';
import * as api from '../../services/skillsApi';
import {
  isSkillExistsError,
  extractSkillName,
  showGitError,
  confirmBatchOverwrite,
} from '../../utils/errorHandlers';
import { syncSkillToTools } from '../../utils/syncHelpers';
import { refreshTrayMenu } from '@/services/appApi';
import styles from './ImportModal.module.less';
import addSkillStyles from './AddSkillModal.module.less';

interface ImportModalProps {
  open: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

export const ImportModal: React.FC<ImportModalProps> = ({
  open,
  onClose,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const { onboardingPlan, loadOnboardingPlan, toolStatus } = useSkillsStore();
  const [selected, setSelected] = React.useState<Set<string>>(new Set());
  const [selectedTools, setSelectedTools] = React.useState<string[]>([]);
  const [loading, setLoading] = React.useState(false);
  const [preferredTools, setPreferredTools] = React.useState<string[] | null>(null);

  // Track if we've initialized tools for this open session
  const toolsInitializedRef = React.useRef(false);

  React.useEffect(() => {
    loadOnboardingPlan();
    setSelected(new Set());
    // Load preferred tools
    api.getPreferredTools().then(setPreferredTools).catch(console.error);
  }, [loadOnboardingPlan]);

  // Reset initialized state when modal closes
  React.useEffect(() => {
    if (!open) {
      toolsInitializedRef.current = false;
    }
  }, [open]);

  // Get all tools for display
  const allTools = React.useMemo(() => {
    return toolStatus?.tools?.map((t) => ({
      id: t.key,
      label: t.label,
      installed: t.installed,
    })) || [];
  }, [toolStatus]);

  // Split tools based on preferred tools setting + selected tools
  const visibleTools = React.useMemo(() => {
    if (preferredTools && preferredTools.length > 0) {
      // If preferred tools are set, show those + any selected tools
      return allTools.filter((t) => preferredTools.includes(t.id) || selectedTools.includes(t.id));
    }
    // Otherwise show installed tools + any selected tools
    return allTools.filter((t) => t.installed || selectedTools.includes(t.id));
  }, [allTools, preferredTools, selectedTools]);

  // Hidden tools: everything not in visible list, sorted by installed first
  const hiddenTools = React.useMemo(() => {
    const hidden = preferredTools && preferredTools.length > 0
      ? allTools.filter((t) => !preferredTools.includes(t.id) && !selectedTools.includes(t.id))
      : allTools.filter((t) => !t.installed && !selectedTools.includes(t.id));
    // Sort: installed first
    return [...hidden].sort((a, b) => {
      if (a.installed === b.installed) return 0;
      return a.installed ? -1 : 1;
    });
  }, [allTools, preferredTools, selectedTools]);

  // Initialize selected tools based on preferredTools
  React.useEffect(() => {
    if (open && !toolsInitializedRef.current && preferredTools !== null) {
      if (preferredTools.length > 0) {
        setSelectedTools(preferredTools);
      } else {
        // preferredTools loaded but empty, use installed tools
        const installed = allTools.filter((t) => t.installed).map((t) => t.id);
        setSelectedTools(installed);
      }
      toolsInitializedRef.current = true;
    }
  }, [open, allTools, preferredTools]);

  const handleToolToggle = (toolId: string) => {
    setSelectedTools((prev) =>
      prev.includes(toolId)
        ? prev.filter((id) => id !== toolId)
        : [...prev, toolId]
    );
  };

  const groups = onboardingPlan?.groups || [];
  const allPaths = React.useMemo(() => {
    const paths: string[] = [];
    groups.forEach((g) => {
      g.variants.forEach((v) => {
        paths.push(v.path);
      });
    });
    return paths;
  }, [groups]);

  const handleToggle = (path: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  const handleSelectAll = () => {
    if (selected.size === allPaths.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(allPaths));
    }
  };

  const handleImport = async () => {
    if (selected.size === 0) return;

    setLoading(true);
    const selectedPaths = Array.from(selected);
    const skippedNames: string[] = [];
    let overwriteAll = false;

    try {
      for (let i = 0; i < selectedPaths.length; i++) {
        const path = selectedPaths[i];
        let result;
        try {
          result = await api.importExistingSkill(path);
        } catch (error) {
          const errMsg = String(error);
          if (isSkillExistsError(errMsg)) {
            const skillName = extractSkillName(errMsg);
            if (overwriteAll) {
              result = await api.importExistingSkill(path, true);
            } else {
              const hasMore = i < selectedPaths.length - 1;
              const action = await confirmBatchOverwrite(skillName, hasMore, t);
              if (action === 'overwrite') {
                result = await api.importExistingSkill(path, true);
              } else if (action === 'overwriteAll') {
                overwriteAll = true;
                result = await api.importExistingSkill(path, true);
              } else {
                skippedNames.push(skillName);
                continue;
              }
            }
          } else {
            throw error;
          }
        }

        // Sync to target tools after successful import
        if (result && selectedTools.length > 0) {
          await syncSkillToTools({
            skillId: result.skill_id,
            centralPath: result.central_path,
            skillName: result.name,
            selectedTools: selectedTools,
            allTools,
            t,
            onTargetExists: 'skip',
          });
        }
      }

      if (skippedNames.length > 0) {
        message.info(t('skills.status.installWithSkipped', { skipped: skippedNames.join(', ') }));
      } else {
        message.success(t('skills.status.importCompleted'));
      }
      onSuccess();
      refreshTrayMenu();
    } catch (error) {
      showGitError(String(error), t, allTools);
    } finally {
      setLoading(false);
    }
  };

  const handleOpenFolder = async (path: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await revealItemInDir(path);
    } catch (error) {
      message.error(String(error));
    }
  };

  return (
    <Modal
      title={t('skills.importTitle')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={600}
    >
      <Spin spinning={loading}>
        <p className={styles.hint}>{t('skills.importSummary')}</p>

        {onboardingPlan && (
          <div className={styles.stats}>
            <span>{t('skills.toolsScanned', { count: onboardingPlan.total_tools_scanned })}</span>
            <span className={styles.dot}>•</span>
            <span>{t('skills.skillsFound', { count: onboardingPlan.total_skills_found })}</span>
          </div>
        )}

        {groups.length === 0 ? (
          <Empty description={t('skills.discoveredEmpty')} />
        ) : (
          <>
            <div className={styles.selectAll}>
              <Checkbox
                checked={selected.size === allPaths.length}
                indeterminate={selected.size > 0 && selected.size < allPaths.length}
                onChange={handleSelectAll}
              >
                {t('skills.selectAll')}
              </Checkbox>
              <span className={styles.count}>
                {t('skills.selectedCount', {
                  selected: selected.size,
                  total: allPaths.length,
                })}
              </span>
            </div>

            <div className={styles.list}>
              {groups.map((group) => (
                <div key={group.name} className={styles.group}>
                  <div className={styles.groupHeader}>
                    <span className={styles.groupName}>{group.name}</span>
                  </div>
                  {group.variants.map((v) => (
                    <div
                      key={v.path}
                      className={`${styles.variant} ${selected.has(v.path) ? styles.selected : ''}`}
                      onClick={() => handleToggle(v.path)}
                    >
                      <Checkbox checked={selected.has(v.path)} />
                      <div className={styles.variantInfo}>
                        <div className={styles.variantTool}>
                          {v.tool}
                          {v.conflicting_tools && v.conflicting_tools.length > 0 && (
                            <Tooltip title={t('skills.conflictWith', { tools: v.conflicting_tools.join(', ') })}>
                              <span className={styles.conflictBadge}>
                                <WarningOutlined /> {v.conflicting_tools.join(', ')}
                              </span>
                            </Tooltip>
                          )}
                        </div>
                        <div className={styles.variantPath}>
                          <span>
                            {v.is_link
                              ? t('skills.linkLabel', { target: v.link_target || v.path })
                              : v.path}
                          </span>
                          <FolderOpenOutlined
                            className={styles.openFolder}
                            onClick={(e) => handleOpenFolder(v.path, e)}
                          />
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ))}
            </div>

            <div className={addSkillStyles.toolsSection}>
              <div className={addSkillStyles.toolsLabel}>{t('skills.syncToTools')}</div>
              <div className={addSkillStyles.toolsHint}>{t('skills.syncToToolsHint')}</div>
              <div className={addSkillStyles.toolsGrid}>
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
                  <span className={addSkillStyles.noTools}>{t('skills.noToolsInstalled')}</span>
                )}
                {hiddenTools.length > 0 && (
                  <Dropdown
                    trigger={['click']}
                    menu={{
                      items: hiddenTools.map((tool) => ({
                        key: tool.id,
                        disabled: !tool.installed,
                        label: (
                          <Checkbox
                            checked={selectedTools.includes(tool.id)}
                            disabled={!tool.installed}
                            onClick={(e) => e.stopPropagation()}
                          >
                            {tool.label}
                            {!tool.installed && (
                              <span className={addSkillStyles.notInstalledTag}> {t('skills.notInstalled')}</span>
                            )}
                          </Checkbox>
                        ),
                        onClick: () => {
                          if (tool.installed) {
                            handleToolToggle(tool.id);
                          }
                        },
                      })),
                    }}
                  >
                    <Button type="dashed" size="small" icon={<PlusOutlined />} />
                  </Dropdown>
                )}
              </div>
            </div>
          </>
        )}

        <div className={styles.footer}>
          <Button onClick={onClose}>{t('common.close')}</Button>
          <Button
            type="primary"
            onClick={handleImport}
            disabled={selected.size === 0}
            loading={loading}
          >
            {t('skills.importAndSync')}
          </Button>
        </div>
      </Spin>
    </Modal>
  );
};
