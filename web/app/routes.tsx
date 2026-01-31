import { createBrowserRouter, Navigate } from 'react-router-dom';
import MainLayout from '@/components/layout/MainLayout';
import { NotesPage } from '@/features/daily';
import { OpenCodePage, ClaudeCodePage, CodexPage } from '@/features/coding';
import { SettingsPage } from '@/features/settings';
import { SkillsPage } from '@/features/coding/skills';
import { McpPage } from '@/features/coding/mcp';
import { DEFAULT_PATH } from '@/constants';

export const router = createBrowserRouter([
  {
    path: '/',
    element: <MainLayout />,
    children: [
      {
        index: true,
        element: <Navigate to={DEFAULT_PATH} replace />,
      },
      {
        path: 'daily/notes',
        element: <NotesPage />,
      },
      {
        path: 'coding/opencode',
        element: <OpenCodePage />,
      },
      {
        path: 'coding/claudecode',
        element: <ClaudeCodePage />,
      },
      {
        path: 'coding/codex',
        element: <CodexPage />,
      },
      {
        path: 'settings',
        element: <SettingsPage />,
      },
      {
        path: 'skills',
        element: <SkillsPage />,
      },
      {
        path: 'mcp',
        element: <McpPage />,
      },
    ],
  },
]);
