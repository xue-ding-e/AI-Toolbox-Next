import React from 'react';
import { getProxyGatewaySupportedCliKeys } from '@/services/proxyGatewayApi';
import type { GatewayCliKey } from '@/services/proxyGatewayApi';

/**
 * Process-wide cache of the gateway-supported CLI keys.
 *
 * The list is static for a given backend build, so one successful fetch per
 * session is enough; every provider form shares this promise instead of
 * issuing its own command on mount. Failures are not cached, so a later mount
 * retries.
 */
let supportedCliKeysPromise: Promise<Set<GatewayCliKey>> | null = null;

const loadSupportedCliKeys = (): Promise<Set<GatewayCliKey>> => {
  if (!supportedCliKeysPromise) {
    supportedCliKeysPromise = getProxyGatewaySupportedCliKeys()
      .then((keys) => new Set(keys))
      .catch((error) => {
        // Let the next caller retry instead of caching a failed lookup.
        supportedCliKeysPromise = null;
        throw error;
      });
  }
  return supportedCliKeysPromise;
};

/**
 * Whether the gateway can take over `cliKey`.
 *
 * Returns `undefined` only while the list is still loading. A failed lookup
 * keeps retrying on an interval rather than resolving to "supported": the
 * callers' `!== false` idiom treats an unresolved answer as "leave the control
 * enabled", so a permanent `undefined` after one transient IPC error would
 * silently disable the gate for the rest of the session.
 */
const RETRY_DELAY_MS = 5000;

export const useGatewaySupportedCliKeys = (): {
  isGatewaySupported: (cliKey: GatewayCliKey) => boolean | undefined;
} => {
  const [supportedCliKeys, setSupportedCliKeys] = React.useState<Set<GatewayCliKey> | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;

    const attempt = () => {
      loadSupportedCliKeys()
        .then((keys) => {
          if (!cancelled) {
            setSupportedCliKeys(keys);
          }
        })
        .catch(() => {
          if (!cancelled) {
            retryTimer = setTimeout(attempt, RETRY_DELAY_MS);
          }
        });
    };

    attempt();
    return () => {
      cancelled = true;
      if (retryTimer !== undefined) {
        clearTimeout(retryTimer);
      }
    };
  }, []);

  return React.useMemo(
    () => ({
      isGatewaySupported: (cliKey: GatewayCliKey) =>
        supportedCliKeys ? supportedCliKeys.has(cliKey) : undefined,
    }),
    [supportedCliKeys],
  );
};
