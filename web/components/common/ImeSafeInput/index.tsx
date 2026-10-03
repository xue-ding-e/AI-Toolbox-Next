import React from 'react';
import { Input } from 'antd';

/**
 * `Input` that keeps an IME (Chinese/Japanese/Korean) composition intact on
 * WebKit (Safari, and therefore macOS WKWebView — what Tauri renders on).
 *
 * Two call sites need it, for two different reasons:
 *
 * 1. `ImeSafeAutoComplete` — antd's `AutoComplete` is rc-select in combobox mode
 *    and renders rc-select's own `<input>`, which carries no composition handling
 *    at all: it reads `event.target.value` on every `input` event and, in
 *    combobox mode, runs `triggerChange` mid-composition.
 * 2. A plain form field whose value is written back from outside on every
 *    keystroke — the Codex base-url and api-key fields, where `onValuesChange`
 *    sanitizes the text into hook state and an effect feeds it straight back with
 *    `form.setFieldsValue`. rc-input is safe on its own *only* while nothing else
 *    rewrites the value; as soon as the rewrite differs from what the IME is
 *    composing — the space a Chinese IME brings along when it commits English in
 *    Chinese mode, a CJK quote the sanitizer rewrites — WebKit ends the
 *    composition and the raw pinyin lands in the field (issue #409).
 *
 * Fix in both cases: own the value this input renders.
 *
 * - While a composition is in flight, keep the rendered value equal to what the
 *   DOM already shows. rc-input re-renders on every keystroke (its internal
 *   `setValue`), and React compares the rendered value against the live DOM node
 *   on each update — a stale rendered value would be written back and end the
 *   composition just as surely.
 * - Withhold the text from `onChange` until `compositionend`. That is the part
 *   that rewrites the *controlled* value from outside, and it is what aborts the
 *   composition on WebKit.
 *
 * `compositionend` then hands the committed text over exactly once.
 *
 * AutoComplete passes its options through the `options` prop — `children` stays
 * reserved for this input.
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

  // An outside writer (rc-select, or a form write-back) may push a new value on
  // every keystroke, including while the IME is composing. Mirroring that
  // straight into the DOM ends the composition on WebKit, so ignore external
  // updates until the composition settles.
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
    // But do not tell the outside writer yet — that is what rewrites the
    // controlled value while the composition is still in flight.
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
    // Clear the wrapper's composing flag before handing over the committed text.
    onCompositionEnd?.(event);
    setMirroredValue(event.currentTarget.value);
    // Both rc-select and rc-input only read `event.target.value`; the composition
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

export default ImeSafeInput;
