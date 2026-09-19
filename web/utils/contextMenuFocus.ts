/**
 * Focus the right-clicked editable element before the context menu opens
 * (issue #369).
 *
 * WebView2's native context menu pastes into the focused element, and
 * right-clicking does not move focus by itself. Inside an antd Modal the
 * focus sits on the modal container, so "Paste" silently did nothing even
 * though Copy worked (it reads the selection). Focusing the editable target
 * restores the native menu's paste.
 */
export function installEditableContextMenuFocus(): void {
  document.addEventListener(
    'contextmenu',
    (event) => {
      const { target } = event;
      if (!(target instanceof Element)) return;

      const editable = target.closest<HTMLElement>(
        'input, textarea, [contenteditable="true"], [contenteditable=""]',
      );
      if (editable && document.activeElement !== editable) {
        editable.focus();
      }
    },
    true,
  );
}
