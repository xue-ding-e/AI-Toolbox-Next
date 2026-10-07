/**
 * Pi 系 CLI（Pi / OmO Native）扩展管理的共享契约。
 *
 * 两个 CLI 共用 senpi 引擎的包体系：`list` 输出逐字相同、`settings.json` 形状相同、
 * 过滤语义相同。后端逻辑也共用（`coding/cli_extensions.rs`），前端这里同理——
 * 差异全部收在 {@link CliExtensionsService} 里，由各页面注入自己的 API 实现。
 *
 * 类型与各页面 `types/<cli>.ts` 里的 `*ExtensionSummary` 同形，字段逐一对应。
 */

export type CliExtensionScope = 'user' | 'project' | 'unknown';

export type CliExtensionKind = 'package' | 'local_file' | 'local_directory';

export interface CliExtensionSummary {
  id: string;
  source: string;
  scope: CliExtensionScope;
  kind: CliExtensionKind;
  path?: string;
  builtIn: boolean;
  currentVersion?: string;
  latestVersion?: string;
  updateAvailable: boolean;
  enabled: boolean;
  switchSupported: boolean;
}

export interface CliExtensionListResult {
  extensionsPath: string;
  packagesPath: string;
  extensions: CliExtensionSummary[];
  raw: string;
  cliPath?: string;
  cliVersion?: string;
}

export interface CliExtensionCommandResult {
  command: string;
  output: string;
}

/**
 * 一个 CLI 的扩展 API。各页面把自己的 `services/<cli>Api.ts` 适配成这个形状传进来。
 *
 * `install` / `uninstall` / `update` / `setEnabled` 的入参形状两个 CLI 完全一致
 * （后端也共用），所以直接复用共享类型。
 */
export interface CliExtensionsService {
  list: () => Promise<CliExtensionListResult>;
  install: (source: string) => Promise<CliExtensionCommandResult>;
  uninstall: (input: {
    source: string;
    scope?: CliExtensionScope;
    kind?: CliExtensionKind;
    path?: string;
  }) => Promise<CliExtensionCommandResult>;
  update: (source?: string) => Promise<CliExtensionCommandResult>;
  setEnabled: (input: {
    source: string;
    kind: CliExtensionKind;
    enabled: boolean;
  }) => Promise<void>;
}
