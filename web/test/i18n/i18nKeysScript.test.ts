import assert from 'node:assert/strict';
import { execFile as execFileCallback } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

interface LocaleEntry {
  key: string;
  value: unknown;
}

interface I18nUsage {
  key: string;
  filePath: string;
  line: number;
  column: number;
}

interface DynamicUsage {
  expression: string;
  protectPrefix: string;
  protectSuffix: string;
  filePath: string;
  line: number;
  column: number;
}

interface MissingKey {
  key: string;
  locale: string;
  filePath: string;
  line: number;
  column: number;
}

interface LocaleMismatch {
  key: string;
  locale: string;
}

interface ParseError {
  filePath: string;
  line: number;
  column: number;
  message: string;
}

interface UnusedKey {
  key: string;
  protected: boolean;
  protectedBy?: string;
  locales: string[];
}

interface LocaleFile {
  locale: string;
  entries: LocaleEntry[];
}

interface I18nAnalysis {
  localeFiles: LocaleFile[];
  usedKeys: string[];
  literalUsages: I18nUsage[];
  staticUsages: I18nUsage[];
  dynamicUsages: DynamicUsage[];
  parseErrors: ParseError[];
  expandedDynamicKeyUsages: I18nUsage[];
  missingStaticKeys: MissingKey[];
  localeMismatches: LocaleMismatch[];
  unusedLocaleKeys: UnusedKey[];
  removableUnusedKeys: UnusedKey[];
  usageLocationsByKey: Map<string, I18nUsage[]>;
}

interface TextSearchResult {
  locale: string;
  key: string;
  value: unknown;
}

interface KeySearchResult extends TextSearchResult {
  usages: I18nUsage[];
}

interface UnusedAuditEntry extends UnusedKey {
  status: 'confirmed-unused' | 'needs-review' | 'protected';
  reason: string;
  exactLiteralUsages: I18nUsage[];
  dynamicSuffixMatches: Array<{
    expression: string;
    filePath: string;
    line: number;
    column: number;
  }>;
}

interface AnalyzeOptions {
  rootDirectory: string;
  localeFilePaths: string[];
  scanRoots: string[];
  dynamicIdentifierValuesByFile?: Record<string, Record<string, string[]>>;
  dynamicProtectedPrefixes?: string[];
}

const i18nKeysModuleUrl = new URL('../../../scripts/i18n-keys.mjs', import.meta.url);
const i18nKeysScriptPath = fileURLToPath(i18nKeysModuleUrl);
const projectRoot = path.resolve(path.dirname(i18nKeysScriptPath), '..');
const execFile = promisify(execFileCallback);
const i18nKeys = await import(i18nKeysModuleUrl.href) as {
  analyzeProject: (options?: AnalyzeOptions) => Promise<I18nAnalysis>;
  auditUnusedKeys: (analysis: I18nAnalysis) => UnusedAuditEntry[];
  findKeysByText: (analysis: I18nAnalysis, query: string, options?: { locale?: string }) => TextSearchResult[];
  findKeysByPrefix: (analysis: I18nAnalysis, query: string) => KeySearchResult[];
  pruneUnusedKeys: (options: {
    analysis?: I18nAnalysis;
    rootDirectory?: string;
    localeFilePaths?: string[];
    scanRoots?: string[];
    dynamicIdentifierValuesByFile?: Record<string, Record<string, string[]>>;
    dynamicProtectedPrefixes?: string[];
    prefixes?: string[];
    allConfirmed?: boolean;
    write?: boolean;
  }) => Promise<{ analysis: I18nAnalysis; removedKeys: string[] }>;
};

interface CliResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

async function createFixture(testContext: test.TestContext) {
  const rootDirectory = await mkdtemp(path.join(tmpdir(), 'i18n-keys-'));
  testContext.after(async () => {
    await rm(rootDirectory, { recursive: true, force: true });
  });

  return {
    rootDirectory,
    localeFilePaths: ['locales/zh-CN.json', 'locales/en-US.json'],
    scanRoots: ['src'],
  };
}

function fixtureCliArgs(fixture: AnalyzeOptions) {
  return [
    '--root',
    fixture.rootDirectory,
    '--locale-files',
    fixture.localeFilePaths.join(','),
    '--scan-roots',
    fixture.scanRoots.join(','),
  ];
}

