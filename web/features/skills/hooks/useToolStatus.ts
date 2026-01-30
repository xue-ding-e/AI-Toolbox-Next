import React from 'react';
import { useSkillsStore } from '../stores/skillsStore';

export function useToolStatus() {
  const { toolStatus, loadToolStatus } = useSkillsStore();

  React.useEffect(() => {
    loadToolStatus();
  }, [loadToolStatus]);

  return {
    toolStatus,
    installedTools: toolStatus?.installed || [],
    newlyInstalledTools: toolStatus?.newly_installed || [],
    allTools: toolStatus?.tools || [],
  };
}
