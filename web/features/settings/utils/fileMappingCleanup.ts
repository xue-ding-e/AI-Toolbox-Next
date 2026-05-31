export type CleanupFileFormat = 'json' | 'toml';

interface CleanupMappingInput {
  isDirectory?: boolean;
  isPattern?: boolean;
  targetPath?: string;
  sourcePath?: string;
}

export const cleanupFileFormatForPath = (path?: string): CleanupFileFormat | null => {
  const normalized = (path ?? '')
    .trim()
    .replace(/^["']|["']$/g, '')
    .replace(/\\/g, '/')
    .toLowerCase();

  if (normalized.endsWith('.json')) {
    return 'json';
  }
  if (normalized.endsWith('.toml')) {
    return 'toml';
  }
  return null;
};

export const cleanupFileFormatForMapping = (
  targetPath?: string,
  sourcePath?: string,
): CleanupFileFormat | null => {
  return cleanupFileFormatForPath(targetPath) ?? cleanupFileFormatForPath(sourcePath);
};

export const supportsCleanupPaths = ({
  isDirectory,
  isPattern,
  targetPath,
  sourcePath,
}: CleanupMappingInput): boolean => {
  return !isDirectory && !isPattern && cleanupFileFormatForMapping(targetPath, sourcePath) !== null;
};

const isCleanupPathValid = (path: string): boolean => {
  const chars = Array.from(path);
  if (chars[0] !== '$') {
    return false;
  }

  let index = 1;
  let hasSegment = false;
  while (index < chars.length) {
    if (chars[index] === '.') {
      index += 1;
      const start = index;
      while (index < chars.length && chars[index] !== '.' && chars[index] !== '[') {
        index += 1;
      }
      if (start === index) {
        return false;
      }
      hasSegment = true;
      continue;
    }

    if (chars[index] === '[') {
      index += 1;
      const quote = chars[index];
      if (quote !== '"' && quote !== "'") {
        return false;
      }
      index += 1;
      let hasContent = false;
      let closed = false;
      while (index < chars.length) {
        if (chars[index] === '\\') {
          index += 2;
          hasContent = true;
          continue;
        }
        if (chars[index] === quote) {
          closed = true;
          index += 1;
          break;
        }
        hasContent = true;
        index += 1;
      }
      if (!closed || !hasContent || chars[index] !== ']') {
        return false;
      }
      index += 1;
      hasSegment = true;
      continue;
    }

    return false;
  }

  return hasSegment;
};

export const normalizeCleanupPaths = (values: unknown): string[] => {
  const items = Array.isArray(values) ? values : [];
  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const item of items) {
    if (typeof item !== 'string') {
      continue;
    }
    const path = item.trim();
    if (!path || seen.has(path)) {
      continue;
    }
    seen.add(path);
    normalized.push(path);
  }

  return normalized;
};

export const invalidCleanupPaths = (values: unknown): string[] => {
  return normalizeCleanupPaths(values).filter((path) => !isCleanupPathValid(path));
};
