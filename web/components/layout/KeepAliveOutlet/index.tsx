import React from 'react';
import { useLocation } from 'react-router-dom';
import type { RouteEntry } from '@/app/routeConfig';

interface Props {
  routes: RouteEntry[];
  max?: number;
}

const KeepAliveContext = React.createContext<{ isActive: boolean }>({ isActive: true });

interface CachedRouteItemProps {
  path: string;
  component: RouteEntry['component'];
  isActive: boolean;
}

/**
 * 页面组件可通过此 hook 感知当前是否处于活跃状态（可见）。
 * 典型用法：页面从隐藏切回可见时触发数据刷新。
 */
export const useKeepAlive = () => React.useContext(KeepAliveContext);

const CachedRouteItem: React.FC<CachedRouteItemProps> = React.memo(
  ({ component: Component, isActive }) => {
    const contextValue = React.useMemo(() => ({ isActive }), [isActive]);

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
    && prevProps.isActive === nextProps.isActive,
);

/**
 * 基于 LRU 策略的路由组件缓存。
 * 已访问过的页面通过 display:none 隐藏而非卸载，
 * 切换回来时瞬间显示、无需重新加载数据。
 * 超出 max 上限时淘汰最久未访问的页面。
 */
const KeepAliveOutlet: React.FC<Props> = ({ routes, max = 10 }) => {
  const location = useLocation();
  const [lruOrder, setLruOrder] = React.useState<string[]>([]);

  const currentPath = React.useMemo(() => {
    // 精确匹配或子路径匹配，确保 /settings 不会误匹配 /settingspage
    let bestMatch: string | undefined;
    for (const r of routes) {
      const isMatch = location.pathname === r.path || location.pathname.startsWith(r.path + '/');
      if (isMatch && (!bestMatch || r.path.length > bestMatch.length)) {
        bestMatch = r.path;
      }
    }
    return bestMatch;
  }, [location.pathname, routes]);

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

  const cachedPaths = React.useMemo(() => new Set(lruOrder), [lruOrder]);

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
          />
        );
      })}
    </>
  );
};

export default KeepAliveOutlet;
