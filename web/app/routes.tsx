import { createBrowserRouter } from 'react-router-dom';
import MainLayout from '@/components/layout/MainLayout';
import { PAGE_ROUTES } from './routeConfig';

// 页面组件的渲染和缓存由 MainLayout 中的 KeepAliveOutlet 管理，
// 此处仅声明路径用于 URL 匹配。
// 根路径 "/" 的重定向由 MainLayout 中已有的 useEffect 处理。
// 新增页面请修改 routeConfig.ts，无需同时修改多处。
export const router = createBrowserRouter([
  {
    path: '/',
    element: <MainLayout />,
    children: PAGE_ROUTES.map(({ path }) => ({
      path: path.replace(/^\//, ''),
    })),
  },
]);
