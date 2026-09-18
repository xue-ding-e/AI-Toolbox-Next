import React from 'react';
import { Button, Input, Modal, Tooltip, Typography, message } from 'antd';
import { Globe } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  closeMiniBrowser,
  getMiniBrowserCurrentUrl,
  isMiniBrowserOpen,
  normaliseMiniBrowserUrl,
  openMiniBrowser,
} from '@/services/miniBrowserApi';
import styles from './MiniBrowserButton.module.less';

/**
 * Header entry for the embedded browser.
 *
 * The window itself is a native webview created by the backend (see
 * `tauri/src/mini_browser.rs`), so this component only owns the "where to go"
 * prompt and the open/closed state.
 */
export const MiniBrowserButton: React.FC = () => {
  const { t } = useTranslation();
  const [open, setOpen] = React.useState(false);
  const [url, setUrl] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [windowOpen, setWindowOpen] = React.useState(false);

  // The browser window lives in the backend and can be closed from its own
  // title bar, so re-read the state whenever the prompt opens.
  const refresh = React.useCallback(async () => {
    try {
      const [isOpen, current] = await Promise.all([
        isMiniBrowserOpen(),
        getMiniBrowserCurrentUrl(),
      ]);
      setWindowOpen(isOpen);
      if (isOpen && current) {
        setUrl((previous) => (previous ? previous : current));
      }
    } catch {
      // Reading state is best-effort: the prompt still works without it.
    }
  }, []);

  const handleOpen = async () => {
    const normalised = normaliseMiniBrowserUrl(url);
    if (!normalised) {
      void message.warning(
        url.trim() ? t('miniBrowser.urlInvalid') : t('miniBrowser.urlRequired'),
      );
      return;
    }
    setBusy(true);
    try {
      await openMiniBrowser(normalised);
      setOpen(false);
    } catch (error) {
      void message.error(
        t('miniBrowser.openFailed', {
          error: error instanceof Error ? error.message : String(error),
        }),
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Tooltip title={t('miniBrowser.tooltip')}>
        <span className={styles.button} role="button" tabIndex={0}
          onClick={() => {
            setOpen(true);
            void refresh();
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault();
              setOpen(true);
              void refresh();
            }
          }}
        >
          <Globe className={styles.icon} size={14} />
          <span className={styles.text}>{t('miniBrowser.button')}</span>
        </span>
      </Tooltip>
      <Modal
        open={open}
        title={t('miniBrowser.title')}
        onCancel={() => setOpen(false)}
        destroyOnHidden
        footer={[
          windowOpen ? (
            <Button
              key="close-window"
              onClick={async () => {
                await closeMiniBrowser().catch(() => undefined);
                setWindowOpen(false);
              }}
            >
              {t('miniBrowser.closeWindow')}
            </Button>
          ) : null,
          <Button key="cancel" onClick={() => setOpen(false)}>
            {t('miniBrowser.cancel')}
          </Button>,
          <Button key="open" type="primary" loading={busy} onClick={handleOpen}>
            {t('miniBrowser.open')}
          </Button>,
        ]}
      >
        <Typography.Paragraph type="secondary" className={styles.hint}>
          {t('miniBrowser.hint')}
        </Typography.Paragraph>
        <Input
          autoFocus
          value={url}
          placeholder={t('miniBrowser.urlPlaceholder')}
          aria-label={t('miniBrowser.urlLabel')}
          onChange={(event) => setUrl(event.currentTarget.value)}
          onPressEnter={handleOpen}
        />
      </Modal>
    </>
  );
};

export default MiniBrowserButton;
