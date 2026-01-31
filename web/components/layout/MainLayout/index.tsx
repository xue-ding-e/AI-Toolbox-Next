import React from 'react';
import { Tabs } from 'antd';
import { useNavigate, useLocation, Outlet } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { CodeOutlined, SettingOutlined } from '@ant-design/icons';
import { platform } from '@tauri-apps/plugin-os';
import { MODULES } from '@/constants';
import { useAppStore } from '@/stores';
import { WSLStatusIndicator } from '@/features/settings/components/WSLStatusIndicator';
import { WSLSyncModal } from '@/features/settings/components/WSLSyncModal';
import { useWSLSync } from '@/features/settings/hooks/useWSLSync';
import { SkillsButton } from '@/features/coding/skills';
import { McpButton } from '@/features/coding/mcp';
import styles from './styles.module.less';

import OpencodeIcon from '@/assets/opencode.svg';
import ClaudeIcon from '@/assets/claude.svg';
import ChatgptIcon from '@/assets/chatgpt.svg';

const TAB_ICONS: Record<string, string> = {
  opencode: OpencodeIcon,
  claudecode: ClaudeIcon,
  codex: ChatgptIcon,
};

// macOS Overlay 模式需要为交通灯按钮预留空间，Windows/Linux 使用原生标题栏
const DRAG_BAR_HEIGHT = platform() === 'windows' || platform() === 'linux' ? 0 : 28; // px
const HEADER_HEIGHT = 56; // px
const CONTENT_TOP_OFFSET = DRAG_BAR_HEIGHT + HEADER_HEIGHT;

const MainLayout: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const { setCurrentModule, setCurrentSubTab } = useAppStore();
  const { config, status } = useWSLSync();

  // Check if current platform is Windows (only show WSL on Windows)
  const isWindows = React.useMemo(() => platform() === 'windows', []);

  // WSL modal state
  const [wslModalOpen, setWslModalOpen] = React.useState(false);

  // Listen for WSL settings open event
  React.useEffect(() => {
    const handleOpenWSLSettings = () => setWslModalOpen(true);
    window.addEventListener('open-wsl-settings', handleOpenWSLSettings);
    return () => {
      window.removeEventListener('open-wsl-settings', handleOpenWSLSettings);
    };
  }, []);

  const isSettingsPage = location.pathname.startsWith('/settings');
  const isSkillsPage = location.pathname.startsWith('/skills');
  const isMcpPage = location.pathname.startsWith('/mcp');
  const isNonTabPage = isSettingsPage || isSkillsPage || isMcpPage;

  // Get coding module's subTabs
  const codingModule = MODULES.find((m) => m.key === 'coding');
  const subTabs = codingModule?.subTabs || [];

  // Current active tab key
  const currentTabKey = React.useMemo(() => {
    for (const tab of subTabs) {
      if (location.pathname.startsWith(tab.path)) {
        return tab.key;
      }
    }
    return subTabs[0]?.key || '';
  }, [location.pathname, subTabs]);


  const handleTabChange = (key: string) => {
    const tab = subTabs.find((t) => t.key === key);
    if (tab) {
      setCurrentModule('coding');
      setCurrentSubTab(key);
      navigate(tab.path);
    }
  };

  const handleTabClick = (key: string) => {
    const tab = subTabs.find((t) => t.key === key);
    if (tab) {
      setCurrentModule('coding');
      setCurrentSubTab(key);
      navigate(tab.path);
    }
  };

  return (
    <div
      className={styles.layout}
      style={{ ['--content-top-offset' as any]: `${CONTENT_TOP_OFFSET}px` }}
    >
      {/* 全局拖拽区域（顶部 28px on macOS），避免上边框无法拖动 */}
      {DRAG_BAR_HEIGHT > 0 && (
        <div
          className={styles.dragBar}
          data-tauri-drag-region
          style={{ height: DRAG_BAR_HEIGHT }}
        >
          <img
            src="/tray-icon.png"
            alt="AI Toolbox"
            className={styles.dragBarIcon}
            data-tauri-drag-region
          />
        </div>
      )}

      {/* Header - 固定在顶部，带毛玻璃效果，包含拖拽区域高度 */}
      <header
        className={styles.header}
        data-tauri-drag-region
        style={{ top: 0, height: CONTENT_TOP_OFFSET, paddingTop: DRAG_BAR_HEIGHT }}
      >
        <div className={styles.headerContent} data-tauri-drag-region>
          {/* Left - Logo area */}
          <div className={styles.logoArea} style={{ WebkitAppRegion: 'no-drag' } as any}>
            <CodeOutlined className={styles.logoIcon} />
            <div className={styles.divider} />
          </div>

          {/* Center - Tabs */}
          <div className={styles.tabsArea} style={{ WebkitAppRegion: 'no-drag' } as any}>
            <div className={`${styles.tabsWrapper} ${isNonTabPage ? styles.noActiveTab : ''}`}>
              <Tabs
                activeKey={currentTabKey}
                onChange={handleTabChange}
                onTabClick={handleTabClick}
                indicator={{
                  size: (origin) => origin - 14,
                  align: 'center',
                }}
                items={subTabs.map((tab) => ({
                  key: tab.key,
                  label: (
                    <span className={styles.tabLabel}>
                      {TAB_ICONS[tab.key] && (
                        <img src={TAB_ICONS[tab.key]} className={styles.tabIcon} alt="" />
                      )}
                      <span>{t(tab.labelKey)}</span>
                    </span>
                  ),
                }))}
              />
            </div>
          </div>

          {/* Right - Actions */}
          <div className={styles.actionsArea} style={{ WebkitAppRegion: 'no-drag' } as any}>
            {/* WSL status indicator (Windows only) */}
            {isWindows && config && status && (
              <>
                <WSLStatusIndicator
                  enabled={config.enabled}
                  status={
                    status.lastSyncStatus === 'success'
                      ? 'success'
                      : status.lastSyncStatus === 'error'
                        ? 'error'
                        : 'idle'
                  }
                  wslAvailable={status.wslAvailable}
                  onClick={() => window.dispatchEvent(new CustomEvent('open-wsl-settings'))}
                />
                <div className={styles.actionsDivider} />
              </>
            )}

            {/* Skills button */}
            <SkillsButton />
            <div className={styles.actionsDivider} />

            {/* MCP button */}
            <McpButton />
            <div className={styles.actionsDivider} />

            {/* Settings button */}
            <div
              className={`${styles.settingsBtn} ${isSettingsPage ? styles.active : ''}`}
              onClick={() => navigate('/settings')}
            >
              <SettingOutlined className={styles.settingsIcon} />
              <span className={styles.settingsText}>{t('modules.settings')}</span>
            </div>
          </div>
        </div>
      </header>

      {/* Main content */}
      <main className={styles.main}>
        <div className={styles.contentArea}>
          <Outlet />
        </div>
      </main>

      {/* WSL Sync Modal - only render on Windows */}
      {isWindows && <WSLSyncModal open={wslModalOpen} onClose={() => setWslModalOpen(false)} />}
    </div>
  );
};

export default MainLayout;