async function runCli(args: string[]): Promise<CliResult> {
  try {
    const result = await execFile(process.execPath, [i18nKeysScriptPath, ...args], {
      cwd: projectRoot,
    });
    return {
      exitCode: 0,
      stdout: result.stdout,
      stderr: result.stderr,
    };
  } catch (error) {
    const execError = error as {
      code?: number;
      stdout?: string;
      stderr?: string;
    };

    return {
      exitCode: typeof execError.code === 'number' ? execError.code : 1,
      stdout: execError.stdout ?? '',
      stderr: execError.stderr ?? '',
    };
  }
}

async function writeJson(filePath: string, value: unknown) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

async function writeText(filePath: string, value: string) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, value, 'utf8');
}

async function writeLocaleFiles(rootDirectory: string, zhCN: unknown, enUS: unknown) {
  await writeJson(path.join(rootDirectory, 'locales', 'zh-CN.json'), zhCN);
  await writeJson(path.join(rootDirectory, 'locales', 'en-US.json'), enUS);
}

async function readLocaleFile(rootDirectory: string, locale: string) {
  const rawContent = await readFile(path.join(rootDirectory, 'locales', `${locale}.json`), 'utf8');
  return JSON.parse(rawContent) as Record<string, unknown>;
}

function createCompleteLocale(language: 'zh-CN' | 'en-US') {
  const isZhCN = language === 'zh-CN';

  return {
    app: {
      title: isZhCN ? '标题' : 'Title',
      subtitle: isZhCN ? '副标题' : 'Subtitle',
    },
    meta: {
      name: isZhCN ? '元信息' : 'Metadata',
    },
    nav: {
      home: isZhCN ? '首页' : 'Home',
    },
    duration: {
      minutesAgo_one: isZhCN ? '{{count}} 分钟前' : '{{count}} minute ago',
      minutesAgo_other: isZhCN ? '{{count}} 分钟前' : '{{count}} minutes ago',
    },
    alpha: {
      model: {
        name: isZhCN ? 'Alpha 名称' : 'Alpha name',
      },
    },
    beta: {
      model: {
        name: isZhCN ? 'Beta 名称' : 'Beta name',
      },
    },
    protected: {
      runtime: isZhCN ? '运行时文案' : 'Runtime copy',
    },
    search: {
      target: 'VSCode Integration',
    },
    unused: {
      remove: isZhCN ? '删除我' : 'Remove me',
    },
  };
}

test('i18n key script handles static calls, plural aliases, label keys, and known dynamic key helpers', async (testContext) => {
  const fixture = await createFixture(testContext);
  await writeLocaleFiles(
    fixture.rootDirectory,
    createCompleteLocale('zh-CN'),
    createCompleteLocale('en-US'),
  );
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'Component.tsx'),
    `
const getKey = (key: string) => \`\${i18nPrefix}.model.\${key}\`;

export function Component({ t, i18n, item, i18nPrefix }: any) {
  return [
    t('app.title'),
    i18n.t("app.subtitle"),
    getMetaText(t, 'meta.name'),
    t('duration.minutesAgo', { count: 2 }),
    t(getKey('name')),
    t(\`protected.\${item}\`),
    { labelKey: 'nav.home' },
  ];
}
`,
  );

  const analysis = await i18nKeys.analyzeProject({
    ...fixture,
    dynamicIdentifierValuesByFile: {
      'src/Component.tsx': {
        i18nPrefix: ['alpha', 'beta'],
      },
    },
  });

  assert.deepEqual(analysis.missingStaticKeys, []);
  assert.deepEqual(analysis.localeMismatches, []);

  assert.ok(analysis.usedKeys.includes('app.title'));
  assert.ok(analysis.usedKeys.includes('app.subtitle'));
  assert.ok(analysis.usedKeys.includes('meta.name'));
  assert.ok(analysis.usedKeys.includes('nav.home'));
  assert.ok(analysis.usedKeys.includes('duration.minutesAgo'));
  assert.ok(analysis.usedKeys.includes('alpha.model.name'));
  assert.ok(analysis.usedKeys.includes('beta.model.name'));

  assert.deepEqual(
    analysis.expandedDynamicKeyUsages.map((usage) => usage.key).sort(),
    ['alpha.model.name', 'beta.model.name'],
  );
  assert.ok(
    analysis.dynamicUsages.some((usage) =>
      usage.expression === 'protected.${item}'
      && usage.protectPrefix === 'protected.'
      && usage.protectSuffix === ''
    ),
  );

  const protectedRuntime = analysis.unusedLocaleKeys.find((entry) => entry.key === 'protected.runtime');
  assert.equal(protectedRuntime?.protected, true);

  const alphaModelResults = i18nKeys.findKeysByPrefix(analysis, 'alpha.model');
  assert.ok(alphaModelResults.some((result) => result.key === 'alpha.model.name' && result.usages.length > 0));
});

