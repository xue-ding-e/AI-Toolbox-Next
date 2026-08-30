import React from 'react';
import { Button, Modal, Space, Typography, message } from 'antd';
import { CopyOutlined, LinkOutlined } from '@ant-design/icons';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';
import {
  cancelKimiOfficialAccountDeviceAuth,
  getKimiOfficialAccountAuthStatus,
} from '@/services/kimiApi';
import type { KimiDeviceAuthStartResult, KimiAuthStatusEvent } from '@/types/kimi';
import {
  createKimiDeviceAuthStatusClassifier,
  isTerminalKimiDeviceAuthStatus,
} from '../utils/deviceAuthStatus';

const { Text } = Typography;

const DEVICE_AUTH_STATUS_TEXT_KEYS: Record<string, string> = {
  waiting: 'kimi.provider.deviceAuthStatusValue.waiting',
  completed: 'kimi.provider.deviceAuthStatusValue.completed',
  failed: 'kimi.provider.deviceAuthStatusValue.failed',
  expired: 'kimi.provider.deviceAuthStatusValue.expired',
  cancelled: 'kimi.provider.deviceAuthStatusValue.cancelled',
};

interface KimiDeviceAuthModalProps {
  authSession: KimiDeviceAuthStartResult | null;
  onClose: () => void;
  onCompleted: () => Promise<void>;
}

export const KimiDeviceAuthModal: React.FC<KimiDeviceAuthModalProps> = ({
  authSession,
  onClose,
  onCompleted,
}) => {
  const { t } = useTranslation();
  const [status, setStatus] = React.useState('waiting');
  const [remainingSeconds, setRemainingSeconds] = React.useState(0);
  // One-shot per auth session: both the event listener and the polling
  // fallback can observe the same terminal status, and polling keeps
  // running after a failure, so terminal feedback must be classified once.
  // The classifier lives in a ref so effect re-runs (the parent re-renders
  // with a fresh inline `onCompleted`) cannot reset the terminal latch and
  // replay a failure notification.
  const classifierRef = React.useRef<ReturnType<typeof createKimiDeviceAuthStatusClassifier>>(
    createKimiDeviceAuthStatusClassifier(),
  );
  React.useEffect(() => {
    classifierRef.current = createKimiDeviceAuthStatusClassifier();
  }, [authSession]);

  React.useEffect(() => {
    if (!authSession) {
      setStatus('waiting');
      setRemainingSeconds(0);
      return;
    }
    const updateRemaining = () => {
      setRemainingSeconds(Math.max(0, authSession.expiresAt - Math.floor(Date.now() / 1000)));
    };
    updateRemaining();
    const timer = window.setInterval(updateRemaining, 1000);
    return () => window.clearInterval(timer);
  }, [authSession]);

  // Parent passes an inline callback; keep it in a ref so re-renders cannot
  // re-run the status effect and reset the listener/polling mid-auth.
  const onCompletedRef = React.useRef(onCompleted);
  React.useEffect(() => {
    onCompletedRef.current = onCompleted;
  }, [onCompleted]);

  React.useEffect(() => {
    if (!authSession) return;
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    const classify = classifierRef.current;
    // Polling fallback: events can be missed (e.g. window reload during
    // auth); the backend status map stays queryable after cleanup. Honor the
    // interval the device-code response asked for (RFC 8628 `interval`).
    const pollIntervalMs = Math.max(3, authSession.pollIntervalSeconds || 5) * 1000;
    const poll = window.setInterval(() => {
      void getKimiOfficialAccountAuthStatus(authSession.sessionId)
        .then((polled) => handleStatus(polled))
        .catch(() => undefined);
    }, pollIntervalMs);
    const handleStatus = (next: string, messageText?: string) => {
      if (disposed) return;
      setStatus(next);
      const feedback = classify(next);
      if (feedback === 'progress') return;
      // Terminal: stop polling and drop the listener before notifying, so a
      // failure state cannot repeat its message on the next tick.
      window.clearInterval(poll);
      unlisten?.();
      if (feedback === 'notify-success') {
        message.success(t('kimi.officialAccount.loginSuccess'));
        void onCompletedRef.current();
      } else if (feedback === 'notify-error') {
        message.error(messageText || t('common.error'));
      }
    };
    void listen<KimiAuthStatusEvent>('kimi-auth-status', (event) => {
      if (event.payload.sessionId !== authSession.sessionId) return;
      handleStatus(event.payload.status, event.payload.message);
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    });
    return () => {
      disposed = true;
      unlisten?.();
      window.clearInterval(poll);
    };
  }, [authSession, t]);

  const handleClose = async () => {
    if (authSession && !isTerminalKimiDeviceAuthStatus(status)) {
      await cancelKimiOfficialAccountDeviceAuth(authSession.sessionId).catch(() => undefined);
    }
    onClose();
  };

  return (
    <Modal
      open={Boolean(authSession)}
      title={t('kimi.provider.deviceAuthTitle')}
      onCancel={() => void handleClose()}
      footer={<Button onClick={() => void handleClose()}>{t('common.cancel')}</Button>}
      width={560}
    >
      {authSession && (
        <Space direction="vertical" size={16} style={{ width: '100%' }}>
          <Text>{t('kimi.provider.deviceAuthHint')}</Text>
          <Space.Compact block>
            <Button
              icon={<LinkOutlined />}
              onClick={() => void openUrl(authSession.verificationUriComplete || authSession.verificationUri)}
            >
              {t('kimi.provider.deviceAuthOpenBrowser')}
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
            {t('kimi.provider.deviceAuthStatus', {
              status: DEVICE_AUTH_STATUS_TEXT_KEYS[status]
                ? t(DEVICE_AUTH_STATUS_TEXT_KEYS[status])
                : status,
              seconds: remainingSeconds,
            })}
          </Text>
        </Space>
      )}
    </Modal>
  );
};

export default KimiDeviceAuthModal;
