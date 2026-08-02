import React from 'react';
import { Modal, Descriptions, Tag, Typography, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { router } from '@/app/routes';
import {
  importFromDeeplinkUnified,
  type DeepLinkApp,
  type DeepLinkImportRequest,
} from '@/services/deeplinkApi';
import { refreshTrayMenu } from '@/services/appApi';
import { DEEP_LINK_IMPORT_COMPLETED } from '@/constants/configEvents';

const { Text, Link } = Typography;

/**
 * Map a deep-link `app` to the router path of its CLI tab, so that after a
 * successful import we can switch to that tab and let the user see the result.
 * `grok` is mapped for completeness even though v1 rejects it at parse time.
 */
const APP_ROUTE_PATH: Record<DeepLinkApp, string> = {
  claude: '/coding/claudecode',
  codex: '/coding/codex',
  gemini: '/coding/geminicli',
};

/** Mask a secret: first 4 chars + 20 asterisks; values ≤ 4 chars fully masked. */
const maskApiKey = (value?: string): string => {
  if (!value) return '—';
  if (value.length <= 4) return '****';
  return `${value.slice(0, 4)}${'*'.repeat(20)}`;
};

const CATEGORY_COLORS: Record<string, string> = {
  official: 'blue',
  third_party: 'orange',
  custom: 'default',
};

const APP_LABEL_KEYS: Record<string, string> = {
  claude: 'common.deepLink.appClaude',
  codex: 'common.deepLink.appCodex',
  gemini: 'common.deepLink.appGemini',
};

export interface DeepLinkImportDialogProps {
  request: DeepLinkImportRequest | null;
  onDismiss: () => void;
}

const DeepLinkImportDialog: React.FC<DeepLinkImportDialogProps> = ({
  request,
  onDismiss,
}) => {
  const { t } = useTranslation();
  const [importing, setImporting] = React.useState(false);

  const open = request !== null;
  const app = request?.app;

  const handleImport = async () => {
    if (!request) return;
    setImporting(true);
    try {
      const result = await importFromDeeplinkUnified(request);
      // Switch to the imported tool's tab so the user sees the result. The
      // matching page (kept alive under KeepAliveOutlet) refreshes its
      // provider list on the dispatched event below; if it was never mounted,
      // navigating to it triggers its initial loadConfig on mount.
      const targetPath = APP_ROUTE_PATH[result.app];
      if (targetPath) {
        await router.navigate(targetPath);
      }
      // Notify the matching tool page (if kept-alive) to refresh its list.
      window.dispatchEvent(
        new CustomEvent(DEEP_LINK_IMPORT_COMPLETED, {
          detail: { app: result.app, id: result.id },
        }),
      );
      try {
        await refreshTrayMenu();
      } catch (trayError) {
        console.error('Failed to refresh tray menu after deep-link import:', trayError);
      }
      message.success(t('common.deepLink.importSuccess'));
      onDismiss();
    } catch (error) {
      console.error('Deep-link import failed:', error);
      const detail = error instanceof Error ? error.message : String(error);
      message.error(`${t('common.deepLink.importFailed')}: ${detail}`);
    } finally {
      setImporting(false);
    }
  };

  return (
    <Modal
      title={t('common.deepLink.title')}
      open={open}
      onOk={handleImport}
      onCancel={onDismiss}
      okText={t('common.deepLink.import')}
      cancelText={t('common.cancel')}
      okButtonProps={{ loading: importing }}
      closable={!importing}
      maskClosable={!importing}
    >
      {request && (
        <>
          <Text type="secondary" style={{ display: 'block', marginBottom: 16 }}>
            {t('common.deepLink.description')}
          </Text>
          <Descriptions size="small" column={1} bordered>
            <Descriptions.Item label={t('common.deepLink.fieldApp')}>
              <Tag color="purple">
                {app ? t(APP_LABEL_KEYS[app] ?? 'common.deepLink.appUnknown') : ''}
              </Tag>
            </Descriptions.Item>
            <Descriptions.Item label={t('common.deepLink.fieldName')}>
              <Text strong>{request.name}</Text>
            </Descriptions.Item>
            <Descriptions.Item label={t('common.deepLink.fieldCategory')}>
              <Tag color={CATEGORY_COLORS[request.category] ?? 'default'}>
                {t(`common.deepLink.category_${request.category}`, request.category)}
              </Tag>
            </Descriptions.Item>
            {request.apiKey && (
              <Descriptions.Item label={t('common.deepLink.fieldApiKey')}>
                <Text code>{maskApiKey(request.apiKey)}</Text>
              </Descriptions.Item>
            )}
            {request.baseUrl && (
              <Descriptions.Item label={t('common.deepLink.fieldBaseUrl')}>
                <Text code style={{ wordBreak: 'break-all' }}>
                  {request.baseUrl}
                </Text>
              </Descriptions.Item>
            )}
            {request.model && (
              <Descriptions.Item label={t('common.deepLink.fieldModel')}>
                <Text code>{request.model}</Text>
              </Descriptions.Item>
            )}
            {request.homepage && (
              <Descriptions.Item label={t('common.deepLink.fieldHomepage')}>
                <Link
                  href={request.homepage}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ wordBreak: 'break-all' }}
                >
                  {request.homepage}
                </Link>
              </Descriptions.Item>
            )}
            {request.notes && (
              <Descriptions.Item label={t('common.deepLink.fieldNotes')}>
                <Text>{request.notes}</Text>
              </Descriptions.Item>
            )}
          </Descriptions>
        </>
      )}
    </Modal>
  );
};

export default DeepLinkImportDialog;
