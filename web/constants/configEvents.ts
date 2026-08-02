export const TRAY_CONFIG_REFRESH_EVENT = 'ai-toolbox:tray-config-refresh';

/**
 * Fired (as a DOM `CustomEvent` with `detail: { app: string; id: string }`)
 * after a deep-link provider import completes, so the matching tool page can
 * re-fetch its provider list without the global dialog needing to know which
 * page is mounted.
 */
export const DEEP_LINK_IMPORT_COMPLETED = 'ai-toolbox:deep-link-import-completed';
