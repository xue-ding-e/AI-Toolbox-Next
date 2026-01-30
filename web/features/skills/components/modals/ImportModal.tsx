import React from 'react';
import { Modal, Checkbox, Button, Empty, message, Spin } from 'antd';
import { useTranslation } from 'react-i18next';
import { useSkillsStore } from '../../stores/skillsStore';
import * as api from '../../services/skillsApi';
import styles from './ImportModal.module.less';

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
  const { onboardingPlan, loadOnboardingPlan } = useSkillsStore();
  const [selected, setSelected] = React.useState<Set<string>>(new Set());
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      loadOnboardingPlan();
      setSelected(new Set());
    }
  }, [open, loadOnboardingPlan]);

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
    try {
      for (const path of selected) {
        await api.importExistingSkill(path);
      }
      message.success(t('skills.status.importCompleted'));
      onSuccess();
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('skills.importTitle')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={600}
      destroyOnClose
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
                    {group.has_conflict && (
                      <span className={styles.conflictBadge}>{t('skills.conflict')}</span>
                    )}
                  </div>
                  {group.variants.map((v) => (
                    <div
                      key={v.path}
                      className={`${styles.variant} ${selected.has(v.path) ? styles.selected : ''}`}
                      onClick={() => handleToggle(v.path)}
                    >
                      <Checkbox checked={selected.has(v.path)} />
                      <div className={styles.variantInfo}>
                        <div className={styles.variantTool}>{v.tool}</div>
                        <div className={styles.variantPath}>
                          {v.is_link
                            ? t('skills.linkLabel', { target: v.link_target || v.path })
                            : v.path}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ))}
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
