/**
 * Codex TOML 配置工具函数
 * 参考 cc-switch 项目的实现，提供 TOML 配置的提取、写入、归一化等功能
 */

/**
 * 引号归一化：将中文引号、全角引号转换为英文引号
 * @param text - 原始文本
 * @returns 归一化后的文本
 */
export function normalizeQuotes(text: string): string {
  if (!text) return text;
  
  return text
    // 中文双引号 → 英文双引号
    .replace(/"/g, '"')
    .replace(/"/g, '"')
    // 中文单引号 → 英文单引号
    .replace(/'/g, "'")
    .replace(/'/g, "'")
    // 全角单引号 → 英文单引号
    .replace(/＇/g, "'")
    // 全角双引号 → 英文双引号
    .replace(/＂/g, '"');
}

/**
 * 从 TOML 配置文本中提取 base_url
 * @param configText - TOML 配置文本
 * @returns base_url 值，不存在则返回 undefined
 */
export function extractCodexBaseUrl(configText: string | undefined | null): string | undefined {
  try {
    const raw = typeof configText === 'string' ? configText : '';
    // 归一化中文/全角引号，避免正则提取失败
    const text = normalizeQuotes(raw);
    if (!text) return undefined;
    
    // 匹配 base_url = "xxx" 或 base_url = 'xxx'
    const match = text.match(/base_url\s*=\s*(['"])([^'"]+)\1/);
    return match?.[2];
  } catch {
    return undefined;
  }
}

/**
 * 在 TOML 配置文本中写入或更新 base_url
 * 如果已存在则替换，不存在则追加到末尾
 * @param configText - 原始 TOML 配置文本
 * @param baseUrl - base_url 值
 * @returns 更新后的 TOML 配置文本
 */
export function setCodexBaseUrl(configText: string, baseUrl: string): string {
  const trimmed = baseUrl.trim();
  if (!trimmed) {
    return configText;
  }
  
  // 归一化原文本中的引号（既能匹配，也能输出稳定格式）
  const normalizedText = normalizeQuotes(configText);
  
  // 移除 URL 中的空格
  const normalizedUrl = trimmed.replace(/\s+/g, '');
  const replacementLine = `base_url = "${normalizedUrl}"`;
  const pattern = /base_url\s*=\s*(['"])([^'"]+)\1/;
  
  // 如果已存在 base_url，则替换
  if (pattern.test(normalizedText)) {
    return normalizedText.replace(pattern, replacementLine);
  }
  
  // 如果不存在，追加到末尾
  const prefix = normalizedText && !normalizedText.endsWith('\n')
    ? `${normalizedText}\n`
    : normalizedText;
  return `${prefix}${replacementLine}\n`;
}

/**
 * 从 TOML 配置文本中移除 base_url 行
 * @param configText - 原始 TOML 配置文本
 * @returns 移除后的 TOML 配置文本
 */
export function removeCodexBaseUrl(configText: string): string {
  const normalized = normalizeQuotes(configText);
  // 移除 base_url 行（包括行尾换行符）
  return normalized.replace(/base_url\s*=\s*(['"])[^'"]+\1\n?/g, '').trim();
}

/**
 * 从 TOML 配置文本中提取 model（在 [chat] section 或顶层）
 * @param configText - TOML 配置文本
 * @returns model 值，不存在则返回 undefined
 */
export function extractCodexModel(configText: string | undefined | null): string | undefined {
  try {
    const raw = typeof configText === 'string' ? configText : '';
    const text = normalizeQuotes(raw);
    if (!text) return undefined;
    
    // 优先匹配 [chat] section 中的 model
    const chatSectionMatch = text.match(/\[chat\]\s*\n\s*model\s*=\s*(['"])([^'"]+)\1/);
    if (chatSectionMatch?.[2]) {
      return chatSectionMatch[2];
    }
    
    // 其次匹配顶层的 model（行首或前面只有空白）
    const topLevelMatch = text.match(/^model\s*=\s*(['"])([^'"]+)\1/m);
    return topLevelMatch?.[2];
  } catch {
    return undefined;
  }
}

/**
 * 在 TOML 配置文本中写入或更新 model
 * 优先更新已存在的 model（无论在 [chat] section 还是顶层）
 * 如果都不存在，则在顶层添加（不创建 [chat] section）
 * @param configText - 原始 TOML 配置文本
 * @param model - model 值
 * @returns 更新后的 TOML 配置文本
 */
export function setCodexModel(configText: string, model: string): string {
  const trimmed = model.trim();
  if (!trimmed) {
    return configText;
  }
  
  const normalizedText = normalizeQuotes(configText);
  const replacementLine = `model = "${trimmed}"`;
  
  // 检查是否存在 [chat] section
  const hasChatSection = /\[chat\]/i.test(normalizedText);
  
  if (hasChatSection) {
    // 在 [chat] section 中查找 model
    const chatModelPattern = /(\[chat\]\s*\n)(\s*model\s*=\s*(['"])[^'"]+\3)/;
    if (chatModelPattern.test(normalizedText)) {
      // [chat] section 中已有 model，替换
      return normalizedText.replace(
        chatModelPattern,
        `$1${replacementLine}`
      );
    } else {
      // [chat] section 存在但没有 model，在 [chat] 后插入
      return normalizedText.replace(
        /\[chat\]\s*\n/,
        `[chat]\n${replacementLine}\n`
      );
    }
  }
  
  // 检查顶层是否有 model（注意：不是 model_provider）
  const topLevelPattern = /^model\s*=\s*(['"])[^'"]+\1/m;
  if (topLevelPattern.test(normalizedText)) {
    // 顶层已有 model，替换
    return normalizedText.replace(topLevelPattern, replacementLine);
  }
  
  // 都不存在，在顶层添加 model（不创建 [chat] section）
  // 尝试在 model_provider 附近添加，保持配置整洁
  const modelProviderPattern = /(model_provider\s*=\s*(['"])[^'"]+\2)/;
  if (modelProviderPattern.test(normalizedText)) {
    // 在 model_provider 之前插入 model
    return normalizedText.replace(modelProviderPattern, `${replacementLine}\n$1`);
  }
  
  // 没有 model_provider，直接追加到末尾
  const prefix = normalizedText && !normalizedText.endsWith('\n')
    ? `${normalizedText}\n`
    : normalizedText;
  return `${prefix}${replacementLine}\n`;
}

/**
 * 从 TOML 配置文本中移除 model 行
 * @param configText - 原始 TOML 配置文本
 * @returns 移除后的 TOML 配置文本
 */
export function removeCodexModel(configText: string): string {
  const normalized = normalizeQuotes(configText);
  
  // 移除 [chat] section 中的 model
  let result = normalized.replace(/(\[chat\]\s*\n)\s*model\s*=\s*(['"])[^'"]+\2\n?/, '$1');
  
  // 移除顶层的 model
  result = result.replace(/^model\s*=\s*(['"])[^'"]+\1\n?/m, '');
  
  return result.trim();
}

/**
 * 从 TOML 配置文本中移除指定字段
 * @param configText - 原始 TOML 配置文本
 * @param fieldName - 要移除的字段名
 * @returns 移除后的 TOML 配置文本
 */
export function removeCodexField(configText: string, fieldName: string): string {
  const normalized = normalizeQuotes(configText);
  // 创建匹配指定字段的正则（支持单引号和双引号）
  const pattern = new RegExp(`^${fieldName}\\s*=\\s*(['"])[^'"]+\\1\\n?`, 'gm');
  return normalized.replace(pattern, '').trim();
}
