import React from 'react';
import { ConfigProvider, Spin, notification } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import enUS from 'antd/locale/en_US';
import { useAppStore, useSettingsStore } from '@/stores';
import { checkForUpdates, openExternalUrl } from '@/services';
import { listen } from '@tauri-apps/api/event';
import i18n from '@/i18n';

interface ProvidersProps {
  children: React.ReactNode;
}

const antdLocales = {
  'zh-CN': zhCN,
  'en-US': enUS,
};

export const Providers: React.FC<ProvidersProps> = ({ children }) => {
  const { language, isInitialized: appInitialized, initApp } = useAppStore();
  const { isInitialized: settingsInitialized, initSettings } = useSettingsStore();

  const isLoading = !appInitialized || !settingsInitialized;

  // Initialize app and settings on mount
  React.useEffect(() => {
    const init = async () => {
      await initApp();
      await initSettings();
    };
    init();
  }, [initApp, initSettings]);

  // Listen for config changes from tray menu
  React.useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<string>('config-changed', async (event) => {
          const configType = event.payload;
          // 只有托盘菜单修改配置时才刷新页面
          if (configType === 'tray') {
            // Full page reload for tray menu config change
            window.location.reload();
          }
          // 其他情况（如主窗口修改）不刷新页面，只刷新托盘菜单
          // 托盘菜单会在后端自动刷新
        });
      } catch (error) {
        console.error('Failed to setup config change listener:', error);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Sync i18n language when app language changes
  React.useEffect(() => {
    if (appInitialized && i18n.language !== language) {
      i18n.changeLanguage(language);
    }
  }, [language, appInitialized]);

  // Check for updates on app startup
  React.useEffect(() => {
    if (!appInitialized) return;

    const checkUpdate = async () => {
      try {
        const info = await checkForUpdates();
        if (info.hasUpdate) {
          notification.info({
            message: i18n.t('settings.about.newVersion'),
            description: i18n.t('settings.about.updateAvailable', { version: info.latestVersion }),
            btn: (
              <a
                onClick={() => {
                  openExternalUrl(info.releaseUrl);
                  notification.destroy();
                }}
                style={{ cursor: 'pointer' }}
              >
                {i18n.t('settings.about.goToDownload')}
              </a>
            ),
            duration: 10,
          });
        }
      } catch (error) {
        console.error('Auto check update failed:', error);
      }
    };

    checkUpdate();
  }, [appInitialized]);

  if (isLoading) {
    return (
      <div
        style={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          height: '100vh',
          width: '100vw',
        }}
      >
        <Spin size="large" />
      </div>
    );
  }

  return (
    <ConfigProvider
      locale={antdLocales[language]}
      theme={{
        token: {
          colorPrimary: '#1890ff',
        },
      }}
    >
      {children}
    </ConfigProvider>
  );
};
