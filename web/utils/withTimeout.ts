/**
 * Race a promise against a wall-clock timeout.
 * On success or failure of `promise`, the timer is cleared so it does not leak.
 * Timeout rejects with `Error(timeoutMessage)`.
 */
export function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  timeoutMessage: string,
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;

  const timeoutPromise = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => {
      reject(new Error(timeoutMessage));
    }, timeoutMs);
  });

  return Promise.race([promise, timeoutPromise]).finally(() => {
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
    }
  });
}

/** Frontend safety net for common-config extract; backend disk I/O times out at 10s. */
export const COMMON_CONFIG_EXTRACT_TIMEOUT_MS = 15_000;
