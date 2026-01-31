import type { TFunction } from 'i18next';
import * as api from '../services/skillsApi';
import type { ToolOption } from '../types';
import { confirmTargetOverwrite } from './errorHandlers';

export interface SyncSkillToToolsOptions {
  skillId: string;
  centralPath: string;
  skillName: string;
  selectedTools: string[];
  allTools: ToolOption[];
  t: TFunction;
  /**
   * How to handle TARGET_EXISTS errors:
   * - 'confirm': Show confirmation dialog (AddSkillModal behavior)
   * - 'skip': Auto-skip silently (ImportModal behavior)
   */
  onTargetExists: 'confirm' | 'skip';
}

/**
 * Sync a skill to multiple tools with unified error handling
 */
export async function syncSkillToTools(options: SyncSkillToToolsOptions): Promise<void> {
  const { skillId, centralPath, skillName, selectedTools, allTools, t, onTargetExists } = options;

  for (const toolId of selectedTools) {
    try {
      await api.syncSkillToTool(centralPath, skillId, toolId, skillName);
    } catch (error) {
      const errMsg = String(error);
      if (errMsg.includes('TARGET_EXISTS|')) {
        if (onTargetExists === 'confirm') {
          const match = errMsg.match(/TARGET_EXISTS\|(.+)/);
          const targetPath = match ? match[1] : '';
          const toolLabel = allTools.find((t) => t.id === toolId)?.label || toolId;
          const shouldOverwrite = await confirmTargetOverwrite(skillName, toolLabel, targetPath, t);
          if (shouldOverwrite) {
            try {
              await api.syncSkillToTool(centralPath, skillId, toolId, skillName, true);
            } catch (retryError) {
              console.error(`Failed to overwrite sync to ${toolId}:`, retryError);
            }
          }
        } else {
          // onTargetExists === 'skip'
          console.log(`Skipping ${toolId}: target exists (already synced)`);
        }
      } else {
        console.error(`Failed to sync to ${toolId}:`, error);
      }
    }
  }
}
