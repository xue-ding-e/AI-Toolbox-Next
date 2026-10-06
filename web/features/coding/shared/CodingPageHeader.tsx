import React from 'react';
import { Button, Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  EditOutlined,
  EllipsisOutlined,
  EyeOutlined,
  FolderOpenOutlined,
  LinkOutlined,
  SyncOutlined,
} from '@ant-design/icons';
import { openUrl } from '@tauri-apps/plugin-opener';

const { Text, Title, Link } = Typography;

export interface CodingPageHeaderProps {
  /** Page title, e.g. `t('codex.title')`. */
  title: React.ReactNode;
  /** Docs URL. The "official docs" link is hidden when omitted. */
  docsUrl?: string;
  /** Overrides the default `common.viewDocs` label. */
  docsText?: string;
  /** Shows the "preview config" link when provided. */
  onPreviewConfig?: () => void;

  /** Overrides the default `common.configPath` label. */
  configPathLabel?: string;
  /** Resolved config path; callers own their own fallback value. */
  configPath: string;
  /** Shows the "customize config dir" button when provided. */
  onCustomizeConfig?: () => void;
  /** Overrides the default `common.customizeConfigDir` label. */
  customizeConfigText?: string;
  customizeConfigDisabled?: boolean;
  /** Shows the "open folder" button when provided. */
  onOpenFolder?: () => void;
  /** Overrides the default `common.openFolder` label. */
  openFolderText?: string;
  /** Shows the "refresh config" button when provided. */
  onRefresh?: () => void;
  /** Overrides the default `common.refreshConfig` label. */
  refreshText?: string;

  /** Shows the "more options" button when provided. */
  onMoreOptions?: () => void;

  /** Extra text buttons appended to the config-path row (e.g. OpenClaw's "Open Web UI"). */
  extraActions?: React.ReactNode;
  /** Hint block rendered below the config-path row (e.g. OpenCode's page hint). */
  hint?: React.ReactNode;
}

const textButtonStyle: React.CSSProperties = { padding: 0, fontSize: 12 };

/**
 * Shared page header for every coding tab.
 *
 * Locks the header skeleton (title + docs/preview links, config path row with
 * customize/open/refresh, and the more-options entry) so newly onboarded CLIs
 * inherit the same layout instead of re-deriving it. Tool-specific wording is
 * passed through the override props; anything that is identical across tools
 * falls back to `common.*` keys.
 */
const CodingPageHeader: React.FC<CodingPageHeaderProps> = ({
  title,
  docsUrl,
  docsText,
  onPreviewConfig,
  configPathLabel,
  configPath,
  onCustomizeConfig,
  customizeConfigText,
  customizeConfigDisabled,
  onOpenFolder,
  openFolderText,
  onRefresh,
  refreshText,
  onMoreOptions,
  extraActions,
  hint,
}) => {
  const { t } = useTranslation();

  return (
    <div style={{ marginBottom: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div>
          <div style={{ marginBottom: 8 }}>
            <Title level={4} style={{ margin: 0, display: 'inline-block', marginRight: 8 }}>
              {title}
            </Title>
            {docsUrl && (
              <Link
                type="secondary"
                style={{ fontSize: 12 }}
                onClick={(event) => {
                  event.stopPropagation();
                  void openUrl(docsUrl);
                }}
              >
                <LinkOutlined /> {docsText ?? t('common.viewDocs')}
              </Link>
            )}
            {onPreviewConfig && (
              <Link
                type="secondary"
                style={{ fontSize: 12, marginLeft: 16 }}
                onClick={(event) => {
                  event.stopPropagation();
                  onPreviewConfig();
                }}
              >
                <EyeOutlined /> {t('common.previewConfig')}
              </Link>
            )}
          </div>
          <Space size="small">
            <Text type="secondary" style={{ fontSize: 12 }}>
              {configPathLabel ?? t('common.configPath')}:
            </Text>
            <Text code style={{ fontSize: 12 }}>
              {configPath}
            </Text>
            {onCustomizeConfig && (
              <Button
                type="text"
                size="small"
                icon={<EditOutlined />}
                onClick={onCustomizeConfig}
                disabled={customizeConfigDisabled}
                style={textButtonStyle}
              >
                {customizeConfigText ?? t('common.customizeConfigDir')}
              </Button>
            )}
            {onOpenFolder && (
              <Button
                type="text"
                size="small"
                icon={<FolderOpenOutlined />}
                onClick={onOpenFolder}
                style={textButtonStyle}
              >
                {openFolderText ?? t('common.openFolder')}
              </Button>
            )}
            {onRefresh && (
              <Button
                type="text"
                size="small"
                icon={<SyncOutlined />}
                onClick={onRefresh}
                style={textButtonStyle}
              >
                {refreshText ?? t('common.refreshConfig')}
              </Button>
            )}
            {extraActions}
          </Space>
        </div>

        {onMoreOptions && (
          <Space>
            <Button type="text" icon={<EllipsisOutlined />} onClick={onMoreOptions}>
              {t('common.moreOptions')}
            </Button>
          </Space>
        )}
      </div>

      {hint}
    </div>
  );
};

export default CodingPageHeader;
