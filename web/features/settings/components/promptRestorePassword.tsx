import { Input, message, Modal, Typography } from 'antd';

/**
 * Ask for the one-shot decryption password and retry the same restore selection.
 * Cancel keeps the dialog closed and guarantees zero restore writes.
 */
export function promptRestorePassword(
  t: (key: string) => string,
  onSubmit: (password: string) => void,
  wrongAttempt: boolean,
): void {
  let passwordValue = '';
  Modal.confirm({
    title: t('settings.backupSettings.encryption.passwordPromptTitle'),
    content: (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {wrongAttempt && (
          <Typography.Text type="danger" style={{ fontSize: 12 }}>
            {t('settings.backupSettings.encryption.errors.passwordWrong')}
          </Typography.Text>
        )}
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {t('settings.backupSettings.encryption.passwordPromptDesc')}
        </Typography.Text>
        <Input.Password
          autoFocus
          onChange={(event) => {
            passwordValue = event.target.value;
          }}
        />
      </div>
    ),
    okText: t('common.confirm'),
    cancelText: t('common.cancel'),
    onOk: () => {
      if (!passwordValue) {
        message.warning(t('settings.backupSettings.encryption.passwordRequiredPrompt'));
        return Promise.reject();
      }
      onSubmit(passwordValue);
    },
  });
}
