import React from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  markDeepLinkFrontendReady,
  type DeepLinkErrorPayload,
  type DeepLinkImportRequest,
} from '@/services/deeplinkApi';

/**
 * Subscribe to `deep-link-import` / `deep-link-error` events emitted by the
 * backend's `handle_deeplink_url` funnel. Once listeners are attached, the hook
 * marks the frontend ready and drains the latest cold-start pending request.
 *
 * Returns the current pending request (or null), a dismissal callback, and the
 * latest error (or null). The owning dialog renders the confirmation UI and
 * calls `importFromDeeplinkUnified` on confirm.
 */
export interface UseDeepLinkImportResult {
  request: DeepLinkImportRequest | null;
  error: DeepLinkErrorPayload | null;
  dismiss: () => void;
}

export const useDeepLinkImport = (
  onError: (error: DeepLinkErrorPayload) => void,
): UseDeepLinkImportResult => {
  const [request, setRequest] = React.useState<DeepLinkImportRequest | null>(null);
  const [error, setError] = React.useState<DeepLinkErrorPayload | null>(null);

  React.useEffect(() => {
    let unlistenImport: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let active = true;
    // A live event is newer than the cold-start slot drained by the handshake.
    // Its IPC response can arrive later; do not replace (or reopen) that request.
    let receivedLiveRequest = false;

    const setup = async () => {
      unlistenImport = await listen<DeepLinkImportRequest>(
        'deep-link-import',
        (event) => {
          if (active) {
            receivedLiveRequest = true;
            setRequest(event.payload);
          }
        },
      );
      if (!active) {
        unlistenImport();
        return;
      }
      unlistenError = await listen<DeepLinkErrorPayload>('deep-link-error', (event) => {
        if (!active) return;
        setError(event.payload);
        onError(event.payload);
      });
      if (!active) {
        unlistenError();
        return;
      }

      const pending = await markDeepLinkFrontendReady();
      if (active && pending && !receivedLiveRequest) {
        setRequest(pending);
      }
    };

    setup().catch((err) => {
      console.error('Failed to attach deep-link listeners:', err);
    });

    return () => {
      active = false;
      unlistenImport?.();
      unlistenError?.();
    };
  }, [onError]);

  const dismiss = React.useCallback(() => {
    setRequest(null);
    setError(null);
  }, []);

  return { request, error, dismiss };
};
