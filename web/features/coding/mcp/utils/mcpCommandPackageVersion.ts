import type { StdioConfig } from '../types';

export type McpCommandPackageManager = 'npx' | 'uv';

export interface McpCommandPackageVersion {
  manager: McpCommandPackageManager;
  packageName: string;
  versionLabel: string;
  displayText: string;
}

const NPX_COMMAND_NAMES = new Set(['npx', 'pnpx', 'tpnx']);
const UVX_COMMAND_NAMES = new Set(['uvx']);
const NPM_VALUE_FLAGS = new Set([
  '--cache',
  '--call',
  '--registry',
  '--script-shell',
  '--shell',
  '--tag',
  '--userconfig',
  '--workspace',
  '-c',
  '-w',
]);
const UV_VALUE_FLAGS = new Set([
  '--config-file',
  '--directory',
  '--env-file',
  '--from',
  '--index',
  '--index-url',
  '--keyring-provider',
  '--python',
  '--refresh-package',
  '--with',
  '--with-editable',
  '--with-requirements',
  '-b',
  '-c',
  '-p',
]);

export function getMcpCommandPackageVersion(config: unknown): McpCommandPackageVersion | null {
  const stdioConfig = asStdioConfig(config);
  if (!stdioConfig) {
    return null;
  }

  const unwrappedCommand = unwrapCmdWrapper(stdioConfig.command, stdioConfig.args);
  const commandName = getExecutableName(unwrappedCommand.command);

  if (NPX_COMMAND_NAMES.has(commandName)) {
    const packageSpec = findNpxPackageSpec(unwrappedCommand.args);
    return buildPackageVersion('npx', packageSpec, parseNpmPackageSpec);
  }

  if (UVX_COMMAND_NAMES.has(commandName)) {
    const packageSpec = findUvPackageSpec(unwrappedCommand.args);
    return buildPackageVersion('uv', packageSpec, parsePythonPackageSpec);
  }

  if (commandName === 'uv') {
    const toolRunArgs = getUvToolRunArgs(unwrappedCommand.args);
    if (!toolRunArgs) {
      return null;
    }
    const packageSpec = findUvPackageSpec(toolRunArgs);
    return buildPackageVersion('uv', packageSpec, parsePythonPackageSpec);
  }

  return null;
}

export function getMcpCommandPackageVersionKey(
  manager: McpCommandPackageManager,
  packageName: string,
): string {
  return `${manager}:${packageName.toLowerCase()}`;
}

export function formatMcpCommandPackageVersionLabel(versionLabel: string): string {
  if (/^\d/u.test(versionLabel)) {
    return `v${versionLabel}`;
  }
  return versionLabel;
}

function asStdioConfig(config: unknown): StdioConfig | null {
  if (!isRecord(config) || typeof config.command !== 'string') {
    return null;
  }

  return {
    command: config.command,
    args: Array.isArray(config.args)
      ? config.args.filter((arg): arg is string => typeof arg === 'string')
      : [],
    env: isStringRecord(config.env) ? config.env : undefined,
  };
}

function unwrapCmdWrapper(command: string, args: string[]): { command: string; args: string[] } {
  if (getExecutableName(command) !== 'cmd') {
    return { command, args };
  }

  const separatorIndex = args.findIndex((arg) => {
    const normalizedArg = arg.toLowerCase();
    return normalizedArg === '/c' || normalizedArg === '/k';
  });
  if (separatorIndex < 0 || separatorIndex >= args.length - 1) {
    return { command, args };
  }

  return {
    command: args[separatorIndex + 1],
    args: args.slice(separatorIndex + 2),
  };
}

function getExecutableName(command: string): string {
  const trimmedCommand = stripWrappingQuotes(command.trim());
  const basename = trimmedCommand.split(/[\\/]/).pop() ?? trimmedCommand;
  return basename.toLowerCase().replace(/\.(cmd|exe|ps1|bat)$/u, '');
}

function findNpxPackageSpec(args: string[]): string | null {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    const normalizedArg = arg.toLowerCase();

    if (!arg) {
      continue;
    }
    if (arg === '--') {
      return firstNonEmptyArg(args.slice(index + 1));
    }
    if (normalizedArg === '--package' || normalizedArg === '-p') {
      return args[index + 1] ?? null;
    }
    if (normalizedArg.startsWith('--package=') || normalizedArg.startsWith('-p=')) {
      return arg.slice(arg.indexOf('=') + 1);
    }
    if (arg.startsWith('-')) {
      if (flagTakesValue(normalizedArg, NPM_VALUE_FLAGS)) {
        index += 1;
      }
      continue;
    }

    return arg;
  }

  return null;
}

function getUvToolRunArgs(args: string[]): string[] | null {
  for (let index = 0; index < args.length - 1; index += 1) {
    if (args[index].toLowerCase() === 'tool' && args[index + 1].toLowerCase() === 'run') {
      return args.slice(index + 2);
    }
  }

  return null;
}

