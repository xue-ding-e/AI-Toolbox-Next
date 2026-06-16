import { readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const currentFilePath = fileURLToPath(import.meta.url);
const projectRoot = path.resolve(path.dirname(currentFilePath), '..');

const DEFAULT_LOCALE_FILES = [
  path.join('web', 'i18n', 'locales', 'zh-CN.json'),
  path.join('web', 'i18n', 'locales', 'en-US.json'),
];

const SCAN_ROOTS = [
  'web/app',
  'web/components',
  'web/constants',
  'web/features',
  'web/hooks',
  'web/services',
  'web/stores',
  'web/types',
  'web/utils',
];

const SOURCE_EXTENSIONS = new Set(['.ts', '.tsx', '.js', '.jsx']);

const DEFAULT_DYNAMIC_PROTECTED_PREFIXES = [
  'subModules.',
  'settings.gateway.cli.',
  'settings.gateway.cliStatus.',
  'gateway.page.statistics.range.',
  'gateway.page.requests.detailTabs.',
  'gateway.takeover.state.',
  'gateway.failover.mode.',
  'image.modes.',
  'image.status.',
  'image.sizePicker.modes.',
  'skills.enabledFilter.',
  'opencode.ohMyOpenCode.agentsMeta.',
  'opencode.ohMyOpenCode.categoriesMeta.',
  'opencode.ohMyOpenCodeSlim.agents.',
  'settings.backup.builtinMappings.',
];

const DEFAULT_DYNAMIC_IDENTIFIER_VALUES_BY_FILE = {
  'web/components/common/ModelFormModal/index.tsx': {
    i18nPrefix: ['opencode'],
  },
  'web/components/common/ModelItem/index.tsx': {
    i18nPrefix: ['opencode', 'openclaw'],
  },
  'web/components/common/OfficialProviderCard/index.tsx': {
    i18nPrefix: ['opencode'],
  },
  'web/components/common/ProviderCard/index.tsx': {
    i18nPrefix: ['opencode', 'openclaw'],
  },
  'web/components/common/ProviderFormModal/index.tsx': {
    i18nPrefix: ['opencode'],
  },
  'web/features/coding/shared/prompt/GlobalPromptConfigCard.tsx': {
    translationKeyPrefix: ['claudecode.prompt', 'codex.prompt', 'geminicli.prompt', 'opencode.prompt'],
  },
  'web/features/coding/shared/prompt/GlobalPromptConfigModal.tsx': {
    translationKeyPrefix: ['claudecode.prompt', 'codex.prompt', 'geminicli.prompt', 'opencode.prompt'],
  },
  'web/features/coding/shared/prompt/GlobalPromptSettings.tsx': {
    translationKeyPrefix: ['claudecode.prompt', 'codex.prompt', 'geminicli.prompt', 'opencode.prompt'],
  },
  'web/features/coding/shared/useRootDirectoryConfig.ts': {
    translationKeyPrefix: ['claudecode', 'codex', 'geminicli'],
  },
};

const PLURAL_SUFFIXES = ['zero', 'one', 'two', 'few', 'many', 'other'];

const HELP_TEXT = `Usage:
  node scripts/i18n-keys.mjs check
  node scripts/i18n-keys.mjs report [--json]
  node scripts/i18n-keys.mjs prune [--prefix key.prefix] [--write]
  node scripts/i18n-keys.mjs find-text <text> [--locale zh-CN]
  node scripts/i18n-keys.mjs find-key <key-or-prefix>

Commands:
  check       Fails when static i18n usages are missing from any locale, or locale key sets differ.
  report      Prints used keys, unused keys, missing keys, locale mismatches, and dynamic calls.
  prune       Removes high-confidence unused keys under --prefix only when --write is provided.
  find-text   Finds locale keys by translated text.
  find-key    Finds locale values and code usage locations by key or prefix.
`;

export async function analyzeProject(options = {}) {
  const rootDirectory = options.rootDirectory ?? projectRoot;
  const localeFilePaths = options.localeFilePaths ?? DEFAULT_LOCALE_FILES;
  const scanRoots = options.scanRoots ?? SCAN_ROOTS;
  const dynamicProtectedPrefixes = [
    ...DEFAULT_DYNAMIC_PROTECTED_PREFIXES,
    ...(options.dynamicProtectedPrefixes ?? []),
  ];
  const dynamicIdentifierValuesByFile = mergeDynamicIdentifierValuesByFile(
    DEFAULT_DYNAMIC_IDENTIFIER_VALUES_BY_FILE,
    options.dynamicIdentifierValuesByFile ?? {},
  );

  const localeFiles = await readLocaleFiles(rootDirectory, localeFilePaths);
  const sourceFiles = await collectSourceFiles(rootDirectory, scanRoots);
  const sourceAnalysis = await analyzeSourceFiles(rootDirectory, sourceFiles);
  const expandedDynamicKeyUsages = expandDynamicKeyUsages(
    sourceAnalysis.dynamicUsages,
    dynamicIdentifierValuesByFile,
  );
  const localeKeysByLocale = new Map(
    localeFiles.map((localeFile) => [localeFile.locale, new Set(localeFile.entries.map((entry) => entry.key))]),
  );
  const canonicalLocaleKeysByLocale = new Map(
    localeFiles.map((localeFile) => [
      localeFile.locale,
      new Set(localeFile.entries.map((entry) => canonicalizeLocaleKey(entry.key))),
    ]),
  );
  const allActualLocaleKeys = new Set(localeFiles.flatMap((localeFile) => localeFile.entries.map((entry) => entry.key)));
  const allCanonicalLocaleKeys = new Set([...allActualLocaleKeys].map(canonicalizeLocaleKey));
  const usedKeys = new Set([
    ...sourceAnalysis.staticUsages.map((usage) => usage.key),
    ...expandedDynamicKeyUsages.map((usage) => usage.key),
  ]);
  const protectedPredicates = buildProtectedPredicates(sourceAnalysis.dynamicUsages, dynamicProtectedPrefixes);

  const missingStaticKeys = [];
  const checkedUsages = [
    ...sourceAnalysis.staticUsages,
    ...expandedDynamicKeyUsages,
  ];
  for (const usage of checkedUsages) {
    for (const localeFile of localeFiles) {
      if (!hasLocaleKey(localeKeysByLocale.get(localeFile.locale), usage.key)) {
        missingStaticKeys.push({
          key: usage.key,
          locale: localeFile.locale,
          filePath: usage.filePath,
          line: usage.line,
          column: usage.column,
        });
      }
    }
  }

  const localeMismatches = [];
  for (const localeFile of localeFiles) {
    const ownKeys = canonicalLocaleKeysByLocale.get(localeFile.locale) ?? new Set();
    for (const key of allCanonicalLocaleKeys) {
      if (!ownKeys.has(key)) {
        localeMismatches.push({ key, locale: localeFile.locale });
      }
    }
  }

  const usageLocationsByKey = groupUsagesByKey(checkedUsages);
  const unusedLocaleKeys = [];
  for (const key of [...allActualLocaleKeys].sort()) {
    if (usedKeys.has(canonicalizeLocaleKey(key))) {
      continue;
    }

    const protectedBy = findProtectionReason(key, protectedPredicates);
    unusedLocaleKeys.push({
      key,
      protected: Boolean(protectedBy),
      protectedBy,
      locales: localeFiles
        .filter((localeFile) => localeKeysByLocale.get(localeFile.locale)?.has(key))
        .map((localeFile) => localeFile.locale),
    });
  }

  return {
    rootDirectory,
    localeFiles,
    sourceFiles,
    usedKeys: [...usedKeys].sort(),
    staticUsages: sourceAnalysis.staticUsages,
    dynamicUsages: sourceAnalysis.dynamicUsages,
    expandedDynamicKeyUsages,
    unresolvedDynamicUsages: sourceAnalysis.dynamicUsages.filter((usage) => usage.protectPrefix === ''),
    missingStaticKeys,
    localeMismatches,
    unusedLocaleKeys,
    removableUnusedKeys: unusedLocaleKeys.filter((entry) => !entry.protected),
    usageLocationsByKey,
  };
}

function hasLocaleKey(localeKeys, key) {
  if (!localeKeys) {
    return false;
  }
  if (localeKeys.has(key)) {
    return true;
  }
  return PLURAL_SUFFIXES.some((suffix) => localeKeys.has(`${key}_${suffix}`));
}

function canonicalizeLocaleKey(key) {
  for (const suffix of PLURAL_SUFFIXES) {
    const suffixText = `_${suffix}`;
    if (key.endsWith(suffixText)) {
      return key.slice(0, -suffixText.length);
    }
  }
  return key;
}

export async function pruneUnusedKeys(options = {}) {
  const analysis = options.analysis ?? await analyzeProject(options);
  const prefixes = normalizePrefixList(options.prefixes ?? []);
  const keysToRemove = new Set(
    analysis.removableUnusedKeys
      .filter((entry) => matchesAnyPrefix(entry.key, prefixes))
      .map((entry) => entry.key),
  );

  if (keysToRemove.size === 0) {
    return { analysis, removedKeys: [] };
  }

  for (const localeFile of analysis.localeFiles) {
    for (const key of keysToRemove) {
      deleteNestedKey(localeFile.data, key);
    }

    if (options.write) {
      await writeFile(localeFile.absolutePath, `${JSON.stringify(localeFile.data, null, 2)}\n`, 'utf8');
    }
  }

  return {
    analysis,
    removedKeys: [...keysToRemove].sort(),
  };
}

function normalizePrefixList(prefixes) {
  return prefixes
    .flatMap((prefix) => String(prefix).split(','))
    .map((prefix) => prefix.trim())
    .filter(Boolean);
}

function mergeDynamicIdentifierValuesByFile(baseValues, overrideValues) {
  const merged = {};
  for (const [filePath, identifierValues] of Object.entries(baseValues)) {
    merged[filePath] = { ...identifierValues };
  }

  for (const [filePath, identifierValues] of Object.entries(overrideValues)) {
    merged[filePath] = {
      ...(merged[filePath] ?? {}),
      ...identifierValues,
    };
  }

  return merged;
}

function expandDynamicKeyUsages(dynamicUsages, dynamicIdentifierValuesByFile) {
  const usages = [];

  for (const usage of dynamicUsages) {
    const identifierValues = dynamicIdentifierValuesByFile[usage.filePath];
    if (!identifierValues) {
      continue;
    }

    const keys = expandDynamicExpression(usage.expression, identifierValues);
    for (const key of keys) {
      usages.push({
        key,
        filePath: usage.filePath,
        line: usage.line,
        column: usage.column,
        fromDynamicExpression: usage.expression,
      });
    }
  }

  return usages.sort(compareUsage);
}

function expandDynamicExpression(expression, identifierValues) {
  let candidates = [expression];
  let expanded = false;

  for (const [identifier, values] of Object.entries(identifierValues)) {
    const token = `\${${identifier}}`;
    if (!candidates.some((candidate) => candidate.includes(token))) {
      continue;
    }

    expanded = true;
    candidates = candidates.flatMap((candidate) =>
      values.map((value) => candidate.split(token).join(value)),
    );
  }

  if (!expanded) {
    return [];
  }

  return [...new Set(candidates.filter((candidate) => !candidate.includes('${')))];
}

function matchesAnyPrefix(key, prefixes) {
  if (prefixes.length === 0) {
    return false;
  }
  return prefixes.some((prefix) => key === prefix || key.startsWith(`${prefix}.`));
}

export function findKeysByText(analysis, query, options = {}) {
  const normalizedQuery = normalizeSearchText(query);
  const localeFilter = options.locale;

  return analysis.localeFiles.flatMap((localeFile) => {
    if (localeFilter && localeFile.locale !== localeFilter) {
      return [];
    }

    return localeFile.entries
      .filter((entry) => normalizeSearchText(String(entry.value)).includes(normalizedQuery))
      .map((entry) => ({
        locale: localeFile.locale,
        key: entry.key,
        value: entry.value,
      }));
  });
}

export function findKeysByPrefix(analysis, query) {
  return [...analysis.localeFiles]
    .flatMap((localeFile) => localeFile.entries
      .filter((entry) => entry.key === query || entry.key.startsWith(`${query}.`))
      .map((entry) => ({
        locale: localeFile.locale,
        key: entry.key,
        value: entry.value,
        usages: analysis.usageLocationsByKey.get(entry.key) ?? [],
      })));
}

async function readLocaleFiles(rootDirectory, localeFilePaths) {
  const localeFiles = [];

  for (const relativePath of localeFilePaths) {
    const absolutePath = path.join(rootDirectory, relativePath);
    const rawContent = await readFile(absolutePath, 'utf8');
    const data = JSON.parse(rawContent);
    const locale = path.basename(relativePath, '.json');
    localeFiles.push({
      locale,
      relativePath,
      absolutePath,
      data,
      entries: flattenLocaleEntries(data),
    });
  }

  return localeFiles;
}

function flattenLocaleEntries(value, prefix = '') {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return [{ key: prefix, value }];
  }

  return Object.entries(value).flatMap(([key, nestedValue]) => {
    const nextPrefix = prefix ? `${prefix}.${key}` : key;
    return flattenLocaleEntries(nestedValue, nextPrefix);
  });
}

