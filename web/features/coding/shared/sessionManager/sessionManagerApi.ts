import { invoke } from '@tauri-apps/api/core';

import type { SessionDetail, SessionListPage, SessionTool } from './types';

interface ListToolSessionsInput {
  tool: SessionTool;
  query?: string;
  page?: number;
  pageSize?: number;
}

export const listToolSessions = async ({
  tool,
  query,
  page = 1,
  pageSize = 10,
}: ListToolSessionsInput): Promise<SessionListPage> => {
  return await invoke<SessionListPage>('list_tool_sessions', {
    tool,
    query,
    page,
    pageSize,
  });
};

export const getToolSessionDetail = async (
  tool: SessionTool,
  sourcePath: string,
): Promise<SessionDetail> => {
  return await invoke<SessionDetail>('get_tool_session_detail', {
    tool,
    sourcePath,
  });
};