test('i18n key script reports missing used keys and locale key mismatches', async (testContext) => {
  const fixture = await createFixture(testContext);
  await writeLocaleFiles(
    fixture.rootDirectory,
    {
      app: {
        title: '标题',
      },
    },
    {
      app: {
        title: 'Title',
      },
      onlyEnglish: 'Only English',
    },
  );
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'Missing.tsx'),
    `
export function Missing({ t }: any) {
  return t('app.missing');
}
`,
  );

  const analysis = await i18nKeys.analyzeProject(fixture);

  assert.deepEqual(
    analysis.missingStaticKeys.map((entry) => `${entry.locale}:${entry.key}`).sort(),
    ['en-US:app.missing', 'zh-CN:app.missing'],
  );
  assert.deepEqual(analysis.localeMismatches, [
    {
      key: 'onlyEnglish',
      locale: 'zh-CN',
    },
  ]);
});

test('i18n key script can find keys by translated text with optional locale filtering', async (testContext) => {
  const fixture = await createFixture(testContext);
  await writeLocaleFiles(
    fixture.rootDirectory,
    createCompleteLocale('zh-CN'),
    createCompleteLocale('en-US'),
  );

  const analysis = await i18nKeys.analyzeProject(fixture);
  const allMatches = i18nKeys.findKeysByText(analysis, 'vscode integration');
  const zhMatches = i18nKeys.findKeysByText(analysis, 'vscode integration', { locale: 'zh-CN' });

  assert.deepEqual(
    allMatches.map((match) => `${match.locale}:${match.key}`).sort(),
    ['en-US:search.target', 'zh-CN:search.target'],
  );
  assert.deepEqual(
    zhMatches.map((match) => `${match.locale}:${match.key}`),
    ['zh-CN:search.target'],
  );
});

test('i18n key script prune supports dry-run, scoped write, and dynamic prefix protection', async (testContext) => {
  const fixture = await createFixture(testContext);
  await writeLocaleFiles(
    fixture.rootDirectory,
    createCompleteLocale('zh-CN'),
    createCompleteLocale('en-US'),
  );
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'Used.tsx'),
    `
export function Used({ t, item }: any) {
  return [t('app.title'), t(\`protected.\${item}\`)];
}
`,
  );

  const analysis = await i18nKeys.analyzeProject(fixture);
  const dryRunResult = await i18nKeys.pruneUnusedKeys({
    analysis,
    prefixes: ['unused'],
    write: false,
  });

  assert.deepEqual(dryRunResult.removedKeys, ['unused.remove']);
  assert.ok(analysis.localeFiles.some((localeFile) =>
    localeFile.entries.some((entry) => entry.key === 'unused.remove')
  ));
  assert.ok((await readLocaleFile(fixture.rootDirectory, 'zh-CN')).unused);

  const protectedRuntime = analysis.unusedLocaleKeys.find((entry) => entry.key === 'protected.runtime');
  assert.equal(protectedRuntime?.protected, true);

  const broadResult = await i18nKeys.pruneUnusedKeys({
    analysis,
    prefixes: [],
    write: true,
  });
  assert.deepEqual(broadResult.removedKeys, []);
  assert.ok((await readLocaleFile(fixture.rootDirectory, 'zh-CN')).unused);

  const writeResult = await i18nKeys.pruneUnusedKeys({
    ...fixture,
    prefixes: ['unused'],
    write: true,
  });

  assert.deepEqual(writeResult.removedKeys, ['unused.remove']);
  assert.equal((await readLocaleFile(fixture.rootDirectory, 'zh-CN')).unused, undefined);
  assert.equal((await readLocaleFile(fixture.rootDirectory, 'en-US')).unused, undefined);
  assert.ok((await readLocaleFile(fixture.rootDirectory, 'zh-CN')).protected);
});

