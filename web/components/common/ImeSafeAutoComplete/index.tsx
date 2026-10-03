import React from 'react';
import { AutoComplete } from 'antd';
import type { AutoCompleteProps } from 'antd';
import type { BaseOptionType, DefaultOptionType } from 'antd/es/select';
import ImeSafeInput from '@/components/common/ImeSafeInput';

/**
 * `AutoComplete` that survives IME (Chinese/Japanese/Korean) input on WebKit
 * (Safari, and therefore macOS WKWebView, which is what Tauri renders on).
 *
 * Why the plain component breaks (issue #409)
 * -------------------------------------------
 * antd's `AutoComplete` is rc-select in combobox mode and renders rc-select's
 * own `<input>` (`@rc-component/select/es/SelectInput/Input.js`). That input
 * reads `event.target.value` on every `input` event and immediately calls
 * rc-select's search path, which in combobox mode runs `triggerChange(value)`
 * (`@rc-component/select/es/Select.js`). Both happen *during* an active IME
 * composition, and the composing flag rc-select receives is only used to skip
 * token splitting — never to defer the change. antd's own `Input` (rc-input)
 * never takes that path, which is why Chinese input can commit the raw pinyin
 * here while plain inputs stay fine.
 *
 * `ImeSafeInput` fixes it by owning the value it renders and withholding the
 * text from rc-select until `compositionend`; see that component for the full
 * mechanism. The same primitive also backs plain form fields whose value is
 * written back from outside on every keystroke.
 *
 * Options must be passed through the `options` prop — `children` is reserved
 * for the guarded input.
 */
// Same generic signature as antd's AutoComplete: `OptionType` is inferred from
// `options`, which is what lets callers' inline `filterOption` see their own
// option shape. Typing it as a plain FC would pin the defaults and break them.
function ImeSafeAutoComplete<
  ValueType = any,
  OptionType extends BaseOptionType | DefaultOptionType = DefaultOptionType,
>({ size, ...props }: AutoCompleteProps<ValueType, OptionType>): React.ReactElement {
  // `size` belongs on the custom input, not on AutoComplete: with a customized
  // input antd ignores it there (and warns). Forwarding keeps dense call sites
  // (the Kimi model table) at their intended height.
  return (
    <AutoComplete {...props}>
      <ImeSafeInput size={size} />
    </AutoComplete>
  );
}

export default ImeSafeAutoComplete;