async function collectSourceFiles(rootDirectory, scanRoots) {
  const files = [];

  for (const scanRoot of scanRoots) {
    const absoluteRoot = path.join(rootDirectory, scanRoot);
    files.push(...await collectSourceFilesFromDirectory(rootDirectory, absoluteRoot));
  }

  return files.sort((left, right) => left.relativePath.localeCompare(right.relativePath));
}

async function collectSourceFilesFromDirectory(rootDirectory, directoryPath) {
  let entries;
  try {
    entries = await readdir(directoryPath, { withFileTypes: true });
  } catch (error) {
    if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
      return [];
    }
    throw error;
  }

  const files = [];
  for (const entry of entries) {
    const entryPath = path.join(directoryPath, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist') {
        continue;
      }
      files.push(...await collectSourceFilesFromDirectory(rootDirectory, entryPath));
      continue;
    }

    if (!entry.isFile() || !SOURCE_EXTENSIONS.has(path.extname(entry.name))) {
      continue;
    }

    files.push({
      absolutePath: entryPath,
      relativePath: path.relative(rootDirectory, entryPath).split(path.sep).join('/'),
    });
  }

  return files;
}

async function analyzeSourceFiles(rootDirectory, sourceFiles) {
  const staticUsages = [];
  const dynamicUsages = [];

  for (const sourceFile of sourceFiles) {
    const content = await readFile(sourceFile.absolutePath, 'utf8');
    const constants = collectStringConstants(content);
    const calls = collectTranslationCalls(content, sourceFile.relativePath, constants);
    staticUsages.push(...calls.staticUsages);
    dynamicUsages.push(...calls.dynamicUsages);
  }

  staticUsages.sort(compareUsage);
  dynamicUsages.sort(compareUsage);

  return { staticUsages, dynamicUsages };
}

