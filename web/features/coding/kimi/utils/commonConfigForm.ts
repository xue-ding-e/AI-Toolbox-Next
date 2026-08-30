import type { KimiCommonConfigInput } from '@/types/kimi';

/**
 * The general-config modal intentionally submits only the TOML payload. The
 * root directory is owned by the dedicated RootDirectoryModal; omitting
 * `rootDir`/`clearRootDir` makes the backend keep the stored value. The old
 * form always sent `clearRootDir: false` while also letting the user clear the
 * rootDir input, so a cleared field silently kept the previous directory.
 */
export function buildKimiCommonConfigSubmitValues(config: string): KimiCommonConfigInput {
  return { config };
}
