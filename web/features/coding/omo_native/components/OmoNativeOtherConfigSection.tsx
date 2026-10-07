import React from 'react';
import { App, Collapse, Form, Space, Typography } from 'antd';
import { SettingOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import JsoncEditor from '@/components/common/JsoncEditor';
import { saveOmoNativeOtherConfig } from '@/services/omoNativeApi';
import { isRecordEmpty } from '../utils/omoNativeProviders';

const { Text } = Typography;

interface OmoNativeOtherConfigSectionProps {
  /** 共享 base 键（`omo.jsonc` 顶层、不属于任何 harness 块的部分）。 */
  sharedBase: Record<string, unknown>;
  /** 保存后重读配置，让生效视图与预览跟上。 */
  onSaved: () => Promise<void> | void;
}

/**
 * 「其他配置」区块：`omo.jsonc` 里**不属于任何 harness 块的顶层共享键**。
 *
 * 形态与 OpenCode / Pi / OMP 的同名区块一致（`Collapse` + 失焦自动保存），
 * 文案由本模块提供。
 *
 * ⚠️ **用 `JsoncEditor` 而不是 `JsonEditor`**（2026-10-07 用户指出）：
 * `omo.jsonc` 是 **JSONC** 文件，而 `JsonEditor` 用 `JSON.parse` 校验 ——
 * 注释和尾逗号一律判为语法错误，于是编辑器满屏红波浪线，且 `valid=false`
 * 会让失焦保存**静默跳过**，用户的编辑丢失。`JsoncEditor` 用 JSON5 解析，
 * 与后端 `save_omo_native_other_config` 的读法（`json5::from_str`）一致。
 *
 * 数据形状与其余 CLI 的「其他配置」一致：**收发对象**（`sharedBase`），
 * 只有编辑器内部的文本表示是 JSONC。
 *
 * ⚠️ 这里**不能**整份重写文件：`[native]` / `[opencode]` 两个块、控制键与全部
 * 注释都要原样保留。后端 `save_omo_native_other_config` 按顶层键做原地补丁。
 */
const OmoNativeOtherConfigSection: React.FC<OmoNativeOtherConfigSectionProps> = ({
  sharedBase,
  onSaved,
}) => {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [collapsed, setCollapsed] = React.useState(true);

  // `JsoncEditor` 收发的是**文本**：它要保留注释与格式，交回来的必须是原文。
  // 解析（JSON5）在 `onChange` / `onBlur` 的第三个参数里完成。
  const displayText = React.useMemo(() => formatSharedBase(sharedBase), [sharedBase]);
  const [text, setText] = React.useState(displayText);

  // 外部刷新（应用方案、恢复备份、切根目录）后要跟上，但用户正在编辑时不要打断。
  const lastExternalRef = React.useRef(displayText);
  React.useEffect(() => {
    if (lastExternalRef.current === displayText) return;
    lastExternalRef.current = displayText;
    setText(displayText);
  }, [displayText]);

  const handleBlur = async (parsed: unknown | null, isValid: boolean) => {
    if (!isValid) return;
    const payload =
      parsed && typeof parsed === 'object' && !Array.isArray(parsed)
        ? (parsed as Record<string, unknown>)
        : {};
    if (JSON.stringify(payload) === JSON.stringify(sharedBase)) return;
    try {
      await saveOmoNativeOtherConfig(payload);
      await onSaved();
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to save OmO Native other config:', error);
      message.error(error instanceof Error ? error.message : t('common.error'));
    }
  };

  return (
    <div
      id="omo-native-other-configuration"
      data-sidebar-section="true"
      data-sidebar-title={t('omoNative.otherConfig.title')}
    >
      <Collapse
        style={{ marginBottom: 16 }}
        activeKey={collapsed ? [] : ['other']}
        onChange={(keys) => setCollapsed(!keys.includes('other'))}
        items={[
          {
            key: 'other',
            label: (
              <Space>
                <SettingOutlined style={{ marginRight: 8 }} />
                <Text strong>{t('omoNative.otherConfig.title')}</Text>
              </Space>
            ),
            children: (
              <Form.Item
                help={
                  <span>
                    <Text type="secondary">{t('omoNative.otherConfig.hint')}，</Text>
                    <span style={{ color: 'var(--ant-color-primary)' }}>
                      {t('omoNative.otherConfig.autoSaveHint')}
                    </span>
                  </span>
                }
                style={{ marginBottom: 0 }}
              >
                <JsoncEditor
                  value={text}
                  height={260}
                  onChange={(nextText) => setText(nextText)}
                  onBlur={(_nextText, isValid, parsed) => {
                    void handleBlur(parsed, isValid);
                  }}
                />
              </Form.Item>
            ),
          },
        ]}
      />
    </div>
  );
};

/** 把共享键对象渲染成编辑器文本；空对象给 `{}` 而不是空串（空串会被当成"还没加载"）。 */
function formatSharedBase(sharedBase: Record<string, unknown>): string {
  return isRecordEmpty(sharedBase) ? '{}' : JSON.stringify(sharedBase, null, 2);
}

export default OmoNativeOtherConfigSection;