function collectStringConstants(content) {
  const constants = new Map();
  const constantRegex = /\bconst\s+([A-Z_a-z][A-Z_a-z0-9]*)\s*=\s*(['"])((?:\\.|(?!\2)[\s\S])*?)\2\s*;?/g;
  let match;
  while ((match = constantRegex.exec(content)) !== null) {
    constants.set(match[1], unescapeBasicString(match[3]));
  }
  return constants;
}

function collectTranslationCalls(content, filePath, constants) {
  const staticUsages = [];
  const dynamicUsages = [];
  const keyBuilderFunctions = collectKeyBuilderFunctions(content);

  staticUsages.push(...collectStaticKeyPropertyUsages(content, filePath));

  for (const call of findCallOpenParens(content, ['t', 'i18n.t'])) {
    const parsed = parseCallArgument(content, call.openParenIndex + 1, 0, constants, keyBuilderFunctions);
    if (!parsed) {
      continue;
    }
    addParsedUsage(parsed, content, filePath, call.openParenIndex, staticUsages, dynamicUsages);
  }

  for (const call of findCallOpenParens(content, ['getMetaText'])) {
    const parsed = parseCallArgument(content, call.openParenIndex + 1, 1, constants, keyBuilderFunctions);
    if (!parsed) {
      continue;
    }
    addParsedUsage(parsed, content, filePath, call.openParenIndex, staticUsages, dynamicUsages);
  }

  return { staticUsages, dynamicUsages };
}

function collectStaticKeyPropertyUsages(content, filePath) {
  const usages = [];
  const propertyRegex = /\b(labelKey)\s*:\s*(['"])((?:\\.|(?!\2)[\s\S])*?)\2/g;
  let match;
  while ((match = propertyRegex.exec(content)) !== null) {
    const location = offsetToLocation(content, match.index);
    usages.push({
      key: unescapeBasicString(match[3]),
      filePath,
      line: location.line,
      column: location.column,
    });
  }
  return usages;
}

function collectKeyBuilderFunctions(content) {
  const builders = new Map();
  const builderRegex = /\bconst\s+([A-Z_a-z][A-Z_a-z0-9]*)\s*=\s*\(\s*([A-Z_a-z][A-Z_a-z0-9]*)(?:\s*:\s*[^)]*)?\s*\)\s*=>\s*`((?:\\.|(?!`)[\s\S])*?)`/g;
  let match;
  while ((match = builderRegex.exec(content)) !== null) {
    builders.set(match[1], {
      parameterName: match[2],
      templateRaw: match[3],
    });
  }
  return builders;
}

function findCallOpenParens(content, names) {
  const calls = [];
  for (const name of names) {
    const escapedName = name.replace('.', '\\s*\\.\\s*');
    const regex = new RegExp(`\\b${escapedName}\\s*\\(`, 'g');
    let match;
    while ((match = regex.exec(content)) !== null) {
      calls.push({
        name,
        openParenIndex: content.indexOf('(', match.index),
      });
    }
  }
  return calls.sort((left, right) => left.openParenIndex - right.openParenIndex);
}

function parseCallArgument(content, startIndex, argumentIndex, constants, keyBuilderFunctions) {
  let index = startIndex;
  let currentArgument = 0;

  while (index < content.length) {
    index = skipWhitespace(content, index);
    if (currentArgument === argumentIndex) {
      return parseArgumentExpression(content, index, constants, keyBuilderFunctions);
    }

    index = skipArgument(content, index);
    index = skipWhitespace(content, index);
    if (content[index] !== ',') {
      return null;
    }
    index += 1;
    currentArgument += 1;
  }

  return null;
}

function parseArgumentExpression(content, index, constants, keyBuilderFunctions) {
  const char = content[index];
  if (char === '\'' || char === '"') {
    return parseQuotedString(content, index);
  }
  if (char === '`') {
    return parseTemplateLiteral(content, index, constants);
  }

  const expressionEnd = findArgumentExpressionEnd(content, index);
  const expression = content.slice(index, expressionEnd).trim();
  if (!expression) {
    return null;
  }

  const keyBuilderUsage = parseKeyBuilderCall(expression, keyBuilderFunctions, constants);
  if (keyBuilderUsage) {
    return keyBuilderUsage;
  }

  return {
    type: 'dynamic',
    expression,
    value: expression,
    protectPrefix: '',
    protectSuffix: '',
  };
}

