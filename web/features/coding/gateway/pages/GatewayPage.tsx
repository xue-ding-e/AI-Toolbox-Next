import React from 'react';
import { BarChart3, FileText, Network, Settings } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import GatewaySettingsPanel from '@/features/settings/pages/GatewaySettingsPanel';
import GatewayRequestsView from '../components/GatewayRequestsView';
import GatewayStatisticsView from '../components/GatewayStatisticsView';
import { joinClassNames } from '../utils/gatewayFormatters';
import {
  DEFAULT_GATEWAY_PATH,
  GATEWAY_TABS,
  getGatewayPathForTab,
  resolveGatewayTabFromPath,
  type GatewayPageTab,
} from '../utils/gatewayNavigation';
import styles from './GatewayPage.module.less';

const GatewayPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const activeTab = resolveGatewayTabFromPath(location.pathname);

  React.useEffect(() => {
    if (location.pathname === '/gateway') {
      navigate(DEFAULT_GATEWAY_PATH, { replace: true });
    }
  }, [location.pathname, navigate]);

  const handleTabChange = (tabKey: GatewayPageTab) => {
    navigate(getGatewayPathForTab(tabKey));
  };

  return (
    <div className={styles.gatewayPage}>
      <div className={styles.header}>
        <div className={styles.titleBlock}>
          <span className={styles.titleIcon}>
            <Network size={18} aria-hidden="true" />
          </span>
          <div>
            <h1>{t('gateway.page.title')}</h1>
            <p>{t('gateway.page.subtitle')}</p>
          </div>
        </div>
        <div className={styles.tabList} role="tablist" aria-label={t('gateway.page.title')}>
          {GATEWAY_TABS.map((tab) => (
            <button
              key={tab.key}
              type="button"
              role="tab"
              aria-selected={activeTab === tab.key}
              className={joinClassNames(styles.tabButton, activeTab === tab.key && styles.tabButtonActive)}
              onClick={() => handleTabChange(tab.key)}
            >
              {tab.key === 'statistics' ? <BarChart3 size={14} aria-hidden="true" /> : null}
              {tab.key === 'requests' ? <FileText size={14} aria-hidden="true" /> : null}
              {tab.key === 'settings' ? <Settings size={14} aria-hidden="true" /> : null}
              <span>{t(tab.labelKey)}</span>
            </button>
          ))}
        </div>
      </div>

      {activeTab === 'statistics' ? <GatewayStatisticsView /> : null}
      {activeTab === 'requests' ? <GatewayRequestsView /> : null}
      {activeTab === 'settings' ? <GatewaySettingsPanel showTitleBlock={false} /> : null}
    </div>
  );
};

export default GatewayPage;
