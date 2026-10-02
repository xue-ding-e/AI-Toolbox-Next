import React from 'react';
import { AutoComplete, Input } from 'antd';
import type { AutoCompleteProps } from 'antd';

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
 * token splitting — never to defer the change.
 *
 * antd's own `Input` (rc-input) does not have this hole: it special-cases
 * `compositionend` and clones the event so Safari's restricted `input.value`
 * getter cannot corrupt the value. `AutoComplete` never goes through it, which
 * is why Chinese input can commit the raw pinyin on macOS while plain inputs
 * stay fine.
 *
 * Fix
 * ---
 * Feed `AutoComplete` a custom child `Input` that owns the value it renders:
 *
 * - While a composition is in flight, keep the rendered value equal to what the
 *   DOM already shows. rc-input re-renders on every keystroke (its internal
 *   `setValue`), and React compares the rendered value against the live DOM
 *   node on each update — a stale rendered value would be written back and end
 *   the composition just as surely as rc-select's own rewrite.
 * - Withhold the text from rc-select until `compositionend`. That is the part
 *   that rewrites the *controlled* value from outside, and it is what aborts
 *   the composition on WebKit.
 *
 * `compositionend` then hands the committed text over exactly once.
 *
 * Options must be passed through the `options` prop — `children` is reserved
 * for the guarded input.
 */
const ImeSafeInput = React.forwardRef<
  React.ComponentRef<typeof Input>,
  Record<string, unknown>
>((rawProps, ref) => {
  const {
    value,
    onChange,
    onCompositionStart,
    onCompositionEnd,
    ...restProps
  } = rawProps as {
    value?: string;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    onCompositionStart?: React.CompositionEventHandler<HTMLInputElement>;
    onCompositionEnd?: React.CompositionEventHandler<HTMLInputElement>;
  } & Record<string, unknown>;

  const composingRef = React.useRef(false);
  const [mirroredValue, setMirroredValue] = React.useState(value ?? '');

  // rc-select rewrites its search value on every keystroke, including while the
  // IME is composing. Mirroring that straight into the DOM ends the composition
  // on WebKit, so ignore external updates until the composition settles.
  React.useEffect(() => {
    if (!composingRef.current) {
      setMirroredValue(value ?? '');
    }
  }, [value]);

  const handleChange: React.ChangeEventHandler<HTMLInputElement> = (event) => {
    // Track the DOM value even mid-composition: rc-input re-renders on every
    // keystroke, and a stale rendered value would make React write the DOM back
    // and end the composition. Keeping the mirror equal makes that write a no-op.
    setMirroredValue(event.target.value);
    // But do not tell rc-select yet — that is what rewrites the controlled value
    // from outside and aborts the composition on WebKit.
    if (composingRef.current) {
      return;
    }
    onChange?.(event);
  };

  const handleCompositionStart: React.CompositionEventHandler<HTMLInputElement> = (event) => {
    composingRef.current = true;
    onCompositionStart?.(event);
  };

  const handleCompositionEnd: React.CompositionEventHandler<HTMLInputElement> = (event) => {
    composingRef.current = false;
    // Clear rc-select's composing flag before handing over the committed text.
    onCompositionEnd?.(event);
    setMirroredValue(event.currentTarget.value);
    // rc-select's input handler only reads `event.target.value`; the composition
    // event targets the same input, so reuse it rather than fabricating one.
    onChange?.(event as unknown as React.ChangeEvent<HTMLInputElement>);
  };

  return (
    <Input
      ref={ref}
      {...(restProps as React.ComponentProps<typeof Input>)}
      value={mirroredValue}
      onChange={handleChange}
      onCompositionStart={handleCompositionStart}
      onCompositionEnd={handleCompositionEnd}
    />
  );
});

ImeSafeInput.displayName = 'ImeSafeInput';

const ImeSafeAutoComplete: React.FC<AutoCompleteProps> = (props) => (
  <AutoComplete {...props}>
    <ImeSafeInput />
  </AutoComplete>
);

export default ImeSafeAutoComplete;
