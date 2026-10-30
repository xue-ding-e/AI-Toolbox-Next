// A version alone cannot distinguish multiple unsigned builds of the same release.
export function formatBuildVersion(version: string, sha: unknown): string {
  const suffix = typeof sha === 'string' && /^[a-f0-9]{40}$/.test(sha)
    ? `+${sha.slice(0, 12)}` : '';
  return `${version || '-'}${suffix}`;
}
