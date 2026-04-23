export type SessionTool = 'codex' | 'claudecode' | 'openclaw' | 'opencode';

export interface SessionMeta {
  providerId: SessionTool;
  sessionId: string;
  title?: string;
  summary?: string;
  projectDir?: string | null;
  createdAt?: number;
  lastActiveAt?: number;
  sourcePath: string;
  resumeCommand?: string | null;
}

export interface SessionMessage {
  role: string;
  content: string;
  ts?: number;
}

export interface SessionListPage {
  items: SessionMeta[];
  page: number;
  pageSize: number;
  total: number;
  hasMore: boolean;
  availablePaths?: string[];
}

export interface SessionDetail {
  meta: SessionMeta;
  messages: SessionMessage[];
}

export interface DeleteSessionFailure {
  sourcePath: string;
  error: string;
}

export interface DeleteToolSessionsResult {
  deletedCount: number;
  failedItems: DeleteSessionFailure[];
}

export interface SessionTocItem {
  index: number;
  preview: string;
  ts?: number;
}

export interface SessionPathOption {
  label: string;
  value: string;
}
