import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider } from 'antd';
import i18n from '@/i18n';
import GatewayRequestsView from '@/features/coding/gateway/components/GatewayRequestsView';
import { observationDetails, observationRows } from './gatewayRequestObservationsData';
import '@/App.css';

document.documentElement.dataset.theme = 'light';
await i18n.changeLanguage('en-US');

const parameters = new URLSearchParams(location.search);
const details = ['a', 'b'].map(suffix => ({
  ...observationDetails[0],
  trace_id: 'fixture-owner-' + suffix,
  native_tools: [],
  rewrites: [],
}));
const rows = details.map(detail => ({ ...observationRows[0], trace_id: detail.trace_id }));
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
const paths = { old: '/synthetic/old-detail.json', new: '/synthetic/new-detail.json' };

window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.requests.push({ command, args: structuredClone(args) });
    if (command === 'proxy_gateway_request_logs') {
      return { data: structuredClone(rows), total: 2, page: 0, page_size: 20 };
    }
    if (command === 'proxy_gateway_request_log_detail') {
      const detail = details.find(item => item.trace_id === args.traceId);
      if (detail) return structuredClone(detail);
    }
    if (command === 'proxy_gateway_session_usage_rebuild_progress') return null;
    if (command === 'plugin:dialog|save') {
      state.pendingPickers += 1;
      return new Promise(resolve => pickers.push({ resolve, settled: false }));
    }
    if (command === 'proxy_gateway_export_request_log_detail') {
      state.pendingExports += 1;
      return new Promise((resolve, reject) => exports.push({ resolve, reject, settled: false }));
    }
    state.unexpected.push({ command, args: structuredClone(args) });
    throw new Error('Unexpected export ownership fixture IPC: ' + command);
  },
};

const detailButton = key => {
  const buttons = [...document.querySelectorAll('[role="dialog"] button')]
    .filter(button => button.getAttribute('aria-label') === i18n.t(key));
  if (buttons.length !== 1) throw new Error('Expected one real detail button: ' + key);
  return buttons[0];
};
const visibleText = selector => [...document.querySelectorAll(selector)]
  .filter(element => element.getClientRects().length > 0
    && getComputedStyle(element).display !== 'none'
    && getComputedStyle(element).visibility !== 'hidden')
  .map(element => element.textContent.trim()).join('\n');
const pendingAt = (queue, index) => {
  const pending = queue[index];
  if (!pending || pending.settled) throw new Error('No pending synthetic operation at index ' + index);
  pending.settled = true;
  return pending;
};

window.requestExportOwnershipFixture = {
  state,
  openDetail(traceId) {
    const row = document.querySelector('[data-row-key="' + traceId + '"]');
    if (!row) throw new Error('Missing synthetic request row: ' + traceId);
    // This invokes the real onRow action, including when switching details.
    // Programmatic DOM clicks do not establish native/modal hit-test behavior.
    row.click();
  },
  selectedTrace() {
    return [...document.querySelectorAll('[role="dialog"] code')]
      .find(element => details.some(detail => detail.trace_id === element.textContent))?.textContent ?? null;
  },
  closeDetail() { detailButton('common.close').click(); },
  clickExport() { detailButton('gateway.page.requests.exportDetail').click(); },
  exportDisabled() { return detailButton('gateway.page.requests.exportDetail').disabled; },
  errors() { return visibleText('[role="alert"]'); },
  notices() { return visibleText('[role="status"]'); },
  unmountView() { document.querySelector('[data-fixture-unmount]').click(); },
  mountView() { document.querySelector('[data-fixture-mount]').click(); },
  resolvePicker(index, destination) {
    if (!Object.hasOwn(paths, destination)) throw new Error('Unknown synthetic destination');
    pendingAt(pickers, index).resolve(paths[destination]);
    state.pendingPickers -= 1;
  },
  settleExport(index, outcome) {
    if (outcome !== 'success' && outcome !== 'reject') throw new Error('Unknown synthetic export outcome');
    const pending = pendingAt(exports, index);
    if (outcome === 'reject') pending.reject(new Error('Synthetic detached export rejection'));
    else pending.resolve();
    state.pendingExports -= 1;
  },
};

function FixtureHost() {
  const [mounted, setMounted] = React.useState(true);
  return (
    <ConfigProvider>
      <App>
        <button data-fixture-unmount onClick={() => setMounted(false)}>Unmount fixture view</button>
        <button data-fixture-mount onClick={() => setMounted(true)}>Mount fixture view</button>
        <main style={{ padding: 16 }}>{mounted ? <GatewayRequestsView /> : null}</main>
      </App>
    </ConfigProvider>
  );
}

// Host controls only mount/unmount the real view. No component handler or state
// is replaced, and rejected operations retain the shared runner's exception gate.
createRoot(document.getElementById('root')).render(<FixtureHost />);
