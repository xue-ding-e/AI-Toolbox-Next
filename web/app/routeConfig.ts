import type { ComponentType } from 'react';
import { NotesPage } from '@/features/daily';
import { OpenCodePage, ClaudeCodePage, CodexPage, GeminiCliPage } from '@/features/coding';
import { OpenClawPage } from '@/features/coding/openclaw';
import { SettingsPage } from '@/features/settings';
import { SkillsPage } from '@/features/coding/skills';
import { McpPage } from '@/features/coding/mcp';
import { ImagePage } from '@/features/coding/image';

export interface RouteEntry {
  path: string;
  component: ComponentType;
}

/**
 * 统一路由配置，新增页面只需在此处添加一条记录。
 * routes.tsx 和 MainLayout 的 KeepAliveOutlet 共同消费此配置。
 *
 * KeepAlive 注意事项：
 * - 页面组件在 Tab 切走时不会卸载，通过 display:none 隐藏
 * - 避免在 loadConfig 等后台刷新函数中直接调用 message.error，应使用 silent 参数
 * - 避免使用 window.location.reload()，应改为调用数据刷新函数
 * - 可通过 useKeepAlive() hook 获取 isActive 状态，感知页面是否可见
 */
export const PAGE_ROUTES: RouteEntry[] = [
  { path: '/daily/notes', component: NotesPage },
  { path: '/coding/opencode', component: OpenCodePage },
  { path: '/coding/claudecode', component: ClaudeCodePage },
  { path: '/coding/codex', component: CodexPage },
  { path: '/coding/openclaw', component: OpenClawPage },
  { path: '/coding/geminicli', component: GeminiCliPage },
  { path: '/settings', component: SettingsPage },
  { path: '/skills', component: SkillsPage },
  { path: '/mcp', component: McpPage },
  { path: '/images', component: ImagePage },
];
