/**
 * Git URL helpers for the skills feature.
 *
 * Skill source refs can be stored in several Git URL shapes:
 *   - HTTPS:  https://github.com/owner/repo.git
 *   - SSH:    ssh://git@github.com:22/owner/repo.git
 *   - SCP:    git@github.com:owner/repo.git  (no scheme)
 *
 * The Tauri opener only accepts URLs with a scheme, so SCP-style refs cannot be
 * opened directly. These helpers normalize any of the above into a plain HTTPS
 * web URL (https://host/owner/repo) that the opener can launch in a browser.
 */

export interface ParsedGitRepo {
  host: string;
  owner: string;
  repo: string;
}

/**
 * Parse a Git remote URL (HTTPS, SSH, or SCP style) into its host / owner / repo.
 * Returns null when the URL does not look like a Git remote pointing at a repo.
 */
export function parseGitRepo(url: string | null | undefined): ParsedGitRepo | null {
  const trimmed = (url ?? '').trim();
  if (!trimmed) return null;

  // ssh://git@host[:port]/owner/repo[.git]
  const sshSchemeMatch = trimmed.match(/^ssh:\/\/(?:[^@]+@)?([^:/]+)(?::\d+)?\/([^/]+)\/([^/]+?)(?:\.git)?(?:\/.*)?$/);
  if (sshSchemeMatch) {
    const [, host, owner, repo] = sshSchemeMatch;
    return { host, owner, repo };
  }

  // git@host:owner/repo[.git]  (SCP style, no scheme)
  // Skip Windows drive paths like C:/... where the "host" would be a single letter.
  const scpMatch = trimmed.match(/^(?:[^@]+@)?([^:/]+):([^/]+)\/([^/]+?)(?:\.git)?(?:\/.*)?$/);
  if (scpMatch) {
    const [, host, owner, repo] = scpMatch;
    if (host.length > 1) {
      return { host, owner, repo };
    }
  }

  // https://host[:port]/owner/repo[.git]
  const httpsMatch = trimmed.match(/^https?:\/\/([^:/]+)(?::\d+)?\/([^/]+)\/([^/]+?)(?:\.git)?(?:\/.*)?$/);
  if (httpsMatch) {
    const [, host, owner, repo] = httpsMatch;
    return { host, owner, repo };
  }

  return null;
}

/**
 * Normalize any Git remote URL into a plain HTTPS web URL.
 * Returns null when the input cannot be parsed as a Git remote.
 */
export function normalizeGitUrlToHttps(url: string | null | undefined): string | null {
  const parsed = parseGitRepo(url);
  if (!parsed) return null;
  return `https://${parsed.host}/${parsed.owner}/${parsed.repo}`;
}