function findUvPackageSpec(args: string[]): string | null {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    const normalizedArg = arg.toLowerCase();

    if (!arg) {
      continue;
    }
    if (normalizedArg === '--from') {
      return args[index + 1] ?? null;
    }
    if (normalizedArg.startsWith('--from=')) {
      return arg.slice(arg.indexOf('=') + 1);
    }
    if (arg === '--') {
      return null;
    }
    if (arg.startsWith('-')) {
      if (flagTakesValue(normalizedArg, UV_VALUE_FLAGS)) {
        index += 1;
      }
      continue;
    }

    return arg;
  }

  return null;
}

function buildPackageVersion(
  manager: McpCommandPackageManager,
  packageSpec: string | null,
  parsePackageSpec: (packageSpec: string) => { packageName: string; versionLabel: string } | null,
): McpCommandPackageVersion | null {
  if (!packageSpec) {
    return null;
  }

  const parsedPackageSpec = parsePackageSpec(packageSpec);
  if (!parsedPackageSpec) {
    return null;
  }

  return {
    manager,
    packageName: parsedPackageSpec.packageName,
    versionLabel: parsedPackageSpec.versionLabel,
    displayText: formatMcpCommandPackageVersionLabel(parsedPackageSpec.versionLabel),
  };
}

function parseNpmPackageSpec(packageSpec: string): { packageName: string; versionLabel: string } | null {
  let normalizedSpec = stripWrappingQuotes(packageSpec.trim());
  if (normalizedSpec.startsWith('npm:')) {
    normalizedSpec = normalizedSpec.slice(4);
  }
  if (!isRegistryPackageSpec(normalizedSpec)) {
    return null;
  }

  const versionSeparatorIndex = findNpmVersionSeparator(normalizedSpec);
  if (versionSeparatorIndex < 0) {
    return {
      packageName: normalizedSpec,
      versionLabel: 'latest',
    };
  }

  const packageName = normalizedSpec.slice(0, versionSeparatorIndex);
  const versionLabel = normalizedSpec.slice(versionSeparatorIndex + 1);
  if (!packageName || !versionLabel) {
    return null;
  }

  return {
    packageName,
    versionLabel,
  };
}

function parsePythonPackageSpec(packageSpec: string): { packageName: string; versionLabel: string } | null {
  const normalizedSpec = stripWrappingQuotes(packageSpec.trim());
  if (!isRegistryPackageSpec(normalizedSpec)) {
    return null;
  }

  const exactMatch = normalizedSpec.match(/^([A-Za-z0-9][A-Za-z0-9_.-]*(?:\[[^\]]+\])?)(?:===|==)(.+)$/u);
  if (exactMatch) {
    return {
      packageName: exactMatch[1],
      versionLabel: exactMatch[2].trim(),
    };
  }

  const atMatch = normalizedSpec.match(/^([A-Za-z0-9][A-Za-z0-9_.-]*(?:\[[^\]]+\])?)@(.+)$/u);
  if (atMatch) {
    return {
      packageName: atMatch[1],
      versionLabel: atMatch[2].trim(),
    };
  }

  const rangeMatch = normalizedSpec.match(/^([A-Za-z0-9][A-Za-z0-9_.-]*(?:\[[^\]]+\])?)([<>~=!]=.+)$/u);
  if (rangeMatch) {
    return {
      packageName: rangeMatch[1],
      versionLabel: rangeMatch[2].trim(),
    };
  }

  if (/^[A-Za-z0-9][A-Za-z0-9_.-]*(?:\[[^\]]+\])?$/u.test(normalizedSpec)) {
    return {
      packageName: normalizedSpec,
      versionLabel: 'latest',
    };
  }

  return null;
}

function findNpmVersionSeparator(packageSpec: string): number {
  if (!packageSpec.startsWith('@')) {
    return packageSpec.indexOf('@');
  }

  const scopeSeparatorIndex = packageSpec.indexOf('/');
  if (scopeSeparatorIndex < 0) {
    return -1;
  }

  return packageSpec.indexOf('@', scopeSeparatorIndex + 1);
}

function isRegistryPackageSpec(packageSpec: string): boolean {
  return (
    !!packageSpec
    && !packageSpec.startsWith('.')
    && !packageSpec.startsWith('/')
    && !packageSpec.startsWith('~')
    && !packageSpec.includes('\\')
    && !packageSpec.includes('://')
    && !packageSpec.startsWith('git+')
  );
}

function flagTakesValue(flag: string, valueFlags: Set<string>): boolean {
  return valueFlags.has(flag) && !flag.includes('=');
}

function firstNonEmptyArg(args: string[]): string | null {
  return args.find((arg) => !!arg) ?? null;
}

function stripWrappingQuotes(value: string): string {
  if (value.length < 2) {
    return value;
  }

  const firstChar = value[0];
  const lastChar = value[value.length - 1];
  if ((firstChar === '"' || firstChar === "'") && firstChar === lastChar) {
    return value.slice(1, -1);
  }

  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isStringRecord(value: unknown): value is Record<string, string> {
  return isRecord(value) && Object.values(value).every((entry) => typeof entry === 'string');
}
