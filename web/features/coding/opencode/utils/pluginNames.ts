import type { OpenCodePluginEntry } from '@/types/opencode';

const OMO_CANONICAL_PLUGIN = 'oh-my-openagent';
const OMO_LEGACY_PLUGIN = 'oh-my-opencode';
const OMO_SLIM_PLUGIN = 'oh-my-opencode-slim';

const NPM_VERSION_SUFFIX_PATTERN = /^@(latest|next|beta|alpha|rc|canary|\d[\w.+-]*(?:\|\|[\s\w.*+<>=~^-]+)?|[\^~*><=][\w.*+-]+)$/i;

const getOpenCodeVersionSeparatorIndex = (pluginName: string): number => {
  const versionSeparatorIndex = pluginName.lastIndexOf('@');
  if (versionSeparatorIndex <= 0) {
    return -1;
  }

  const versionSuffix = pluginName.slice(versionSeparatorIndex);
  return NPM_VERSION_SUFFIX_PATTERN.test(versionSuffix)
    ? versionSeparatorIndex
    : -1;
};

export const getOpenCodePluginPackageName = (pluginName: string): string => {
  const trimmedPluginName = pluginName.trim();
  if (!trimmedPluginName) {
    return trimmedPluginName;
  }

  if (!trimmedPluginName.startsWith('@')) {
    const versionSeparatorIndex = getOpenCodeVersionSeparatorIndex(trimmedPluginName);
    return versionSeparatorIndex > 0
      ? trimmedPluginName.slice(0, versionSeparatorIndex)
      : trimmedPluginName;
  }

  const scopeSeparatorIndex = trimmedPluginName.indexOf('/');
  if (scopeSeparatorIndex === -1) {
    return trimmedPluginName;
  }

  const versionSeparatorIndex = getOpenCodeVersionSeparatorIndex(trimmedPluginName);
  return versionSeparatorIndex > scopeSeparatorIndex
    ? trimmedPluginName.slice(0, versionSeparatorIndex)
    : trimmedPluginName;
};

export const getOpenCodePluginName = (pluginEntry: OpenCodePluginEntry): string => (
  typeof pluginEntry === 'string' ? pluginEntry : pluginEntry[0]
);

export const createOpenCodePluginEntry = (pluginName: string): OpenCodePluginEntry => pluginName;

export const normalizeOpenCodePluginName = (pluginName: string): string => {
  const trimmedPluginName = pluginName.trim();
  const packageName = getOpenCodePluginPackageName(trimmedPluginName);

  if (packageName === OMO_LEGACY_PLUGIN) {
    return `${OMO_CANONICAL_PLUGIN}${trimmedPluginName.slice(packageName.length)}`;
  }

  return trimmedPluginName;
};

const hasOpenCodePluginVersionSuffix = (pluginName: string): boolean => (
  getOpenCodePluginPackageName(pluginName) !== pluginName.trim()
);

export const normalizeOpenCodePluginEntry = (pluginEntry: OpenCodePluginEntry): OpenCodePluginEntry => {
  if (typeof pluginEntry === 'string') {
    return normalizeOpenCodePluginName(pluginEntry);
  }

  return [normalizeOpenCodePluginName(pluginEntry[0]), pluginEntry[1]];
};

const getOpenCodePluginOptions = (pluginEntry: OpenCodePluginEntry): Record<string, unknown> | undefined => (
  typeof pluginEntry === 'string' ? undefined : pluginEntry[1]
);

const buildOpenCodePluginEntry = (
  pluginName: string,
  pluginOptions?: Record<string, unknown>,
): OpenCodePluginEntry => (
  pluginOptions ? [pluginName, pluginOptions] : pluginName
);

const mergeOpenCodePluginEntries = (
  existingEntry: OpenCodePluginEntry,
  candidateEntry: OpenCodePluginEntry,
): OpenCodePluginEntry => {
  const existingPluginName = getOpenCodePluginName(existingEntry);
  const candidatePluginName = getOpenCodePluginName(candidateEntry);

  let mergedPluginName = existingPluginName;
  if (
    existingPluginName !== candidatePluginName
    && canonicalOmoPluginPackageName(existingPluginName) === OMO_CANONICAL_PLUGIN
    && canonicalOmoPluginPackageName(candidatePluginName) === OMO_CANONICAL_PLUGIN
  ) {
    if (hasOpenCodePluginVersionSuffix(candidatePluginName) || !hasOpenCodePluginVersionSuffix(existingPluginName)) {
      mergedPluginName = candidatePluginName;
    }
  }

  return buildOpenCodePluginEntry(
    mergedPluginName,
    getOpenCodePluginOptions(existingEntry) ?? getOpenCodePluginOptions(candidateEntry),
  );
};

const canonicalOmoPluginPackageName = (pluginName: string): string | null => {
  const packageName = getOpenCodePluginPackageName(pluginName);
  if (packageName === OMO_CANONICAL_PLUGIN || packageName === OMO_LEGACY_PLUGIN) {
    return OMO_CANONICAL_PLUGIN;
  }
  if (packageName === OMO_SLIM_PLUGIN) {
    return OMO_SLIM_PLUGIN;
  }
  return null;
};

export const isOpenCodePluginEquivalent = (leftPluginName: string, rightPluginName: string): boolean => {
  const normalizedLeft = normalizeOpenCodePluginName(leftPluginName);
  const normalizedRight = normalizeOpenCodePluginName(rightPluginName);
  const leftOmoPackageName = canonicalOmoPluginPackageName(normalizedLeft);
  const rightOmoPackageName = canonicalOmoPluginPackageName(normalizedRight);

  if (leftOmoPackageName && rightOmoPackageName) {
    return leftOmoPackageName === rightOmoPackageName;
  }

  return normalizedLeft === normalizedRight;
};

export const sanitizeOpenCodePluginList = (pluginEntries: OpenCodePluginEntry[]): OpenCodePluginEntry[] => {
  const sanitizedPluginEntries: OpenCodePluginEntry[] = [];

  pluginEntries.forEach((pluginEntry) => {
    const normalizedPluginEntry = normalizeOpenCodePluginEntry(pluginEntry);
    const normalizedPluginName = getOpenCodePluginName(normalizedPluginEntry);
    if (!normalizedPluginName) {
      return;
    }

    const existingIndex = sanitizedPluginEntries.findIndex((existingPluginEntry) => (
      isOpenCodePluginEquivalent(getOpenCodePluginName(existingPluginEntry), normalizedPluginName)
    ));

    if (existingIndex === -1) {
      sanitizedPluginEntries.push(normalizedPluginEntry);
      return;
    }

    sanitizedPluginEntries[existingIndex] = mergeOpenCodePluginEntries(
      sanitizedPluginEntries[existingIndex],
      normalizedPluginEntry,
    );
  });

  return sanitizedPluginEntries;
};
