import { useState, useCallback, useEffect, useRef } from 'react';
import {
  extractCodexBaseUrl,
  setCodexBaseUrl,
  extractCodexModel,
  setCodexModel,
  normalizeQuotes,
  removeCodexBaseUrl,
  removeCodexModel,
} from '@/utils/codexConfigUtils';

interface UseCodexConfigStateProps {
  initialData?: {
    settingsConfig?: string;
  };
  /** Modal 是否打开，用于触发初始化 */
  open?: boolean;
}

interface CodexSettingsConfig {
  auth?: {
    OPENAI_API_KEY?: string;
  };
  config?: string;
}

/**
 * Codex 配置状态管理 Hook
 * 参考 cc-switch 项目实现，提供字段与 TOML 配置的双向同步
 */
export function useCodexConfigState({ initialData, open }: UseCodexConfigStateProps = {}) {
  // 基础状态
  const [codexApiKey, setCodexApiKey] = useState('');
  const [codexBaseUrl, setCodexBaseUrlState] = useState('');
  const [codexModel, setCodexModelState] = useState('');
  const [codexConfig, setCodexConfigState] = useState('');
  // auth.json 内容（包含 OPENAI_API_KEY 和其他用户可能添加的字段）
  const [codexAuth, setCodexAuthState] = useState<Record<string, unknown>>({});

  // 防止循环更新的标志位
  const isUpdatingBaseUrlRef = useRef(false);
  const isUpdatingModelRef = useRef(false);

  // 用户是否在输入框中手动设置了值（输入框优先于 TOML 编辑器）
  const userSetBaseUrlRef = useRef(false);
  const userSetModelRef = useRef(false);

  // 标记 API Key 输入框是否正在更新（用于同步到 auth.json）
  const isUpdatingApiKeyRef = useRef(false);

  // 跟踪是否已初始化（防止重复初始化）
  const isInitializedRef = useRef(false);
  const prevOpenRef = useRef(false);

  // 初始化配置（只在 Modal 首次打开时触发）
  useEffect(() => {
    // 检测 Modal 从关闭变为打开的时刻
    const justOpened = !!open && !prevOpenRef.current;
    prevOpenRef.current = !!open;

    // 如果 Modal 关闭，重置初始化标志
    if (!open) {
      isInitializedRef.current = false;
      return;
    }

    // 只在首次打开时初始化，避免状态更新导致重复初始化
    if (!justOpened && isInitializedRef.current) {
      return;
    }

    // 标记已初始化
    isInitializedRef.current = true;

    // 重置用户输入标志
    userSetBaseUrlRef.current = false;
    userSetModelRef.current = false;

    if (!initialData?.settingsConfig) {
      // 新建模式：重置所有状态
      setCodexApiKey('');
      setCodexBaseUrlState('');
      setCodexModelState('');
      setCodexConfigState('');
      setCodexAuthState({});
      return;
    }

    try {
      const config: CodexSettingsConfig = JSON.parse(initialData.settingsConfig);

      // 设置 auth.json（保留完整内容，包括用户可能添加的其他字段）
      const authObj = config.auth || {};
      setCodexAuthState(authObj);

      // 设置 API Key（从 auth 中提取）
      const apiKey = typeof authObj.OPENAI_API_KEY === 'string' ? authObj.OPENAI_API_KEY : '';
      setCodexApiKey(apiKey);

      // 设置 config.toml（保留完整配置用于预览）
      const configStr = config.config || '';

      // 提取 Base URL 和 Model 用于输入框显示
      const baseUrl = extractCodexBaseUrl(configStr) || '';
      const model = extractCodexModel(configStr) || '';

      setCodexBaseUrlState(baseUrl);
      setCodexModelState(model);

      // 保留完整配置，不移除字段（TOML 编辑器用于预览最终配置）
      setCodexConfigState(configStr);
    } catch (error) {
      console.error('Failed to parse initial config:', error);
    }
  }, [open, initialData]);

  // 与 TOML 配置保持 Base URL 同步（configToml 变化 → 提取 baseUrl）
  // 只有当 config 中存在 base_url 字段时才更新，且用户未在输入框中手动设置
  useEffect(() => {
    if (isUpdatingBaseUrlRef.current || userSetBaseUrlRef.current) {
      return;
    }
    const extracted = extractCodexBaseUrl(codexConfig);
    // 只有当 config 中存在 base_url 时才更新
    if (extracted && extracted !== codexBaseUrl) {
      setCodexBaseUrlState(extracted);
    }
  }, [codexConfig, codexBaseUrl]);

  // 与 TOML 配置保持 Model 同步（configToml 变化 → 提取 model）
  // 只有当 config 中存在 model 字段时才更新，且用户未在输入框中手动设置
  useEffect(() => {
    if (isUpdatingModelRef.current || userSetModelRef.current) {
      return;
    }
    const extracted = extractCodexModel(codexConfig);
    // 只有当 config 中存在 model 时才更新
    if (extracted && extracted !== codexModel) {
      setCodexModelState(extracted);
    }
  }, [codexConfig, codexModel]);

  // 处理 API Key 变化（同步更新 auth.json 中的 OPENAI_API_KEY）
  const handleApiKeyChange = useCallback((key: string) => {
    const trimmedKey = key.trim();
    setCodexApiKey(trimmedKey);
    // 标记正在从 API Key 输入框更新，需要同步到 auth.json 编辑器
    isUpdatingApiKeyRef.current = true;
    // 同步更新 auth.json，保留其他字段
    setCodexAuthState((prev) => ({
      ...prev,
      OPENAI_API_KEY: trimmedKey,
    }));
    // 使用 requestAnimationFrame 确保在下一帧重置
    requestAnimationFrame(() => {
      setTimeout(() => {
        isUpdatingApiKeyRef.current = false;
      }, 50);
    });
  }, []);

  // 处理 auth.json 变化（从 JSON 编辑器同步 OPENAI_API_KEY 到输入框）
  const handleAuthChange = useCallback((authObj: Record<string, unknown>) => {
    setCodexAuthState(authObj);
    // 从 auth.json 中提取 OPENAI_API_KEY 并同步到输入框
    const apiKey = typeof authObj.OPENAI_API_KEY === 'string' ? authObj.OPENAI_API_KEY : '';
    if (apiKey !== codexApiKey) {
      setCodexApiKey(apiKey);
    }
  }, [codexApiKey]);

  // 处理 Base URL 变化（baseUrl 变化 → 写入 configToml）
  const handleBaseUrlChange = useCallback((url: string) => {
    const sanitized = url.trim();
    setCodexBaseUrlState(sanitized);

    // 标记用户已在输入框中设置值，后续不再从 TOML 编辑器覆盖
    userSetBaseUrlRef.current = true;

    if (!sanitized) {
      // 如果清空，从 config 中移除
      userSetBaseUrlRef.current = false;
      setCodexConfigState((prev) => removeCodexBaseUrl(prev));
      return;
    }

    // 标记正在更新，防止循环
    isUpdatingBaseUrlRef.current = true;
    setCodexConfigState((prev) => {
      const newConfig = setCodexBaseUrl(prev, sanitized);
      return newConfig;
    });
    // 使用 requestAnimationFrame 确保在下一帧重置
    requestAnimationFrame(() => {
      setTimeout(() => {
        isUpdatingBaseUrlRef.current = false;
      }, 50);
    });
  }, []);

  // 处理 Model 变化（model 变化 → 写入 configToml）
  const handleModelChange = useCallback((model: string) => {
    const trimmed = model.trim();
    setCodexModelState(trimmed);

    // 标记用户已在输入框中设置值，后续不再从 TOML 编辑器覆盖
    userSetModelRef.current = true;

    if (!trimmed) {
      // 如果清空，从 config 中移除
      userSetModelRef.current = false;
      setCodexConfigState((prev) => removeCodexModel(prev));
      return;
    }

    // 标记正在更新，防止循环
    isUpdatingModelRef.current = true;
    setCodexConfigState((prev) => {
      const newConfig = setCodexModel(prev, trimmed);
      return newConfig;
    });
    // 使用 requestAnimationFrame 确保在下一帧重置
    requestAnimationFrame(() => {
      setTimeout(() => {
        isUpdatingModelRef.current = false;
      }, 50);
    });
  }, []);

  // 处理 Config 变化（手动编辑 configToml → 提取字段）
  const handleConfigChange = useCallback((value: string) => {
    // 归一化中文/全角/弯引号，避免 TOML 解析报错
    const normalized = normalizeQuotes(value);
    setCodexConfigState(normalized);

    // 自动提取 Base URL 和 Model（如果不是正在更新中，且用户未在输入框中手动设置）
    // 输入框的值优先于 TOML 编辑器的值
    if (!isUpdatingBaseUrlRef.current && !userSetBaseUrlRef.current) {
      const extracted = extractCodexBaseUrl(normalized);
      // 只有当 config 中存在 base_url 字段时才更新状态
      if (extracted && extracted !== codexBaseUrl) {
        setCodexBaseUrlState(extracted);
      }
    }

    if (!isUpdatingModelRef.current && !userSetModelRef.current) {
      const extractedModel = extractCodexModel(normalized);
      // 只有当 config 中存在 model 字段时才更新状态
      if (extractedModel && extractedModel !== codexModel) {
        setCodexModelState(extractedModel);
      }
    }
  }, [codexBaseUrl, codexModel]);

  // 设置 Config（支持函数更新）
  const setCodexConfig = useCallback((value: string | ((prev: string) => string)) => {
    if (typeof value === 'function') {
      setCodexConfigState((prev) => {
        const newValue = value(prev);
        return normalizeQuotes(newValue);
      });
    } else {
      setCodexConfigState(normalizeQuotes(value));
    }
  }, []);

  // 重置配置（用于预设切换或导入）
  const resetCodexConfig = useCallback((auth: Record<string, unknown>, config: string) => {
    // 设置 API Key
    const apiKey = typeof auth.OPENAI_API_KEY === 'string' ? auth.OPENAI_API_KEY : '';
    setCodexApiKey(apiKey);

    // 提取并设置字段
    const baseUrl = extractCodexBaseUrl(config) || '';
    const model = extractCodexModel(config) || '';

    setCodexBaseUrlState(baseUrl);
    setCodexModelState(model);

    // 从 config 中移除已提取的字段
    let cleanedConfig = config;
    if (baseUrl) {
      cleanedConfig = removeCodexBaseUrl(cleanedConfig);
    }
    if (model) {
      cleanedConfig = removeCodexModel(cleanedConfig);
    }

    setCodexConfigState(cleanedConfig);
  }, []);

  // 获取最终的 settingsConfig（用于保存）
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const getFinalSettingsConfig = useCallback((externalConfig?: string): string => {
    // 如果传入了外部配置，优先使用它；否则使用内部状态
    let finalConfig = externalConfig !== undefined ? externalConfig : codexConfig;

    // 写入 base_url
    if (codexBaseUrl) {
      finalConfig = setCodexBaseUrl(finalConfig, codexBaseUrl);
    }

    // 写入 model
    if (codexModel) {
      finalConfig = setCodexModel(finalConfig, codexModel);
    }

    // 使用完整的 auth.json 内容，但确保 OPENAI_API_KEY 使用输入框的值
    const finalAuth = {
      ...codexAuth,
      OPENAI_API_KEY: codexApiKey,
    };

    const settingsConfig: CodexSettingsConfig = {
      auth: finalAuth,
      config: finalConfig.trim(),
    };

    return JSON.stringify(settingsConfig);
  }, [codexApiKey, codexAuth, codexBaseUrl, codexModel, codexConfig]);

  return {
    // 状态
    codexApiKey,
    codexAuth,
    codexBaseUrl,
    codexModel,
    codexConfig,

    // 标志位（用于同步控制）
    isUpdatingApiKeyRef,

    // 变更处理器
    handleApiKeyChange,
    handleAuthChange,
    handleBaseUrlChange,
    handleModelChange,
    handleConfigChange,

    // 工具方法
    setCodexConfig,
    resetCodexConfig,
    getFinalSettingsConfig,
  };
}
