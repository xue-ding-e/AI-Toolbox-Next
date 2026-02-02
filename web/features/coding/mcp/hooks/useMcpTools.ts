import { useMemo } from 'react';
import { useMcpStore } from '../stores/mcpStore';

export const useMcpTools = () => {
  const { tools } = useMcpStore();

  const installedTools = useMemo(() => tools.filter((t) => t.installed), [tools]);
  const supportsMcpTools = useMemo(() => tools.filter((t) => t.supports_mcp), [tools]);
  const installedMcpTools = useMemo(() => tools.filter((t) => t.installed && t.supports_mcp), [tools]);

  const getToolByKey = (key: string) => tools.find((t) => t.key === key);

  return {
    tools,
    installedTools,
    supportsMcpTools,
    installedMcpTools,
    getToolByKey,
  };
};

export default useMcpTools;