test('i18n key script audits exact source literals before pruning all confirmed unused keys', async (testContext) => {
  const fixture = await createFixture(testContext);
  const zhCN = {
    app: {
      title: '标题',
    },
    unused: {
      literal: '字面量残留',
      remove: '删除我',
    },
  };
  const enUS = {
    app: {
      title: 'Title',
    },
    unused: {
      literal: 'Literal remains',
      remove: 'Remove me',
    },
  };

  await writeLocaleFiles(fixture.rootDirectory, zhCN, enUS);
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'LiteralAudit.tsx'),
    `
const staleButStillPresent = 'unused.literal';

export function LiteralAudit({ t }: any) {
  return t('app.title');
}
`,
  );

  const analysis = await i18nKeys.analyzeProject(fixture);
  const auditEntries = i18nKeys.auditUnusedKeys(analysis);
  const literalEntry = auditEntries.find((entry) => entry.key === 'unused.literal');
  const removeEntry = auditEntries.find((entry) => entry.key === 'unused.remove');

  assert.equal(literalEntry?.status, 'needs-review');
  assert.deepEqual(
    literalEntry?.exactLiteralUsages.map((usage) => `${usage.filePath}:${usage.line}`),
    ['src/LiteralAudit.tsx:2'],
  );
  assert.equal(removeEntry?.status, 'confirmed-unused');

  const dryRunResult = await i18nKeys.pruneUnusedKeys({
    analysis,
    allConfirmed: true,
    write: false,
  });
  assert.deepEqual(dryRunResult.removedKeys, ['unused.remove']);

  const writeResult = await i18nKeys.pruneUnusedKeys({
    ...fixture,
    allConfirmed: true,
    write: true,
  });
  assert.deepEqual(writeResult.removedKeys, ['unused.remove']);
  assert.deepEqual((await readLocaleFile(fixture.rootDirectory, 'zh-CN')).unused, {
    literal: '字面量残留',
  });
  assert.deepEqual((await readLocaleFile(fixture.rootDirectory, 'en-US')).unused, {
    literal: 'Literal remains',
  });
});

test('i18n key script surfaces malformed locale JSON as an analysis error', async (testContext) => {
  const fixture = await createFixture(testContext);
  await mkdir(path.join(fixture.rootDirectory, 'locales'), { recursive: true });
  await writeFile(path.join(fixture.rootDirectory, 'locales', 'zh-CN.json'), '{"app":', 'utf8');
  await writeJson(path.join(fixture.rootDirectory, 'locales', 'en-US.json'), {
    app: {
      title: 'Title',
    },
  });

  await assert.rejects(
    () => i18nKeys.analyzeProject(fixture),
    /JSON|Unexpected|Expected/u,
  );
});

test('i18n key script ignores comments and non-i18n strings while preserving parser edge cases', async (testContext) => {
  const fixture = await createFixture(testContext);
  const zhCN = {
    app: {
      title: '标题',
      "quote's": '带引号',
      template: '模板',
    },
    status: {
      ready: {
        label: '就绪',
        tooltip: '提示',
      },
    },
  };
  const enUS = {
    app: {
      title: 'Title',
      "quote's": 'Quoted',
      template: 'Template',
    },
    status: {
      ready: {
        label: 'Ready',
        tooltip: 'Tooltip',
      },
    },
  };

  await writeLocaleFiles(fixture.rootDirectory, zhCN, enUS);
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'ParserEdges.tsx'),
    `
const ignoredString = "t('fake.inString')";
const ignoredTemplate = \`labelKey: 'fake.inTemplate'\`;
// t('fake.inLineComment')
/* t('fake.inBlockComment') */
/* labelKey: 'fake.labelKeyInComment' */

export function ParserEdges({ t, key, state }: any) {
  return [
    t('app.title'),
    t('app.quote\\'s'),
    t(\`app.template\`),
    t(key),
    t(\`status.\${state}.label\`),
    { labelKey: 'app.title' },
  ];
}
`,
  );

  const analysis = await i18nKeys.analyzeProject(fixture);
  const usedKeys = new Set(analysis.usedKeys);

  assert.deepEqual(analysis.missingStaticKeys, []);
  assert.equal(usedKeys.has('fake.inString'), false);
  assert.equal(usedKeys.has('fake.inTemplate'), false);
  assert.equal(usedKeys.has('fake.inLineComment'), false);
  assert.equal(usedKeys.has('fake.inBlockComment'), false);
  assert.equal(usedKeys.has('fake.labelKeyInComment'), false);
  assert.ok(usedKeys.has("app.quote's"));
  assert.ok(usedKeys.has('app.template'));

  assert.ok(
    analysis.dynamicUsages.some((usage) => usage.expression === 'key' && usage.protectPrefix === ''),
  );

  const statusLabel = analysis.unusedLocaleKeys.find((entry) => entry.key === 'status.ready.label');
  const statusTooltip = analysis.unusedLocaleKeys.find((entry) => entry.key === 'status.ready.tooltip');
  assert.equal(statusLabel?.protected, true);
  assert.equal(statusTooltip?.protected, false);
});

