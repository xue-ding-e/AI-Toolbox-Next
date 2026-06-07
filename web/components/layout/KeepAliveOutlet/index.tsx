import React from 'react';
import { useLocation } from 'react-router-dom';
import type { RouteEntry } from '@/app/routeConfig';
import { getRouteScrollKey, matchRouteEntry } from '@/app/routeMatching';

interface Props {
  routes: RouteEntry[];
  max?: number;
  scrollContainerRef?: React.RefObject<HTMLElement | null>;
}

interface KeepAliveContextValue {
  isActive: boolean;
  getScrollTop: () => number;
  rememberScrollPosition: () => number;
}

const KeepAliveContext = React.createContext<KeepAliveContextValue>({
  isActive: true,
  getScrollTop: () => 0,
  rememberScrollPosition: () => 0,
});

interface CachedRouteItemProps {
  path: string;
  component: RouteEntry['component'];
  isActive: boolean;
  getScrollTop: () => number;
  rememberScrollPosition: () => number;
}

/**
 * 页面组件可通过此 hook 感知当前是否处于活跃状态（可见）。
 * 典型用法：页面从隐藏切回可见时触发数据刷新。
 */
export const useKeepAlive = () => React.useContext(KeepAliveContext);

const CachedRouteItem: React.FC<CachedRouteItemProps> = React.memo(
  ({ component: Component, isActive, getScrollTop, rememberScrollPosition }) => {
    const contextValue = React.useMemo(() => ({
      isActive,
      getScrollTop,
      rememberScrollPosition,
    }), [getScrollTop, isActive, rememberScrollPosition]);

    return (
      <KeepAliveContext.Provider value={contextValue}>
        <div style={{ display: isActive ? undefined : 'none' }}>
          <Component />
        </div>
      </KeepAliveContext.Provider>
    );
  },
  (prevProps, nextProps) =>
    prevProps.path === nextProps.path
    && prevProps.component === nextProps.component
    && prevProps.isActive === nextProps.isActive
    && prevProps.getScrollTop === nextProps.getScrollTop
    && prevProps.rememberScrollPosition === nextProps.rememberScrollPosition,
);

/**
 * 基于 LRU 策略的路由组件缓存。
 * 已访问过的页面通过 display:none 隐藏而非卸载，
 * 切换回来时瞬间显示、无需重新加载数据。
 * 超出 max 上限时淘汰最久未访问的页面。
 */
const KeepAliveOutlet: React.FC<Props> = ({ routes, max = 10, scrollContainerRef }) => {
  const location = useLocation();
  const [lruOrder, setLruOrder] = React.useState<string[]>([]);
  const scrollPositionsRef = React.useRef(new Map<string, number>());
  const activeScrollKeyRef = React.useRef<string | null>(null);
  const restoreFrameRef = React.useRef<number | null>(null);
  const restoreScrollTop = getRestoreScrollTop(location.state);

  const currentRoute = React.useMemo(() => {
    return matchRouteEntry(routes, location.pathname);
  }, [location.pathname, routes]);

  const currentPath = currentRoute?.path;
  const currentScrollKey = React.useMemo(
    () => getRouteScrollKey(currentRoute, location.pathname, location.search),
    [currentRoute, location.pathname, location.search],
  );

  React.useEffect(() => {
    if (!currentPath) return;
    setLruOrder((prev) => {
      const filtered = prev.filter((p) => p !== currentPath);
      const next = [...filtered, currentPath];
      if (next.length > max) {
        return next.slice(next.length - max);
      }
      return next;
    });
  }, [currentPath, max]);

  React.useEffect(() => {
    const scrollContainer = scrollContainerRef?.current;
    if (!scrollContainer) {
      return undefined;
    }

    const handleScroll = () => {
      const activeScrollKey = activeScrollKeyRef.current;
      if (activeScrollKey) {
        scrollPositionsRef.current.set(activeScrollKey, scrollContainer.scrollTop);
      }
    };

    scrollContainer.addEventListener('scroll', handleScroll, { passive: true });
    return () => scrollContainer.removeEventListener('scroll', handleScroll);
  }, [scrollContainerRef]);

  const getScrollTop = React.useCallback(() => {
    return scrollContainerRef?.current?.scrollTop ?? 0;
  }, [scrollContainerRef]);

  const rememberScrollPosition = React.useCallback(() => {
    const scrollTop = getScrollTop();
    const activeScrollKey = activeScrollKeyRef.current;
    if (activeScrollKey) {
      scrollPositionsRef.current.set(activeScrollKey, scrollTop);
    }
    return scrollTop;
  }, [getScrollTop]);

  React.useLayoutEffect(() => {
    const scrollContainer = scrollContainerRef?.current;
    if (!scrollContainer) {
      return undefined;
    }

    activeScrollKeyRef.current = currentScrollKey;
    const scrollTop = restoreScrollTop ?? scrollPositionsRef.current.get(currentScrollKey) ?? 0;
    if (restoreScrollTop !== null) {
      scrollPositionsRef.current.set(currentScrollKey, restoreScrollTop);
    }
    scrollContainer.scrollTop = scrollTop;

    if (restoreFrameRef.current !== null) {
      window.cancelAnimationFrame(restoreFrameRef.current);
      restoreFrameRef.current = null;
    }

    restoreFrameRef.current = window.requestAnimationFrame(() => {
      scrollContainer.scrollTop = scrollTop;
      restoreFrameRef.current = null;
    });

    return () => {
      if (restoreFrameRef.current !== null) {
        window.cancelAnimationFrame(restoreFrameRef.current);
        restoreFrameRef.current = null;
      }
    };
  }, [currentScrollKey, restoreScrollTop, scrollContainerRef]);

  const cachedPaths = React.useMemo(() => {
    const nextCachedPaths = new Set(lruOrder);
    if (currentPath) {
      nextCachedPaths.add(currentPath);
    }
    return nextCachedPaths;
  }, [currentPath, lruOrder]);

  return (
    <>
      {routes.map(({ path, component: Component }) => {
        if (!cachedPaths.has(path)) return null;
        const isActive = path === currentPath;
        return (
          <CachedRouteItem
            key={path}
            path={path}
            component={Component}
            isActive={isActive}
            getScrollTop={getScrollTop}
            rememberScrollPosition={rememberScrollPosition}
          />
        );
      })}
    </>
  );
};

function getRestoreScrollTop(state: unknown): number | null {
  if (!state || typeof state !== 'object') {
    return null;
  }

  const value = (state as { restoreScrollTop?: unknown }).restoreScrollTop;
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

export default KeepAliveOutlet;
