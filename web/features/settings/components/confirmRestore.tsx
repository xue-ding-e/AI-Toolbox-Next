import type { ReactNode } from 'react';
import { Checkbox, Modal, Space, Typography } from 'antd';
import type { RestoreOptions, RestoreResult } from '@/services/backupApi';

const { Text } = Typography;

/** Local mutable holder so Modal.confirm content can toggle without reopening. */
const createSkipCliCustomRootsHolder = () => {
  const holder = { value: false };
  return holder;
};

const buildRestoreConfirmContent = (
  description: ReactNode,
  holder: { value: boolean },
  t: (key: string) => string
) => (
  <div>
    <div style={{ marginBottom: 12 }}>{description}</div>
    <Checkbox
      defaultChecked={false}
      onChange={(event) => {
        holder.value = event.target.checked;
      }}
    >
      <Space direction="vertical" size={0}>
        <span>{t('settings.backupSettings.skipCliCustomRoots')}</span>
        <Text type="secondary" style={{ fontSize: 12 }}>
          {t('settings.backupSettings.skipCliCustomRootsDesc')}
        </Text>
      </Space>
    </Checkbox>
  </div>
);

/** Confirm once, then keep this dialog's option holder in the retry closure. */
export function confirmRestore(
  t: (key: string) => string,
  description: ReactNode,
  restore: (options: RestoreOptions) => Promise<RestoreResult>,
  executeRestore: (restoreFn: (restorePassword?: string) => Promise<RestoreResult>) => void,
): void {
  const skipCliCustomRootsHolder = createSkipCliCustomRootsHolder();
  Modal.confirm({
    title: t('settings.backupSettings.confirmRestore'),
    content: buildRestoreConfirmContent(description, skipCliCustomRootsHolder, t),
    okText: t('common.confirm'),
    cancelText: t('common.cancel'),
    onOk: () => {
      // Keep onOk void/fire-and-forget. Read the same local holder on each
      // attempt, including password retries, rather than opening a new dialog.
      void executeRestore((restorePassword) => restore({
        skipCliCustomRoots: skipCliCustomRootsHolder.value,
        restorePassword,
      }));
    },
  });
}