test('i18n key script resolves AST-only usages in JSX, scoped constants, conditionals, and string concatenation', async (testContext) => {
  const fixture = await createFixture(testContext);
  const zhCN = {
    app: {
      title: '标题',
      subtitle: '副标题',
      inner: '内部',
      outer: '外部',
    },
    common: {
      disabled: '禁用',
      enabled: '启用',
    },
    nav: {
      home: '首页',
      settings: '设置',
    },
    settings: {
      profile: {
        title: '档案标题',
        tooltip: '档案提示',
      },
    },
  };
  const enUS = {
    app: {
      title: 'Title',
      subtitle: 'Subtitle',
      inner: 'Inner',
      outer: 'Outer',
    },
    common: {
      disabled: 'Disabled',
      enabled: 'Enabled',
    },
    nav: {
      home: 'Home',
      settings: 'Settings',
    },
    settings: {
      profile: {
        title: 'Profile title',
        tooltip: 'Profile tooltip',
      },
    },
  };

  await writeLocaleFiles(fixture.rootDirectory, zhCN, enUS);
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'AstEdges.tsx'),
    `
const SHADOW_KEY = 'app.outer';
const PREFIX = 'app';
const getNavKey = (name: string) => \`nav.\${name}\`;

export function AstEdges({ t, tab, state }: any) {
  const SHADOW_KEY = 'app.inner';
  return (
    <>
      <Item labelKey="nav.home" />
      <Item labelKey={'nav.settings'} />
      {[
        t(SHADOW_KEY),
        t(\`\${PREFIX}.title\`),
        t('app.' + 'subtitle'),
        t('settings.' + tab + '.title'),
        t(state ? 'common.enabled' : 'common.disabled'),
        t(getNavKey('home')),
      ]}
    </>
  );
}
`,
  );

  const analysis = await i18nKeys.analyzeProject(fixture);
  const usedKeys = new Set(analysis.usedKeys);

  assert.deepEqual(analysis.parseErrors, []);
  assert.deepEqual(analysis.missingStaticKeys, []);
  assert.equal(usedKeys.has('app.outer'), false);
  assert.ok(usedKeys.has('app.inner'));
  assert.ok(usedKeys.has('app.title'));
  assert.ok(usedKeys.has('app.subtitle'));
  assert.ok(usedKeys.has('common.disabled'));
  assert.ok(usedKeys.has('common.enabled'));
  assert.ok(usedKeys.has('nav.home'));
  assert.ok(usedKeys.has('nav.settings'));

  assert.ok(
    analysis.dynamicUsages.some((usage) =>
      usage.expression === 'settings.${tab}.title'
      && usage.protectPrefix === 'settings.'
      && usage.protectSuffix === '.title'
    ),
  );

  const profileTitle = analysis.unusedLocaleKeys.find((entry) => entry.key === 'settings.profile.title');
  const profileTooltip = analysis.unusedLocaleKeys.find((entry) => entry.key === 'settings.profile.tooltip');
  assert.equal(profileTitle?.protected, true);
  assert.equal(profileTooltip?.protected, false);
});

