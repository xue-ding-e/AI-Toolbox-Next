import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider } from 'antd';
import i18n from '@/i18n';
import GatewayRequestsView from '@/features/coding/gateway/components/GatewayRequestsView';
import { observationDetails, observationRows } from './gatewayRequestObservationsData';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
document.documentElement.dataset.theme = 'light';
await i18n.changeLanguage('en-US');

// Reuse only synthetic DTO seeds, not the observation fixture or its checks.
const detail = {
  ...observationDetails[0],
  trace_id: 'fixture-export-detail',
  native_tools: [],
  rewrites: [],
};
const row = { ...observationRows[0], trace_id: detail.trace_id };
const state = {
  runId: parameters.get('runId'),
  scenario: parameters.get('scenario'),
  requests: [],
  unexpected: [],
  pendingPickers: 0,
  pendingExports: 0,
};
const pickers = [];
const exports = [];
const exportPath = '/synthetic/gateway-request-export.json';

window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.requests.push({ command, args: structuredClone(args) });
    if (command === 'proxy_gateway_request_logs') {
      return { data: [structuredClone(row)], total: 1, page: 0, page_size: 20 };
    }
    if (command === 'proxy_gateway_request_log_detail' && args.traceId === detail.trace_id) {
      return structuredClone(detail);
    }
    if (command === 'proxy_gateway_session_usage_rebuild_progress') return null;
    // The real @tauri-apps/plugin-dialog@2.6.0 save wrapper reaches this IPC.
    // Delayed promises model the JS boundary, not a native dialog's modality.
    if (command === 'plugin:dialog|save') {
      if (state.scenario === 'cancel') return null;
      if (state.scenario === 'picker-rejected') throw new Error('Synthetic picker rejection');
      if (state.scenario === 'picker-pending') {
        state.pendingPickers += 1;
        return new Promise(resolve => pickers.push(resolve));
      }
      return exportPath;
    }
    if (command === 'proxy_gateway_export_request_log_detail') {
      if (state.scenario === 'export-pending') {
        state.pendingExports += 1;
        return new Promise(resolve => exports.push(resolve));
      }
      return;
    }
    // No native/backend fallback, filesystem write, or external request.
    state.unexpected.push({ command, args: structuredClone(args) });
    throw new Error('Unexpected export fixture IPC: ' + command);
  },
};

const exportButton = () => {
  const buttons = [...document.querySelectorAll('[role="dialog"] button')].filter(button =>
    button.getAttribute('aria-label') === i18n.t('gateway.page.requests.exportDetail'));
  if (buttons.length !== 1) throw new Error('Expected one real detail export button');
  return buttons[0];
};
const visibleText = selector => [...document.querySelectorAll(selector)]
  .filter(element => element.getClientRects().length > 0
    && getComputedStyle(element).display !== 'none'
    && getComputedStyle(element).visibility !== 'hidden')
  .map(element => element.textContent.trim()).join('\n');

window.requestExportFixture = {
  state,
  openDetail() {
    const rows = document.querySelectorAll('[data-row-key="fixture-export-detail"]');
    if (rows.length !== 1) throw new Error('Expected one synthetic request row');
    rows[0].click();
  },
  clickExport() { exportButton().click(); },
  exportDisabled() { return exportButton().disabled; },
  errors() { return visibleText('[role="alert"]'); },
  notices() { return visibleText('[role="status"]'); },
  resolvePickers() {
    if (!pickers.length) throw new Error('No pending picker to cancel');
    for (const resolve of pickers.splice(0)) resolve(null);
    state.pendingPickers = 0;
  },
  resolveExports() {
    if (!exports.length) throw new Error('No pending export to complete');
    for (const resolve of exports.splice(0)) resolve();
    state.pendingExports = 0;
  },
};

// No replacement export handler or unhandledrejection/error listener is installed.
createRoot(document.getElementById('root')).render(
  <ConfigProvider>
    <App><main style={{ padding: 16 }}><GatewayRequestsView /></main></App>
  </ConfigProvider>,
);
