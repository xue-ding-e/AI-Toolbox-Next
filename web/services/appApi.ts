/**
 * App API Service
 *
 * Handles app-level operations like version info and updates.
 */

import { getVersion } from '@tauri-apps/api/app';
import { openUrl as openUrlExternal } from '@tauri-apps/plugin-opener';

const GITHUB_REPO = 'coulsontl/ai-toolbox';
const GITHUB_URL = `https://github.com/${GITHUB_REPO}`;
const GITHUB_API_RELEASES = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;

export interface UpdateInfo {
  hasUpdate: boolean;
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  releaseNotes: string;
}

/**
 * Get current app version
 */
export const getAppVersion = async (): Promise<string> => {
  return await getVersion();
};

/**
 * Check for updates from GitHub releases
 */
export const checkForUpdates = async (): Promise<UpdateInfo> => {
  const currentVersion = await getVersion();

  const response = await fetch(GITHUB_API_RELEASES);
  if (!response.ok) {
    throw new Error(`Failed to fetch releases: ${response.statusText}`);
  }

  const release = await response.json();
  const latestVersion = release.tag_name?.replace(/^v/, '') || '';

  return {
    hasUpdate: compareVersions(latestVersion, currentVersion) > 0,
    currentVersion,
    latestVersion,
    releaseUrl: release.html_url || GITHUB_URL,
    releaseNotes: release.body || '',
  };
};

/**
 * Open GitHub repository page
 */
export const openGitHubPage = async (): Promise<void> => {
  await openUrlExternal(GITHUB_URL);
};

/**
 * Open a URL in the default browser
 */
export const openExternalUrl = async (url: string): Promise<void> => {
  await openUrlExternal(url);
};

/**
 * Compare two version strings
 * Returns: 1 if v1 > v2, -1 if v1 < v2, 0 if equal
 */
const compareVersions = (v1: string, v2: string): number => {
  const parts1 = v1.split('.').map(Number);
  const parts2 = v2.split('.').map(Number);

  const maxLength = Math.max(parts1.length, parts2.length);

  for (let i = 0; i < maxLength; i++) {
    const num1 = parts1[i] || 0;
    const num2 = parts2[i] || 0;

    if (num1 > num2) return 1;
    if (num1 < num2) return -1;
  }

  return 0;
};