test('i18n key script tracks wrapper keys, object map values, configured dynamic prefixes, and Rust locale strings', async (testContext) => {
  const fixture = await createFixture(testContext);
  const zhCN = {
    settings: {
      provider: {
        headers: '请求头',
      },
      syncMessages: {
        keyAuthFailed: '公钥认证失败：{{detail}}',
      },
      webdav: {
        errors: {
          timeout: '连接超时',
        },
      },
    },
    skills: {
      errors: {
        gitNotFound: 'Git 未安装',
        gitTimeout: 'Git 操作超时',
      },
    },
    opencode: {
      provider: {
        baseUrlConfirmV1: '需要 /v1',
        baseUrlConfirmV1Beta: '需要 /v1 或 /v1beta',
      },
    },
    unused: {
      remove: '删除我',
    },
  };
  const enUS = {
    settings: {
      provider: {
        headers: 'Headers',
      },
      syncMessages: {
        keyAuthFailed: 'Public key authentication failed: {{detail}}',
      },
      webdav: {
        errors: {
          timeout: 'Connection timeout',
        },
      },
    },
    skills: {
      errors: {
        gitNotFound: 'Git is not installed',
        gitTimeout: 'Git operation timed out',
      },
    },
    opencode: {
      provider: {
        baseUrlConfirmV1: 'Requires /v1',
        baseUrlConfirmV1Beta: 'Requires /v1 or /v1beta',
      },
    },
    unused: {
      remove: 'Remove me',
    },
  };

  await writeLocaleFiles(fixture.rootDirectory, zhCN, enUS);
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'Wrappers.tsx'),
    `
const getKey = (key: string) => \`\${i18nPrefix}.provider.\${key}\`;
const GIT_ERROR_CODES = {
  GIT_NOT_FOUND: 'skills.errors.gitNotFound',
  GIT_TIMEOUT: 'skills.errors.gitTimeout',
} as const;

export function Wrappers({ t, code, i18nPrefix }: any) {
  const i18nKey = GIT_ERROR_CODES[code];
  let confirmMessageKey = '';
  if (code === 'GIT_NOT_FOUND') {
    confirmMessageKey = 'opencode.provider.baseUrlConfirmV1';
  } else {
    confirmMessageKey = 'opencode.provider.baseUrlConfirmV1Beta';
  }
  return [
    withDetail('settings.syncMessages.keyAuthFailed', 'detail', 'ssh', t),
    t(getKey('headers')),
    t(i18nKey),
    t(confirmMessageKey),
  ];
}
`,
  );
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'backend.rs'),
    `
fn webdav_error_key() -> &'static str {
    "settings.webdav.errors.timeout"
}
`,
  );

  const analysis = await i18nKeys.analyzeProject({
    ...fixture,
    dynamicIdentifierValuesByFile: {
      'src/Wrappers.tsx': {
        i18nPrefix: ['settings'],
      },
    },
  });
  const usedKeys = new Set(analysis.usedKeys);

  assert.deepEqual(analysis.parseErrors, []);
  assert.deepEqual(analysis.missingStaticKeys, []);
  assert.ok(usedKeys.has('settings.syncMessages.keyAuthFailed'));
  assert.ok(usedKeys.has('settings.provider.headers'));
  assert.ok(usedKeys.has('settings.webdav.errors.timeout'));
  assert.ok(usedKeys.has('skills.errors.gitNotFound'));
  assert.ok(usedKeys.has('skills.errors.gitTimeout'));
  assert.ok(usedKeys.has('opencode.provider.baseUrlConfirmV1'));
  assert.ok(usedKeys.has('opencode.provider.baseUrlConfirmV1Beta'));
  assert.equal(analysis.removableUnusedKeys.some((entry) => entry.key === 'unused.remove'), true);
  assert.equal(analysis.unusedLocaleKeys.some((entry) => entry.key === 'settings.syncMessages.keyAuthFailed'), false);
  assert.equal(analysis.unusedLocaleKeys.some((entry) => entry.key === 'settings.provider.headers'), false);
  assert.equal(analysis.unusedLocaleKeys.some((entry) => entry.key === 'settings.webdav.errors.timeout'), false);
  assert.equal(analysis.unusedLocaleKeys.some((entry) => entry.key === 'skills.errors.gitNotFound'), false);
  assert.equal(analysis.unusedLocaleKeys.some((entry) => entry.key === 'opencode.provider.baseUrlConfirmV1'), false);
  assert.equal(analysis.unusedLocaleKeys.some((entry) => entry.key === 'opencode.provider.baseUrlConfirmV1Beta'), false);
});

test('i18n key script reports TypeScript parse errors instead of trusting partial scans', async (testContext) => {
  const fixture = await createFixture(testContext);
  await writeLocaleFiles(
    fixture.rootDirectory,
    {
      app: {
        title: '标题',
      },
    },
    {
      app: {
        title: 'Title',
      },
    },
  );
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'Broken.tsx'),
    `
export function Broken({ t }: any) {
  return t('app.title'
}
`,
  );

  const analysis = await i18nKeys.analyzeProject(fixture);
  assert.equal(analysis.parseErrors.length > 0, true);
  assert.equal(analysis.usedKeys.includes('app.title'), false);

  const check = await runCli([...fixtureCliArgs(fixture), 'check']);
  assert.notEqual(check.exitCode, 0);
  assert.match(check.stderr, /Source files with parse errors/u);
  assert.match(check.stderr, /Broken\.tsx/u);

  const report = await runCli([...fixtureCliArgs(fixture), 'report', '--json']);
  assert.equal(report.exitCode, 0);
  const parsedReport = JSON.parse(report.stdout) as {
    parseErrors: ParseError[];
  };
  assert.equal(parsedReport.parseErrors.length > 0, true);
});