function parseKeyBuilderCall(expression, keyBuilderFunctions, constants) {
  const callMatch = expression.match(/^([A-Z_a-z][A-Z_a-z0-9]*)\s*\(\s*(['"])((?:\\.|(?!\2)[\s\S])*?)\2\s*\)$/);
  if (!callMatch) {
    return null;
  }

  const builder = keyBuilderFunctions.get(callMatch[1]);
  if (!builder) {
    return null;
  }

  const argumentValue = unescapeBasicString(callMatch[3]);
  const raw = builder.templateRaw
    .split(`\${${builder.parameterName}}`)
    .join(argumentValue);
  if (raw.includes('${')) {
    return buildDynamicTemplateUsage(raw, constants);
  }

  return {
    type: 'static',
    value: unescapeBasicString(raw),
  };
}

function parseQuotedString(content, index) {
  const quote = content[index];
  let cursor = index + 1;
  let value = '';
  while (cursor < content.length) {
    const char = content[cursor];
    if (char === '\\') {
      value += content.slice(cursor, cursor + 2);
      cursor += 2;
      continue;
    }
    if (char === quote) {
      return {
        type: 'static',
        value: unescapeBasicString(value),
        endIndex: cursor + 1,
      };
    }
    value += char;
    cursor += 1;
  }
  return null;
}

function parseTemplateLiteral(content, index, constants) {
  let cursor = index + 1;
  let raw = '';
  let hasExpression = false;
  while (cursor < content.length) {
    const char = content[cursor];
    if (char === '\\') {
      raw += content.slice(cursor, cursor + 2);
      cursor += 2;
      continue;
    }
    if (char === '`') {
      if (!hasExpression) {
        return {
          type: 'static',
          value: unescapeBasicString(raw),
          endIndex: cursor + 1,
        };
      }
      return buildDynamicTemplateUsage(raw, constants);
    }
    if (char === '$' && content[cursor + 1] === '{') {
      hasExpression = true;
      const expressionEnd = findTemplateExpressionEnd(content, cursor + 2);
      const expression = content.slice(cursor + 2, expressionEnd).trim();
      const resolved = constants.get(expression);
      raw += resolved === undefined ? `\${${expression}}` : resolved;
      cursor = expressionEnd + 1;
      continue;
    }
    raw += char;
    cursor += 1;
  }
  return null;
}

function buildDynamicTemplateUsage(raw, constants) {
  const firstExpressionIndex = raw.indexOf('${');
  const lastExpressionStart = raw.lastIndexOf('${');
  const lastExpressionEnd = raw.indexOf('}', lastExpressionStart);
  let prefix = firstExpressionIndex === -1 ? '' : raw.slice(0, firstExpressionIndex);
  let suffix = lastExpressionEnd === -1 ? '' : raw.slice(lastExpressionEnd + 1);

  const singleIdentifierExpression = raw.match(/^\$\{([A-Z_a-z][A-Z_a-z0-9]*)\}(.*)$/);
  if (singleIdentifierExpression && constants.has(singleIdentifierExpression[1])) {
    prefix = `${constants.get(singleIdentifierExpression[1])}${singleIdentifierExpression[2]}`;
    suffix = '';
  }

  return {
    type: 'dynamic',
    expression: raw,
    value: raw,
    protectPrefix: normalizeProtectionPrefix(prefix),
    protectSuffix: suffix,
  };
}

function normalizeProtectionPrefix(prefix) {
  if (!prefix || prefix.includes('${')) {
    return '';
  }
  return prefix;
}

function skipArgument(content, index) {
  let cursor = index;
  let depth = 0;
  while (cursor < content.length) {
    const char = content[cursor];
    if (char === '\'' || char === '"') {
      const parsed = parseQuotedString(content, cursor);
      cursor = parsed?.endIndex ?? cursor + 1;
      continue;
    }
    if (char === '`') {
      cursor = skipTemplateLiteral(content, cursor);
      continue;
    }
    if (char === '(' || char === '[' || char === '{') {
      depth += 1;
    } else if (char === ')' || char === ']' || char === '}') {
      if (depth === 0) {
        return cursor;
      }
      depth -= 1;
    } else if (char === ',' && depth === 0) {
      return cursor;
    }
    cursor += 1;
  }
  return cursor;
}

function findArgumentExpressionEnd(content, index) {
  return skipArgument(content, index);
}

function skipTemplateLiteral(content, index) {
  let cursor = index + 1;
  while (cursor < content.length) {
    const char = content[cursor];
    if (char === '\\') {
      cursor += 2;
      continue;
    }
    if (char === '`') {
      return cursor + 1;
    }
    if (char === '$' && content[cursor + 1] === '{') {
      cursor = findTemplateExpressionEnd(content, cursor + 2) + 1;
      continue;
    }
    cursor += 1;
  }
  return cursor;
}

function findTemplateExpressionEnd(content, index) {
  let cursor = index;
  let depth = 1;
  while (cursor < content.length) {
    const char = content[cursor];
    if (char === '\'' || char === '"') {
      const parsed = parseQuotedString(content, cursor);
      cursor = parsed?.endIndex ?? cursor + 1;
      continue;
    }
    if (char === '`') {
      cursor = skipTemplateLiteral(content, cursor);
      continue;
    }
    if (char === '{') {
      depth += 1;
    } else if (char === '}') {
      depth -= 1;
      if (depth === 0) {
        return cursor;
      }
    }
    cursor += 1;
  }
  return cursor;
}

function skipWhitespace(content, index) {
  let cursor = index;
  while (/\s/.test(content[cursor] ?? '')) {
    cursor += 1;
  }
  return cursor;
}

function addParsedUsage(parsed, content, filePath, index, staticUsages, dynamicUsages) {
  const location = offsetToLocation(content, index);
  if (parsed.type === 'static') {
    staticUsages.push({
      key: parsed.value,
      filePath,
      line: location.line,
      column: location.column,
    });
    return;
  }

  dynamicUsages.push({
    expression: parsed.expression,
    protectPrefix: parsed.protectPrefix,
    protectSuffix: parsed.protectSuffix,
    filePath,
    line: location.line,
    column: location.column,
  });
}

function offsetToLocation(content, offset) {
  const before = content.slice(0, offset);
  const lines = before.split(/\r?\n/);
  return {
    line: lines.length,
    column: lines[lines.length - 1].length + 1,
  };
}

function compareUsage(left, right) {
  return left.filePath.localeCompare(right.filePath)
    || left.line - right.line
    || left.column - right.column;
}

function groupUsagesByKey(usages) {
  const grouped = new Map();
  for (const usage of usages) {
    const list = grouped.get(usage.key) ?? [];
    list.push(usage);
    grouped.set(usage.key, list);
  }
  return grouped;
}

function buildProtectedPredicates(dynamicUsages, dynamicProtectedPrefixes) {
  const predicates = [];

  for (const prefix of dynamicProtectedPrefixes) {
    predicates.push({
      reason: `configured prefix ${prefix}`,
      matches: (key) => key.startsWith(prefix),
    });
  }

  for (const usage of dynamicUsages) {
    if (!usage.protectPrefix) {
      continue;
    }
    predicates.push({
      reason: `${usage.filePath}:${usage.line} dynamic ${usage.protectPrefix}*${usage.protectSuffix}`,
      matches: (key) => key.startsWith(usage.protectPrefix) && key.endsWith(usage.protectSuffix),
    });
  }

  return predicates;
}

function findProtectionReason(key, predicates) {
  return predicates.find((predicate) => predicate.matches(key))?.reason;
}

function deleteNestedKey(root, key) {
  const parts = key.split('.');
  let current = root;
  const parents = [];

  for (const part of parts.slice(0, -1)) {
    if (!current || typeof current !== 'object' || Array.isArray(current)) {
      return false;
    }
    parents.push([current, part]);
    current = current[part];
  }

  if (!current || typeof current !== 'object' || Array.isArray(current)) {
    return false;
  }

  delete current[parts[parts.length - 1]];

  for (let index = parents.length - 1; index >= 0; index -= 1) {
    const [parent, part] = parents[index];
    const child = parent[part];
    if (child && typeof child === 'object' && !Array.isArray(child) && Object.keys(child).length === 0) {
      delete parent[part];
    }
  }

  return true;
}

function unescapeBasicString(value) {
  return value
    .replace(/\\n/g, '\n')
    .replace(/\\t/g, '\t')
    .replace(/\\r/g, '\r')
    .replace(/\\'/g, '\'')
    .replace(/\\"/g, '"')
    .replace(/\\\\/g, '\\');
}

function normalizeSearchText(value) {
  return value.trim().toLocaleLowerCase();
}

function formatLocation(usage) {
  return `${usage.filePath}:${usage.line}:${usage.column}`;
}

function printCheckReport(analysis) {
  if (analysis.missingStaticKeys.length > 0) {
    console.error('Missing locale keys used by code:');
    for (const missing of analysis.missingStaticKeys) {
      console.error(`- ${missing.key} missing in ${missing.locale} (${missing.filePath}:${missing.line}:${missing.column})`);
    }
  }

  if (analysis.localeMismatches.length > 0) {
    console.error('Locale key mismatches:');
    for (const mismatch of analysis.localeMismatches) {
      console.error(`- ${mismatch.key} missing in ${mismatch.locale}`);
    }
  }

  if (analysis.missingStaticKeys.length === 0 && analysis.localeMismatches.length === 0) {
    console.log('i18n check passed.');
  }
}

function printReport(analysis) {
  console.log(`Locales: ${analysis.localeFiles.map((localeFile) => localeFile.locale).join(', ')}`);
  console.log(`Source files scanned: ${analysis.sourceFiles.length}`);
  console.log(`Static used keys: ${analysis.usedKeys.length}`);
  console.log(`Dynamic calls: ${analysis.dynamicUsages.length}`);
  console.log(`Missing static keys: ${analysis.missingStaticKeys.length}`);
  console.log(`Locale mismatches: ${analysis.localeMismatches.length}`);
  console.log(`Unused locale keys: ${analysis.unusedLocaleKeys.length}`);
  console.log(`Removable unused keys: ${analysis.removableUnusedKeys.length}`);

  if (analysis.missingStaticKeys.length > 0) {
    console.log('\nMissing static keys:');
    for (const missing of analysis.missingStaticKeys) {
      console.log(`- ${missing.key} missing in ${missing.locale} (${missing.filePath}:${missing.line}:${missing.column})`);
    }
  }

  if (analysis.localeMismatches.length > 0) {
    console.log('\nLocale mismatches:');
    for (const mismatch of analysis.localeMismatches) {
      console.log(`- ${mismatch.key} missing in ${mismatch.locale}`);
    }
  }

  if (analysis.removableUnusedKeys.length > 0) {
    console.log('\nHigh-confidence unused keys:');
    for (const entry of analysis.removableUnusedKeys) {
      console.log(`- ${entry.key} (${entry.locales.join(', ')})`);
    }
  }

  if (analysis.dynamicUsages.length > 0) {
    console.log('\nDynamic i18n calls:');
    for (const usage of analysis.dynamicUsages) {
      const protection = usage.protectPrefix
        ? ` protects ${usage.protectPrefix}*${usage.protectSuffix}`
        : ' unresolved';
      console.log(`- ${formatLocation(usage)} ${usage.expression}${protection}`);
    }
  }
}

function printFindTextResults(results) {
  if (results.length === 0) {
    console.log('No matching translation text found.');
    return;
  }

  for (const result of results) {
    console.log(`${result.locale} ${result.key}: ${result.value}`);
  }
}

function printFindKeyResults(results) {
  if (results.length === 0) {
    console.log('No matching translation key found.');
    return;
  }

  for (const result of results) {
    console.log(`${result.locale} ${result.key}: ${result.value}`);
    for (const usage of result.usages) {
      console.log(`  used at ${formatLocation(usage)}`);
    }
  }
}

function parseArgs(args) {
  const flags = new Map();
  const positional = [];

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--') {
      positional.push(...args.slice(index + 1));
      break;
    }

    if (!arg.startsWith('--')) {
      positional.push(arg);
      continue;
    }

    const [flagName, inlineValue] = arg.slice(2).split('=', 2);
    if (inlineValue !== undefined) {
      flags.set(flagName, inlineValue);
      continue;
    }

    const next = args[index + 1];
    if (next && !next.startsWith('--')) {
      flags.set(flagName, next);
      index += 1;
    } else {
      flags.set(flagName, true);
    }
  }

  return { flags, positional };
}

async function main() {
  const { flags, positional } = parseArgs(process.argv.slice(2));
  const command = positional[0];

  if (!command || command === 'help' || flags.has('help')) {
    console.log(HELP_TEXT);
    return;
  }

  const analysis = await analyzeProject();

  if (command === 'check') {
    printCheckReport(analysis);
    if (analysis.missingStaticKeys.length > 0 || analysis.localeMismatches.length > 0) {
      process.exitCode = 1;
    }
    return;
  }

  if (command === 'report') {
    if (flags.has('json')) {
      console.log(JSON.stringify({
        usedKeys: analysis.usedKeys,
        dynamicUsages: analysis.dynamicUsages,
        expandedDynamicKeyUsages: analysis.expandedDynamicKeyUsages,
        missingStaticKeys: analysis.missingStaticKeys,
        localeMismatches: analysis.localeMismatches,
        unusedLocaleKeys: analysis.unusedLocaleKeys,
        removableUnusedKeys: analysis.removableUnusedKeys,
      }, null, 2));
      return;
    }
    printReport(analysis);
    return;
  }

  if (command === 'prune') {
    const write = flags.has('write');
    const prefixes = normalizePrefixList(flags.has('prefix') ? [flags.get('prefix')] : []);
    if (write && prefixes.length === 0) {
      throw new Error('prune --write requires --prefix to avoid deleting broad dynamic i18n keys accidentally.');
    }
    const result = await pruneUnusedKeys({ analysis, write, prefixes });
    if (result.removedKeys.length === 0) {
      const scope = prefixes.length > 0 ? ` under ${prefixes.join(', ')}` : '';
      console.log(`No high-confidence unused i18n keys to prune${scope}.`);
      return;
    }
    const scope = prefixes.length > 0 ? ` under ${prefixes.join(', ')}` : '';
    console.log(`${write ? 'Removed' : 'Would remove'} ${result.removedKeys.length} high-confidence unused i18n key(s)${scope}:`);
    for (const key of result.removedKeys) {
      console.log(`- ${key}`);
    }
    if (!write) {
      console.log('\nRun with --prefix <key-prefix> --write to update locale files.');
    }
    return;
  }

  if (command === 'find-text') {
    const query = positional.slice(1).join(' ');
    if (!query) {
      throw new Error('find-text requires a text query.');
    }
    printFindTextResults(findKeysByText(analysis, query, { locale: flags.get('locale') }));
    return;
  }

  if (command === 'find-key') {
    const query = positional[1];
    if (!query) {
      throw new Error('find-key requires a key or prefix.');
    }
    printFindKeyResults(findKeysByPrefix(analysis, query));
    return;
  }

  throw new Error(`Unknown command: ${command}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === currentFilePath) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
