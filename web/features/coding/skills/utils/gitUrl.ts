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
/**
 * Strip web-UI subpath suffixes so a subfolder ref resolves to its containing
 * repo: /tree/{branch}/..., /blob/{branch}/... and GitLab's /-/tree/... form.
 * Cutting at the first marker also handles branch names containing slashes.
 */
const stripWebSubpath = (rawPath: string): string => {
  return rawPath.replace(/\/(?:-\/)?(?:tree|blob)\/[\s\S]*$/, '');
};

const parseRepositoryPath = (host: string, rawPath: string): ParsedGitRepo | null => {
  const pathSegments = stripWebSubpath(rawPath)
    .split('/')
    .map((segment) => segment.trim())
    .filter(Boolean);
  if (pathSegments.length < 2) return null;

  const gitSuffixIndex = pathSegments.findIndex((segment) => segment.toLowerCase().endsWith('.git'));
  const repoIndex = gitSuffixIndex >= 0 ? gitSuffixIndex : pathSegments.length - 1;
  if (repoIndex < 1) return null;

  const repo = pathSegments[repoIndex].replace(/\.git$/i, '');
  const owner = pathSegments.slice(0, repoIndex).join('/');
  if (!host || !owner || !repo) return null;
  return { host, owner, repo };
};

export function parseGitRepo(url: string | null | undefined): ParsedGitRepo | null {
  const trimmed = (url ?? '').trim();
  if (!trimmed) return null;

  // ssh://git@host[:port]/group[/subgroup]/repo[.git]
  const sshSchemeMatch = trimmed.match(/^ssh:\/\/(?:[^@/]+@)?([^/:]+)(?::\d+)?\/(.+)$/);
  if (sshSchemeMatch) {
    return parseRepositoryPath(sshSchemeMatch[1], sshSchemeMatch[2]);
  }

  if (/^https?:\/\//i.test(trimmed)) {
    try {
      const parsedUrl = new URL(trimmed);
      return parseRepositoryPath(parsedUrl.hostname, parsedUrl.pathname);
    } catch {
      return null;
    }
  }

  // git@host:group[/subgroup]/repo[.git] (SCP style, no scheme).
  // Skip Windows drive paths like C:/... where the host would be one letter.
  const scpMatch = trimmed.match(/^(?:[^@/:]+@)?([^/:]+):(.+)$/);
  if (scpMatch && scpMatch[1].length > 1) {
    return parseRepositoryPath(scpMatch[1], scpMatch[2]);
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

/**
 * Best browser-openable web URL for a source ref: keeps https refs (including
 * /tree/ subfolder URLs) as-is, converts SSH/SCP refs to the repo's HTTPS web
 * URL, and returns null when the ref cannot be opened in a browser.
 */
export function toGitWebUrl(url: string | null | undefined): string | null {
  const trimmed = (url ?? '').trim();
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  return normalizeGitUrlToHttps(trimmed);
}
