import React from 'react';
import { Modal } from 'antd';
import { Plus, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { hashTagColorIndex, normalizeTagList } from '../../utils/skillTags';
import cardStyles from '../SkillCard.module.less';
import styles from './BatchTagDialog.module.less';

// Palette classes come from the SkillCard stylesheet so the tag color set has
// a single definition (kept in sync with TAG_COLOR_COUNT in ../../utils/skillTags).
const TAG_COLOR_CLASS_NAMES: readonly string[] = [
  cardStyles.tagColor0,
  cardStyles.tagColor1,
  cardStyles.tagColor2,
  cardStyles.tagColor3,
  cardStyles.tagColor4,
  cardStyles.tagColor5,
  cardStyles.tagColor6,
  cardStyles.tagColor7,
];

interface BatchTagDialogProps {
  open: boolean;
  /** Number of enabled skills the tags will be appended to. */
  skillCount: number;
  /** Number of skipped skills (management_enabled=false). */
  skippedCount: number;
  /** Known tags from the whole list, used only for datalist autocomplete. */
  allTags: string[];
  applying: boolean;
  onCancel: () => void;
  onApply: (tags: string[]) => void;
}

export const BatchTagDialog: React.FC<BatchTagDialogProps> = ({
  open,
  skillCount,
  skippedCount,
  allTags,
  applying,
  onCancel,
  onApply,
}) => {
  const { t } = useTranslation();
  const [draft, setDraft] = React.useState('');
  const [pendingTags, setPendingTags] = React.useState<string[]>([]);

  // Reset the draft list every time the dialog opens.
  React.useEffect(() => {
    if (open) {
      setDraft('');
      setPendingTags([]);
    }
  }, [open]);

  const commitDraft = React.useCallback(() => {
    const trimmed = draft.trim();
    if (!trimmed) {
      return;
    }
    setPendingTags((prev) => normalizeTagList([...prev, trimmed]));
    setDraft('');
  }, [draft]);

  const handleKeyDown = React.useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        commitDraft();
      }
    },
    [commitDraft],
  );

  const handleRemoveTag = React.useCallback((tag: string) => {
    setPendingTags((prev) => prev.filter((item) => item !== tag));
  }, []);

  const handleOk = React.useCallback(() => {
    if (pendingTags.length === 0) {
      return;
    }
    onApply(pendingTags);
  }, [onApply, pendingTags]);

  return (
    <Modal
      title={t('skills.tags.batchTitle')}
      open={open}
      onCancel={onCancel}
      onOk={handleOk}
      okText={t('common.apply')}
      cancelText={t('common.cancel')}
      confirmLoading={applying}
      okButtonProps={{ disabled: pendingTags.length === 0 }}
      destroyOnHidden
      width={440}
    >
      <div className={styles.content}>
        <p className={styles.hint}>
          {t('skills.tags.batchHint', { count: skillCount })}
          {skippedCount > 0 && (
            <>
              {' '}
              {t('skills.tags.batchSkippedHint', { skipped: skippedCount })}
            </>
          )}
        </p>

        {pendingTags.length > 0 && (
          <div className={styles.tagRow}>
            {pendingTags.map((tag) => (
              <span
                key={tag}
                className={`${styles.tagPill} ${TAG_COLOR_CLASS_NAMES[hashTagColorIndex(tag)]}`}
              >
                <span className={styles.tagPillText}>{tag}</span>
                <button
                  type="button"
                  className={styles.tagRemoveBtn}
                  title={t('skills.tags.remove')}
                  aria-label={`${t('skills.tags.remove')}: ${tag}`}
                onClick={() => handleRemoveTag(tag)}
              >
                <X size={10} aria-hidden="true" />
              </button>
              </span>
            ))}
          </div>
        )}

        <div className={styles.tagInputRow}>
          <input
            className={styles.tagInput}
            list="skills-batch-tag-options"
            value={draft}
            placeholder={t('skills.tags.addPlaceholder')}
            aria-label={t('skills.tags.addPlaceholder')}
            disabled={applying}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={handleKeyDown}
            onBlur={commitDraft}
          />
          <datalist id="skills-batch-tag-options">
            {allTags.map((tag) => (
              <option key={tag} value={tag} />
            ))}
          </datalist>
          <button
            type="button"
            className={styles.tagAddBtn}
            title={t('skills.tags.add')}
            aria-label={t('skills.tags.add')}
            disabled={applying || draft.trim().length === 0}
            onClick={commitDraft}
          >
            <Plus size={13} aria-hidden="true" />
          </button>
        </div>
      </div>
    </Modal>
  );
};