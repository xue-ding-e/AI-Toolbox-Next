import type { OpenCodeAgentConfig } from '@/types/opencode';

const findFrontmatterRange = (content: string): {
  bodyStart: number;
  closingStart: number;
  frontmatterStart: number;
} | undefined => {
  const normalizedStart = content.startsWith('\uFEFF') ? 1 : 0;
  const firstLineEnd = content.indexOf('\n', normalizedStart);
  const firstLine = content.slice(normalizedStart, firstLineEnd >= 0 ? firstLineEnd : content.length)
    .replace(/\r$/, '');
  if (firstLine !== '---' || firstLineEnd < 0) return undefined;

  let lineStart = firstLineEnd + 1;
  while (lineStart <= content.length) {
    const lineEnd = content.indexOf('\n', lineStart);
    const end = lineEnd >= 0 ? lineEnd : content.length;
    const line = content.slice(lineStart, end).replace(/\r$/, '');
    if (line === '---' || line === '...') {
      return {
        frontmatterStart: firstLineEnd + 1,
        closingStart: lineStart,
        bodyStart: lineEnd >= 0 ? lineEnd + 1 : content.length,
      };
    }
    if (lineEnd < 0) break;
    lineStart = lineEnd + 1;
  }
  return undefined;
};

const detectNewline = (content: string): string => (content.includes('\r\n') ? '\r\n' : '\n');

export const replaceOpenCodeMarkdownAgentPrompt = (
  rawContent: string,
  prompt: string | undefined,
): string => {
  const range = findFrontmatterRange(rawContent);
  if (!range) return rawContent;
  const newline = detectNewline(rawContent);
  const normalizedPrompt = prompt?.replace(/\r?\n/g, newline) ?? '';
  return `${rawContent.slice(0, range.bodyStart)}${newline}${normalizedPrompt}`;
};

export const replaceOpenCodeMarkdownAgentFrontmatter = (
  rawContent: string,
  frontmatter: string,
): string => {
  const range = findFrontmatterRange(rawContent);
  if (!range) return rawContent;
  const newline = detectNewline(rawContent);
  const normalizedFrontmatter = frontmatter
    .replace(/\r?\n/g, newline)
    .replace(/(?:\r?\n)+$/, '');
  return `${rawContent.slice(0, range.frontmatterStart)}${normalizedFrontmatter}${newline}${rawContent.slice(range.closingStart)}`;
};

const quoteYamlString = (value: string): string => JSON.stringify(value);

const yamlBlockScalarPattern = /^[|>](?:[1-9][+-]?|[+-][1-9]?|[+-])?(?:\s+#.*)?$/;

const findFrontmatterFieldRange = (
  frontmatter: string,
  fieldName: 'model' | 'variant',
): { end: number; start: number } | undefined => {
  const fieldPattern = new RegExp(`^${fieldName}\\s*:(.*)(?:\\r?\\n|$)`, 'm');
  const match = fieldPattern.exec(frontmatter);
  if (!match || match.index === undefined) return undefined;

  const start = match.index;
  let end = start + match[0].length;
  if (!yamlBlockScalarPattern.test(match[1].trim())) {
    return { start, end };
  }

  while (end < frontmatter.length) {
    const nextLineEnd = frontmatter.indexOf('\n', end);
    const lineEnd = nextLineEnd >= 0 ? nextLineEnd + 1 : frontmatter.length;
    const line = frontmatter.slice(end, lineEnd).replace(/\r?\n$/, '');
    if (line.trim() !== '' && !/^\s/.test(line)) {
      break;
    }
    end = lineEnd;
  }

  return { start, end };
};

export const setOpenCodeMarkdownAgentFrontmatterField = (
  rawContent: string,
  fieldName: 'model' | 'variant',
  value: string | undefined,
): string => {
  const range = findFrontmatterRange(rawContent);
  if (!range) return rawContent;
  const newline = detectNewline(rawContent);
  const frontmatter = rawContent.slice(range.frontmatterStart, range.closingStart);
  const replacement = value ? `${fieldName}: ${quoteYamlString(value)}${newline}` : '';
  const fieldRange = findFrontmatterFieldRange(frontmatter, fieldName);
  const nextFrontmatter = fieldRange
    ? `${frontmatter.slice(0, fieldRange.start)}${replacement}${frontmatter.slice(fieldRange.end)}`
    : value
      ? `${frontmatter.replace(/(?:\r?\n)*$/, newline)}${replacement}`
      : frontmatter;
  return `${rawContent.slice(0, range.frontmatterStart)}${nextFrontmatter}${rawContent.slice(range.closingStart)}`;
};

export const mergeOpenCodeAgentConfigs = (
  baseConfig: OpenCodeAgentConfig | undefined,
  markdownConfigs: Array<OpenCodeAgentConfig | undefined>,
): OpenCodeAgentConfig => {
  const mergeObjects = (
    target: Record<string, unknown>,
    source: Record<string, unknown>,
  ): Record<string, unknown> => Object.entries(source).reduce<Record<string, unknown>>(
    (merged, [key, value]) => {
      const currentValue = merged[key];
      const canMerge = value !== null
        && typeof value === 'object'
        && !Array.isArray(value)
        && currentValue !== null
        && typeof currentValue === 'object'
        && !Array.isArray(currentValue);
      return {
        ...merged,
        [key]: canMerge
          ? mergeObjects(
            currentValue as Record<string, unknown>,
            value as Record<string, unknown>,
          )
          : value,
      };
    },
    { ...target },
  );

  return markdownConfigs.reduce<OpenCodeAgentConfig>(
    (merged, markdownConfig) => mergeObjects(
      merged,
      markdownConfig ?? {},
    ) as OpenCodeAgentConfig,
    { ...(baseConfig ?? {}) },
  );
};

export type OpenCodeAgentConfigField = 'model' | 'variant';

export type OpenCodeAgentConfigFieldSource =
  | { type: 'json' }
  | { index: number; type: 'markdown' };

export const resolveOpenCodeAgentConfigFieldSource = (
  baseConfig: OpenCodeAgentConfig | undefined,
  markdownConfigs: Array<OpenCodeAgentConfig | undefined>,
  fieldName: OpenCodeAgentConfigField,
): OpenCodeAgentConfigFieldSource | undefined => {
  for (let index = markdownConfigs.length - 1; index >= 0; index -= 1) {
    const markdownConfig = markdownConfigs[index];
    if (markdownConfig && Object.prototype.hasOwnProperty.call(markdownConfig, fieldName)) {
      return { type: 'markdown', index };
    }
  }

  if (baseConfig && Object.prototype.hasOwnProperty.call(baseConfig, fieldName)) {
    return { type: 'json' };
  }

  return undefined;
};
