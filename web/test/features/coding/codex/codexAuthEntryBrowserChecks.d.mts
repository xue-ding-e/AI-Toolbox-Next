/** Narrow CDP transport boundary used by the synthetic browser regression. */
export type SendBrowserCommand = (
  method: string,
  packet: Record<string, unknown>,
) => Promise<unknown>;

export function dispatchAuthEntryKey(
  send: SendBrowserCommand,
  value: 'Enter' | 'Escape',
): Promise<void>;

export function verifyCodexAuthEntry(context: {
  send: SendBrowserCommand;
  evaluate: (expression: string) => Promise<unknown>;
  baseUrl: string;
}): Promise<string[]>;