test('i18n key CLI reports check success and failure exit codes', async (testContext) => {
  const successFixture = await createFixture(testContext);
  await writeLocaleFiles(
    successFixture.rootDirectory,
    createCompleteLocale('zh-CN'),
    createCompleteLocale('en-US'),
  );
  await writeText(
    path.join(successFixture.rootDirectory, 'src', 'Success.tsx'),
    `
export function Success({ t }: any) {
  return t('app.title');
}
`,
  );

  const success = await runCli([...fixtureCliArgs(successFixture), 'check']);
  assert.equal(success.exitCode, 0);
  assert.match(success.stdout, /i18n check passed/u);

  const failureFixture = await createFixture(testContext);
  await writeLocaleFiles(
    failureFixture.rootDirectory,
    {
      app: {
        title: '标题',
      },
    },
    {
      app: {
        title: 'Title',
      },
    },
  );
  await writeText(
    path.join(failureFixture.rootDirectory, 'src', 'Failure.tsx'),
    `
export function Failure({ t }: any) {
  return t('app.missing');
}
`,
  );

  const failure = await runCli([...fixtureCliArgs(failureFixture), 'check']);
  assert.notEqual(failure.exitCode, 0);
  assert.match(failure.stderr, /app\.missing/u);
  assert.match(failure.stderr, /Missing locale keys/u);
});

test('i18n key CLI prints JSON reports and supports find commands', async (testContext) => {
  const fixture = await createFixture(testContext);
  await writeLocaleFiles(
    fixture.rootDirectory,
    createCompleteLocale('zh-CN'),
    createCompleteLocale('en-US'),
  );
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'Lookup.tsx'),
    `
export function Lookup({ t }: any) {
  return t('app.title');
}
`,
  );

  const report = await runCli([...fixtureCliArgs(fixture), 'report', '--json']);
  assert.equal(report.exitCode, 0);
  const parsedReport = JSON.parse(report.stdout) as {
    usedKeys: string[];
    missingStaticKeys: unknown[];
  };
  assert.ok(parsedReport.usedKeys.includes('app.title'));
  assert.deepEqual(parsedReport.missingStaticKeys, []);

  const findText = await runCli([...fixtureCliArgs(fixture), 'find-text', '--', 'vscode integration']);
  assert.equal(findText.exitCode, 0);
  assert.match(findText.stdout, /zh-CN search\.target/u);
  assert.match(findText.stdout, /en-US search\.target/u);

  const findKey = await runCli([...fixtureCliArgs(fixture), 'find-key', 'app.title']);
  assert.equal(findKey.exitCode, 0);
  assert.match(findKey.stdout, /zh-CN app\.title/u);
  assert.match(findKey.stdout, /used at src\/Lookup\.tsx/u);
});

test('i18n key CLI rejects unsafe prune and applies scoped write prune', async (testContext) => {
  const fixture = await createFixture(testContext);
  await writeLocaleFiles(
    fixture.rootDirectory,
    createCompleteLocale('zh-CN'),
    createCompleteLocale('en-US'),
  );
  await writeText(
    path.join(fixture.rootDirectory, 'src', 'Used.tsx'),
    `
export function Used({ t }: any) {
  return t('app.title');
}
`,
  );

  const unsafePrune = await runCli([...fixtureCliArgs(fixture), 'prune', '--write']);
  assert.notEqual(unsafePrune.exitCode, 0);
  assert.match(unsafePrune.stderr, /requires --prefix/u);

  const writePrune = await runCli([...fixtureCliArgs(fixture), 'prune', '--prefix', 'unused', '--write']);
  assert.equal(writePrune.exitCode, 0);
  assert.match(writePrune.stdout, /Removed 1 high-confidence unused i18n key/u);
  assert.equal((await readLocaleFile(fixture.rootDirectory, 'zh-CN')).unused, undefined);
  assert.equal((await readLocaleFile(fixture.rootDirectory, 'en-US')).unused, undefined);
});
