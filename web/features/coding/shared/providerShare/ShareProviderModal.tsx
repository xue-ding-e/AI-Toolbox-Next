import React from 'react';
import { Alert, Button, Descriptions, Input, Modal, Segmented, Tag, Typography, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { Copy } from 'lucide-react';
import {
  buildProviderShareUrl,
  extractProviderConnectionFields,
  maskApiKey,
  sanitizeShareHomepage,
  type ProviderShareApp,
} from '@/features/shared/deepLink/providerShareUrl';
import { importDeepLinkRequest } from '@/features/shared/deepLink/deeplinkImportAction';
import type { DeepLinkCategory, DeepLinkImportRequest } from '@/services/deeplinkApi';

const { Text, Link } = Typography;

/**
 * A provider subset the share modal needs. The per-tool provider types
 * (ClaudeCodeProvider / CodexProvider / GeminiCliProvider) all satisfy this
 * shape, so pages can pass their provider records directly.
 */
export interface ShareableProvider {
  name: string;
  category: string;
  settingsConfig: string;
  websiteUrl?: string;
  notes?: string;
  icon?: string;
  iconColor?: string;
}

export interface ShareProviderModalProps {
  open: boolean;
  /** Tool the provider to share belongs to; drives the field extraction. */
  sourceApp: ProviderShareApp;
  provider: ShareableProvider | null;
  onClose: () => void;
}

const SHARE_TARGET_APPS: ProviderShareApp[] = ['claude', 'codex', 'gemini'];

const APP_LABEL_KEYS: Record<ProviderShareApp, string> = {
  claude: 'common.deepLink.appClaude',
  codex: 'common.deepLink.appCodex',
  gemini: 'common.deepLink.appGemini',
};

const CATEGORY_COLORS: Record<string, string> = {
  official: 'blue',
  third_party: 'orange',
  custom: 'default',
};

/** Map a provider category onto the deep-link category value domain. */
const normalizeCategory = (category: string): DeepLinkCategory => {
  if (category === 'official' || category === 'third_party') return category;
  return 'custom';
};

const ShareProviderModal: React.FC<ShareProviderModalProps> = ({
  open,
  sourceApp,
  provider,
  onClose,
}) => {
  const { t } = useTranslation();
  const [targetApp, setTargetApp] = React.useState<ProviderShareApp>(sourceApp);
  const [importing, setImporting] = React.useState(false);

  // Re-open for a different provider always resets the target back to its own
  // tool, so a previous cross-tool choice never leaks into the next share.
  React.useEffect(() => {
    if (open) {
      setTargetApp(sourceApp);
    }
  }, [open, sourceApp]);

  const connectionFields = React.useMemo(
    () => (provider ? extractProviderConnectionFields(sourceApp, provider.settingsConfig) : {}),
    [provider, sourceApp],
  );

  const shareUrl = React.useMemo(() => {
    if (!provider) return '';
    return buildProviderShareUrl({
      app: targetApp,
      name: provider.name,
      category: normalizeCategory(provider.category),
      ...connectionFields,
      homepage: provider.websiteUrl,
      notes: provider.notes,
      icon: provider.icon,
      iconColor: provider.iconColor,
    });
  }, [provider, targetApp, connectionFields]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(shareUrl);
      message.success(t('common.copied'));
    } catch (error) {
      console.error('Failed to copy share link:', error);
      message.error(t('common.error'));
    }
  };

  /**
   * Import the shared provider into the selected target tool on this device.
   * The share modal itself is the confirmation surface (the user has already
   * reviewed the masked preview and picked the target tool), so we build the
   * same request the backend parser would produce for the share URL and
   * persist it directly through the unified deep-link import command.
   */
  const handleImportToLocal = async () => {
    if (!provider) return;
    setImporting(true);
    try {
      const request: DeepLinkImportRequest = {
        resource: 'provider',
        app: targetApp,
        name: provider.name,
        category: normalizeCategory(provider.category),
        apiKey: connectionFields.apiKey,
        baseUrl: connectionFields.baseUrl,
        model: connectionFields.model,
        homepage: sanitizeShareHomepage(provider.websiteUrl),
        notes: provider.notes,
        icon: provider.icon,
        iconColor: provider.iconColor,
        rawUrl: shareUrl,
      };
      await importDeepLinkRequest(request);
      message.success(t('common.deepLink.importSuccess'));
      onClose();
    } catch (error) {
      console.error('Failed to import shared provider:', error);
      const detail = error instanceof Error ? error.message : String(error);
      message.error(`${t('common.deepLink.importFailed')}: ${detail}`);
    } finally {
      setImporting(false);
    }
  };

  return (
    <Modal
      title={t('common.deepLink.shareTitle')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={560}
    >
      <Text type="secondary" style={{ display: 'block', marginBottom: 12 }}>
        {t('common.deepLink.shareDescription')}
      </Text>

      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          marginBottom: 12,
        }}
      >
        <Text style={{ flexShrink: 0 }}>{t('common.deepLink.shareTargetTool')}</Text>
        <Segmented
          value={targetApp}
          onChange={(value) => setTargetApp(value as ProviderShareApp)}
          options={SHARE_TARGET_APPS.map((app) => ({
            value: app,
            label: t(APP_LABEL_KEYS[app]),
          }))}
        />
      </div>

      {provider && (
        <Descriptions size="small" column={1} bordered style={{ marginBottom: 12 }}>
          <Descriptions.Item label={t('common.deepLink.fieldName')}>
            <Text strong>{provider.name}</Text>
          </Descriptions.Item>
          <Descriptions.Item label={t('common.deepLink.fieldCategory')}>
            <Tag color={CATEGORY_COLORS[provider.category] ?? 'default'}>
              {t(`common.deepLink.category_${provider.category}`, provider.category)}
            </Tag>
          </Descriptions.Item>
          {connectionFields.apiKey && (
            <Descriptions.Item label={t('common.deepLink.fieldApiKey')}>
              <Text code>{maskApiKey(connectionFields.apiKey)}</Text>
            </Descriptions.Item>
          )}
          {connectionFields.baseUrl && (
            <Descriptions.Item label={t('common.deepLink.fieldBaseUrl')}>
              <Text code style={{ wordBreak: 'break-all' }}>
                {connectionFields.baseUrl}
              </Text>
            </Descriptions.Item>
          )}
          {connectionFields.model && (
            <Descriptions.Item label={t('common.deepLink.fieldModel')}>
              <Text code>{connectionFields.model}</Text>
            </Descriptions.Item>
          )}
          {provider.websiteUrl && (
            <Descriptions.Item label={t('common.deepLink.fieldHomepage')}>
              <Link
                href={provider.websiteUrl}
                target="_blank"
                rel="noopener noreferrer"
                style={{ wordBreak: 'break-all' }}
              >
                {provider.websiteUrl}
              </Link>
            </Descriptions.Item>
          )}
          {provider.notes && (
            <Descriptions.Item label={t('common.deepLink.fieldNotes')}>
              <Text>{provider.notes}</Text>
            </Descriptions.Item>
          )}
        </Descriptions>
      )}

      <Input.TextArea value={shareUrl} readOnly rows={3} style={{ marginBottom: 12 }} />

      <Alert
        type="warning"
        showIcon
        message={t('common.deepLink.shareWarning')}
        style={{ marginBottom: 12 }}
      />

      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
        <Button
          loading={importing}
          onClick={() => void handleImportToLocal()}
        >
          {t('common.deepLink.shareImportToLocal')}
        </Button>
        <Button
          type="primary"
          icon={<Copy size={14} />}
          onClick={() => void handleCopy()}
        >
          {t('common.copy')}
        </Button>
      </div>
    </Modal>
  );
};

export default ShareProviderModal;
