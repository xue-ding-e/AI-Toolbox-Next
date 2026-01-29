import React from 'react';
import { ConfigProvider, Spin, App, theme as antdTheme } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import enUS from 'antd/locale/en_US';
import { useAppStore, useSettingsStore } from '@/stores';
import { useThemeStore } from '@/stores/themeStore';
import { checkForUpdates, openExternalUrl, setWindowBackgroundColor } from '@/services';
import { listen } from '@tauri-apps/api/event';
import i18n from '@/i18n';

interface ProvidersProps {
  children: React.ReactNode;
}

const antdLocales = {
  'zh-CN': zhCN,
  'en-US': enUS,
};

/**
 * Inner component that uses App.useApp() to get theme-aware notification
 */
const AppInitializer: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { notification } = App.useApp();
  const hasCheckedUpdate = React.useRef(false);

  // Check for updates on app startup (at most once per hour)
  React.useEffect(() => {
    if (hasCheckedUpdate.current) return;
    hasCheckedUpdate.current = true;

    const LAST_CHECK_KEY = 'lastUpdateCheckTime';
    const now = Date.now();
    const lastCheck = Number(localStorage.getItem(LAST_CHECK_KEY) || '0');
    if (now - lastCheck < 3600000) return;

    const checkUpdate = async () => {
      try {
        const info = await checkForUpdates();
        localStorage.setItem(LAST_CHECK_KEY, String(now));
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
  }, [notification]);

  // Listen for config changes from tray menu
  React.useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<string>('config-changed', async (event) => {
          const configType = event.payload;
          if (configType === 'tray') {
            window.location.reload();
          }
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

  return <>{children}</>;
};

export const Providers: React.FC<ProvidersProps> = ({ children }) => {
  const { language, isInitialized: appInitialized, initApp } = useAppStore();
  const { isInitialized: settingsInitialized, initSettings } = useSettingsStore();
  const { mode, resolvedTheme, isInitialized: themeInitialized, initTheme, updateResolvedTheme } = useThemeStore();

  const isLoading = !appInitialized || !settingsInitialized || !themeInitialized;

  // Initialize app, settings and theme on mount
  React.useEffect(() => {
    const init = async () => {
      await initApp();
      await initSettings();
      await initTheme();
    };
    init();
  }, [initApp, initSettings, initTheme]);

  // Listen for system theme changes
  React.useEffect(() => {
    if (!themeInitialized) return;

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

    const handleChange = (e: MediaQueryListEvent) => {
      if (mode === 'system') {
        updateResolvedTheme(e.matches ? 'dark' : 'light');
      }
    };

    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }, [mode, themeInitialized, updateResolvedTheme]);

  // Apply data-theme attribute to document
  React.useEffect(() => {
    if (themeInitialized) {
      document.documentElement.setAttribute('data-theme', resolvedTheme);
    }
  }, [resolvedTheme, themeInitialized]);

  // Set window background color for macOS titlebar
  React.useEffect(() => {
    if (themeInitialized) {
      // Light theme: #ffffff, Dark theme: #1f1f1f
      const bgColor = resolvedTheme === 'dark' ? { r: 31, g: 31, b: 31 } : { r: 255, g: 255, b: 255 };
      setWindowBackgroundColor(bgColor.r, bgColor.g, bgColor.b).catch(console.error);
    }
  }, [resolvedTheme, themeInitialized]);

  // Sync i18n language when app language changes
  React.useEffect(() => {
    if (appInitialized && i18n.language !== language) {
      i18n.changeLanguage(language);
    }
  }, [language, appInitialized]);

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
        algorithm: resolvedTheme === 'dark' ? antdTheme.darkAlgorithm : antdTheme.defaultAlgorithm,
        token: {
          colorPrimary: '#1890ff',
        },
      }}
    >
      <App>
        <AppInitializer>
          {children}
        </AppInitializer>
      </App>
    </ConfigProvider>
  );
};
