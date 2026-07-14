import React from 'react';
import { Button, Modal, Space, Typography, message } from 'antd';
import { CopyOutlined, LinkOutlined } from '@ant-design/icons';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';
import {
  cancelGrokOfficialAccountDeviceAuth,
  type GrokDeviceAuthStartResult,
} from '@/services/grokApi';

const { Text } = Typography;

interface GrokAuthStatusEvent {
  sessionId: string;
  status: string;
  message?: string;
  accountId?: string;
}

interface GrokDeviceAuthModalProps {
  authSession: GrokDeviceAuthStartResult | null;
  onClose: () => void;
  onCompleted: () => Promise<void>;
}

// Backend sequence: waiting_for_user -> authorized -> saving -> completed.
// Only "completed" means the account row is fully written and safe to reload.
const SUCCESS_STATUS = 'completed';
const TERMINAL_STATUSES = new Set([
  SUCCESS_STATUS,
  'cancelled',
  'expired',
  'denied',
  'access_denied',
  'failed',
  'error',
]);

const GrokDeviceAuthModal: React.FC<GrokDeviceAuthModalProps> = ({
  authSession,
  onClose,
  onCompleted,
}) => {
  const { t } = useTranslation();
  const [status, setStatus] = React.useState('waiting_for_user');
  const [remainingSeconds, setRemainingSeconds] = React.useState(0);
  const completedRef = React.useRef(false);

  React.useEffect(() => {
    if (!authSession) {
      setStatus('waiting_for_user');
      setRemainingSeconds(0);
      completedRef.current = false;
      return;
    }
    completedRef.current = false;
    const updateRemaining = () => {
      setRemainingSeconds(Math.max(0, authSession.expiresAt - Math.floor(Date.now() / 1000)));
    };
    updateRemaining();
    const timer = window.setInterval(updateRemaining, 1000);
    return () => window.clearInterval(timer);
  }, [authSession]);

  React.useEffect(() => {
    if (!authSession) return;
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    void listen<GrokAuthStatusEvent>('grok-auth-status', (event) => {
      if (disposed || event.payload.sessionId !== authSession.sessionId) return;
      setStatus(event.payload.status);
      if (event.payload.status === SUCCESS_STATUS) {
        if (completedRef.current) return;
        completedRef.current = true;
        message.success(t('grok.provider.officialAccountOauthSuccess'));
        void onCompleted();
      } else if (
        TERMINAL_STATUSES.has(event.payload.status)
        && event.payload.status !== 'cancelled'
        && event.payload.status !== SUCCESS_STATUS
      ) {
        message.error(event.payload.message || t('common.error'));
      }
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [authSession, onCompleted, t]);

  const handleClose = async () => {
    if (authSession && !TERMINAL_STATUSES.has(status)) {
      await cancelGrokOfficialAccountDeviceAuth(authSession.sessionId).catch(() => undefined);
    }
    onClose();
  };

  return (
    <Modal
      open={Boolean(authSession)}
      title={t('grok.provider.deviceAuthTitle')}
      onCancel={() => void handleClose()}
      footer={<Button onClick={() => void handleClose()}>{t('common.cancel')}</Button>}
      width={560}
    >
      {authSession && (
        <Space direction="vertical" size={16} style={{ width: '100%' }}>
          <Text>{t('grok.provider.deviceAuthHint')}</Text>
          <Space.Compact block>
            <Button
              icon={<LinkOutlined />}
              onClick={() => void openUrl(authSession.verificationUriComplete || authSession.verificationUri)}
            >
              {t('grok.provider.deviceAuthOpenBrowser')}
            </Button>
            <Button
              icon={<CopyOutlined />}
              onClick={() => void navigator.clipboard.writeText(authSession.userCode).then(
                () => message.success(t('common.copied')),
              )}
            >
              {t('common.copy')}
            </Button>
          </Space.Compact>
          <Typography.Title level={3} copyable style={{ margin: 0, textAlign: 'center' }}>
            {authSession.userCode}
          </Typography.Title>
          <Text type="secondary">
            {t('grok.provider.deviceAuthStatus', { status, seconds: remainingSeconds })}
          </Text>
        </Space>
      )}
    </Modal>
  );
};

export default GrokDeviceAuthModal;
