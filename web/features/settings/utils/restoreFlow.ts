interface RestoreSuccessDialog<Content> {
  title: string;
  content: Content;
  okText: string;
  closable: false;
  maskClosable: false;
  keyboard: false;
  onOk: () => void;
}

interface RestoreFlowAdapters<Result, Content> {
  setLoading: (loading: boolean) => void;
  translate: (key: string) => string;
  buildSuccessContent: (result: Result) => Content;
  showSuccess: (dialog: RestoreSuccessDialog<Content>) => void;
  showWarnings: (result: Result) => void;
  promptPassword: (onSubmit: (password: string) => void, wrongAttempt: boolean) => void;
  restart: () => void;
  logError: (label: string, error: unknown) => void;
  showError: (message: string) => void;
}

const parseRestoreError = (error: unknown, t: (key: string) => string): { type?: string; message: string } => {
  const fallback = t('settings.backupSettings.restoreFailed');
  try {
    const errorObj = JSON.parse(String(error));
    const messageText = errorObj.suggestion
      ? `${fallback}: ${t(errorObj.suggestion)}`
      : `${fallback}: ${String(error)}`;
    return { type: errorObj.type, message: messageText };
  } catch {
    return { message: `${fallback}: ${String(error)}` };
  }
};

/** Run the page's restore flow without coupling orchestration to React or Modal. */
export async function executeRestoreFlow<Result, Content>(
  restoreFn: (restorePassword?: string) => Promise<Result>,
  adapters: RestoreFlowAdapters<Result, Content>,
  restorePassword?: string,
): Promise<void> {
  const t = adapters.translate;
  adapters.setLoading(true);
  try {
    const restoreResult = await restoreFn(restorePassword);
    // A successful restore requires restarting before using stale in-memory state.
    adapters.showSuccess({
      title: t('settings.backupSettings.restoreSuccess'),
      content: adapters.buildSuccessContent(restoreResult),
      okText: t('common.restart'),
      closable: false,
      maskClosable: false,
      keyboard: false,
      onOk: () => {
        adapters.restart();
      },
    });
    adapters.showWarnings(restoreResult);
  } catch (error) {
    const parsed = parseRestoreError(error, t);
    if (parsed.type === 'passwordRequired' || parsed.type === 'passwordWrong') {
      // Nothing has been written yet: ask for the password and retry the same file.
      adapters.promptPassword(
        (password) => {
          void executeRestoreFlow(restoreFn, adapters, password);
        },
        parsed.type === 'passwordWrong',
      );
      return;
    }
    adapters.logError('Restore failed:', error);
    adapters.showError(parsed.message);
  } finally {
    // Preserve per-attempt loading, including when a password prompt is left open.
    adapters.setLoading(false);
  }
}
