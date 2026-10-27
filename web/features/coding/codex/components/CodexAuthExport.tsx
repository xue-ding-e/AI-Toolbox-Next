import React from 'react';
import { Alert, App, Button, Space } from 'antd';
import { Copy, Eye, EyeOff } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import PlainTextEditor from '@/components/common/PlainTextEditor';
import { exportCodexOfficialAccountAuthJson } from '@/services/codexApi';

interface Props {
  providerId: string;
  accountId: string;
}

/** Mounted only for the current, visible account detail context. */
const CodexAuthExport: React.FC<Props> = ({ providerId, accountId }) => {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [preview, setPreview] = React.useState<string | null>(null);
  const [pending, setPending] = React.useState<'preview' | 'copy' | null>(null);
  const request = React.useRef(0);
  const busy = React.useRef(false);
  React.useEffect(() => () => { request.current += 1; }, []);

  const generate = async (copy: boolean) => {
    if (busy.current) return;
    busy.current = true;
    const version = ++request.current;
    setPending(copy ? 'copy' : 'preview');
    // Do not leave a previous snapshot visible while generating a new one.
    setPreview(null);
    try {
      const content = await exportCodexOfficialAccountAuthJson(providerId, accountId, copy);
      if (request.current !== version) return;
      if (copy) message.success(t('common.copied'));
      else if (content !== null) setPreview(content);
    } catch {
      // Credentials must never reach logs/toasts, including backend error payloads.
      if (request.current === version) message.error(t('codex.provider.authExportFailed'));
    } finally {
      if (request.current === version) {
        busy.current = false;
        setPending(null);
      }
    }
  };

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Alert type="warning" showIcon title={t('codex.provider.authExportWarning')} />
      <Space wrap>
        <Button icon={<Eye size={14} />} loading={pending === 'preview'} disabled={pending !== null}
          onClick={() => void generate(false)}>
          {t('codex.provider.authExportPreview')}
        </Button>
        <Button icon={<Copy size={14} />} loading={pending === 'copy'} disabled={pending !== null}
          onClick={() => void generate(true)}>
          {t('codex.provider.authExportCopy')}
        </Button>
        {preview !== null && (
          <Button icon={<EyeOff size={14} />} onClick={() => setPreview(null)}>
            {t('codex.provider.authExportHide')}
          </Button>
        )}
      </Space>
      {preview !== null && <PlainTextEditor value={preview} language="plaintext" readOnly disposeModelOnUnmount />}
    </Space>
  );
};

export default CodexAuthExport;
