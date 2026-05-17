export type GatewayPageTab = 'statistics' | 'requests' | 'settings';

export interface GatewayTabConfig {
  key: GatewayPageTab;
  labelKey: string;
  path: string;
}

export const GATEWAY_TABS: GatewayTabConfig[] = [
  {
    key: 'statistics',
    labelKey: 'gateway.page.tabs.statistics',
    path: '/gateway/statistics',
  },
  {
    key: 'requests',
    labelKey: 'gateway.page.tabs.requests',
    path: '/gateway/requests',
  },
  {
    key: 'settings',
    labelKey: 'gateway.page.tabs.settings',
    path: '/gateway/settings',
  },
];

export const DEFAULT_GATEWAY_PATH = GATEWAY_TABS[0].path;

export const isGatewayPath = (pathname: string) =>
  pathname === '/gateway' || pathname.startsWith('/gateway/');

export const resolveGatewayTabFromPath = (pathname: string): GatewayPageTab => {
  const matchedTab = GATEWAY_TABS.find((tab) => pathname === tab.path || pathname.startsWith(`${tab.path}/`));
  return matchedTab?.key ?? 'statistics';
};

export const getGatewayPathForTab = (tabKey: GatewayPageTab) =>
  GATEWAY_TABS.find((tab) => tab.key === tabKey)?.path ?? DEFAULT_GATEWAY_PATH;
