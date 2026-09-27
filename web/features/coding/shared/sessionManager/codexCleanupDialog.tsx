import React from 'react';
import { Checkbox, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import type { CodexCleanupOptions } from './types';
import type { CodexCleanupDialogPlan } from './codexCleanupOptions';
import styles from './SessionManagerPanel.module.less';

const { Text } = Typography;

/**
 * Compose a delete confirmation body with the optional cleanup options.
 *
 * `onChoice` receives every selection change so the caller can read the latest
 * value in `onOk`: `Modal.confirm` renders its content once and the selection
 * lives inside the body component.
 */
export const withCodexCleanupContent = (
  plan: CodexCleanupDialogPlan | null,
  baseContent: React.ReactNode,
  onChoice: (choice: CodexCleanupOptions) => void,
): React.ReactNode => (
  <>
    {baseContent}
    {plan ? <CodexCleanupDialogBody plan={plan} onChange={onChoice} /> : null}
  </>
);

interface CodexCleanupDialogBodyProps {
  plan: CodexCleanupDialogPlan;
  onChange: (choice: CodexCleanupOptions) => void;
}

/**
 * The optional Codex residue cleanup inside a delete confirmation.
 *
 * `Modal.confirm` renders its content once, so the selection is owned here and
 * pushed out through `onChange`; the caller keeps the latest value in a plain
 * local and reads it in `onOk`.
 */
const CodexCleanupDialogBody: React.FC<CodexCleanupDialogBodyProps> = ({ plan, onChange }) => {
  const { t } = useTranslation();
  const [choice, setChoice] = React.useState<CodexCleanupOptions>(plan.choice);

  const update = (next: Partial<CodexCleanupOptions>) => {
    const merged = { ...choice, ...next };
    setChoice(merged);
    onChange(merged);
  };

  return (
    <div className={styles.cleanupOptions}>
      {plan.rows.map((row) => {
        const checked = row.kind === 'workspace' ? choice.removeWorkspace : choice.removeTrustEntry;
        return (
          <div key={row.kind} className={styles.cleanupRow}>
            <Checkbox
              checked={checked}
              onChange={(event) => {
                update(
                  row.kind === 'workspace'
                    ? { removeWorkspace: event.target.checked }
                    : { removeTrustEntry: event.target.checked },
                );
              }}
            >
              {t(row.labelKey, row.params)}
            </Checkbox>
            <Text className={styles.cleanupDetail}>{t(row.detailKey, row.params)}</Text>
          </div>
        );
      })}
    </div>
  );
};

export default CodexCleanupDialogBody;