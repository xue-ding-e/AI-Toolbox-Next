import { useMcpStore } from '../stores/mcpStore';

export const useMcpTools = () => {
  const { tools } = useMcpStore();

  const installedTools = tools.filter((t) => t.installed);
  const supportsMcpTools = tools.filter((t) => t.supports_mcp);
  const installedMcpTools = tools.filter((t) => t.installed && t.supports_mcp);

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
