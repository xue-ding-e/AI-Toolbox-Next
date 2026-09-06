import type {
  RouteChromeConfig,
  RouteContentPadding,
  RouteEntry,
  RouteChromeMode,
} from './routeConfig';

export interface NormalizedRouteChrome {
  mode: RouteChromeMode;
  contentPadding: RouteContentPadding;
  ownerTabKey?: string;
  parentPath?: string;
}

export const DEFAULT_ROUTE_CHROME: NormalizedRouteChrome = {
  mode: 'default',
  contentPadding: 'default',
};

export interface TabPathEntry {
  key: string;
  path: string;
}

/**
 * Resolve the coding tab to restore on a cold boot.
 *
 * A rebuilt (lightweight-mode exit) or restarted app loads the webview at the
 * app root URL (`/index.html` via `WebviewUrl::default()`, or `/` on some
 * setups), which matches no configured route. The persisted `current_sub_tab`
 * in app settings holds the last active tab key; only trust it on such a
 * cold-boot path and only when the tab is still visible — anything else
 * returns null so the caller falls back to the first visible tab. This must
 * not fire for hidden-tab redirects, whose pathname always matches a route.
 */
export function resolveInitialTabPath(
  pathname: string,
  routes: ReadonlyArray<RouteEntry>,
  subTabs: ReadonlyArray<TabPathEntry>,
  savedTabKey: string | undefined | null,
): string | null {
  if (matchRouteEntry(routes, pathname) !== undefined || !savedTabKey) {
    return null;
  }

  return subTabs.find((tab) => tab.key === savedTabKey)?.path ?? null;
}

export function matchRouteEntry(routes: ReadonlyArray<RouteEntry>, pathname: string): RouteEntry | undefined {
  let bestMatch: RouteEntry | undefined;

  routes.forEach((route) => {
    const isMatch = pathname === route.path || pathname.startsWith(`${route.path}/`);
    if (isMatch && (!bestMatch || route.path.length > bestMatch.path.length)) {
      bestMatch = route;
    }
  });

  return bestMatch;
}

export function normalizeRouteChrome(chrome: RouteChromeConfig | undefined): NormalizedRouteChrome {
  const normalizedChrome: NormalizedRouteChrome = {
    mode: chrome?.mode ?? DEFAULT_ROUTE_CHROME.mode,
    contentPadding: chrome?.contentPadding ?? DEFAULT_ROUTE_CHROME.contentPadding,
  };

  if (chrome?.ownerTabKey) {
    normalizedChrome.ownerTabKey = chrome.ownerTabKey;
  }

  if (chrome?.parentPath) {
    normalizedChrome.parentPath = chrome.parentPath;
  }

  return normalizedChrome;
}

export function getRouteChrome(route: RouteEntry | undefined): NormalizedRouteChrome {
  return normalizeRouteChrome(route?.chrome);
}

export function shouldShowRouteAppHeader(chrome: NormalizedRouteChrome): boolean {
  return chrome.mode !== 'secondary';
}

export function getRouteScrollKey(
  route: RouteEntry | undefined,
  pathname: string,
  search: string,
): string {
  if (!route) {
    return `${pathname}${search}`;
  }

  const chrome = getRouteChrome(route);
  if (chrome.mode === 'secondary') {
    return `${route.path}${search}`;
  }

  return route.path;
}
