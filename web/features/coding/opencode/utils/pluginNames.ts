const OMO_CANONICAL_PLUGIN = 'oh-my-openagent';
const OMO_LEGACY_PLUGIN = 'oh-my-opencode';

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

export const normalizeOpenCodePluginName = (pluginName: string): string => {
  const trimmedPluginName = pluginName.trim();
  const packageName = getOpenCodePluginPackageName(trimmedPluginName);

  if (packageName === OMO_LEGACY_PLUGIN) {
    return `${OMO_CANONICAL_PLUGIN}${trimmedPluginName.slice(packageName.length)}`;
  }

  return trimmedPluginName;
};

export const isOpenCodePluginEquivalent = (leftPluginName: string, rightPluginName: string): boolean => {
  const normalizedLeft = normalizeOpenCodePluginName(leftPluginName);
  const normalizedRight = normalizeOpenCodePluginName(rightPluginName);
  return getOpenCodePluginPackageName(normalizedLeft) === getOpenCodePluginPackageName(normalizedRight);
};

export const sanitizeOpenCodePluginList = (pluginNames: string[]): string[] => {
  const sanitizedPluginNames: string[] = [];

  pluginNames.forEach((pluginName) => {
    const normalizedPluginName = normalizeOpenCodePluginName(pluginName);
    if (!normalizedPluginName) {
      return;
    }

    const existingIndex = sanitizedPluginNames.findIndex((existingPluginName) => (
      isOpenCodePluginEquivalent(existingPluginName, normalizedPluginName)
    ));

    if (existingIndex === -1) {
      sanitizedPluginNames.push(normalizedPluginName);
      return;
    }

    const existingPackageName = getOpenCodePluginPackageName(sanitizedPluginNames[existingIndex]);
    const normalizedPackageName = getOpenCodePluginPackageName(normalizedPluginName);
    if (
      existingPackageName === OMO_CANONICAL_PLUGIN
      && normalizedPackageName === OMO_CANONICAL_PLUGIN
      && sanitizedPluginNames[existingIndex] !== normalizedPluginName
    ) {
      sanitizedPluginNames[existingIndex] = normalizedPluginName;
    }
  });

  return sanitizedPluginNames;
};
