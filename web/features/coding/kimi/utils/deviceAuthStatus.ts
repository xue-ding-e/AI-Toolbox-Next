/**
 * Device-code login status semantics shared by the event listener and the
 * polling fallback in `KimiDeviceAuthModal`.
 *
 * Backend statuses: "waiting" -> "completed" | "failed" | "expired" (terminal),
 * plus "cancelled" set locally by the cancel API. Login only saves the account
 * (is_applied=false); apply is a separate action. Only "completed" means the
 * account row is fully written and safe to reload.
 */
export const KIMI_DEVICE_AUTH_SUCCESS_STATUS = 'completed';

export const KIMI_DEVICE_AUTH_TERMINAL_STATUSES: ReadonlySet<string> = new Set([
  KIMI_DEVICE_AUTH_SUCCESS_STATUS,
  'cancelled',
  'expired',
  'denied',
  'access_denied',
  'failed',
  'error',
]);

export function isTerminalKimiDeviceAuthStatus(status: string): boolean {
  return KIMI_DEVICE_AUTH_TERMINAL_STATUSES.has(status);
}

export type KimiDeviceAuthFeedback =
  /** Non-terminal: keep polling, status text may update. */
  | 'progress'
  /** First observation of "completed": notify and reload. */
  | 'notify-success'
  /** First observation of a failure terminal state: notify once. */
  | 'notify-error'
  /** Terminal already reported, or "cancelled": stop silently. */
  | 'terminal-silent';

/**
 * The status event listener and the 5s polling fallback can both observe the
 * same terminal status, and polling keeps firing after a failure until the
 * modal unmounts. Each terminal state must therefore classify as a
 * notification at most once per auth session.
 */
export function createKimiDeviceAuthStatusClassifier(): (
  status: string,
) => KimiDeviceAuthFeedback {
  let terminalHandled = false;
  return (status: string) => {
    if (!isTerminalKimiDeviceAuthStatus(status)) {
      return 'progress';
    }
    if (terminalHandled) {
      return 'terminal-silent';
    }
    terminalHandled = true;
    if (status === KIMI_DEVICE_AUTH_SUCCESS_STATUS) {
      return 'notify-success';
    }
    // "cancelled" is user-initiated; closing the modal is feedback enough.
    return status === 'cancelled' ? 'terminal-silent' : 'notify-error';
  };
}
