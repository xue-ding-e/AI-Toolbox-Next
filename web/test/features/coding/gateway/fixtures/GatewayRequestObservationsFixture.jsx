import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider } from 'antd';
import i18n from '@/i18n';
import GatewayRequestsView from '@/features/coding/gateway/components/GatewayRequestsView';
import { observationDetails, observationRows } from './gatewayRequestObservationsData';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
document.documentElement.dataset.theme = 'light';
await i18n.changeLanguage(parameters.get('language') || 'zh-CN');

const state = { runId: parameters.get('runId'), requests: [], unexpected: [] };
window.__observationExecuted = false;
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.requests.push({ command, args: structuredClone(args) });
    if (command === 'proxy_gateway_request_logs') {
      return { data: structuredClone(observationRows), total: observationRows.length, page: 0, page_size: 20 };
    }
    if (command === 'proxy_gateway_request_log_detail') {
      const detail = observationDetails.find(item => item.trace_id === args.traceId);
      if (detail) return structuredClone(detail);
    }
    // The real view polls this read-only status on every mount. No rebuild is
    // running in the fixture, and starting/syncing one is never permitted.
    if (command === 'proxy_gateway_session_usage_rebuild_progress') return null;
    // There is deliberately no fallback to Tauri, the network, or user files.
    state.unexpected.push({ command, args: structuredClone(args) });
    throw new Error('Unexpected fixture IPC: ' + command);
  },
};

window.requestObservationsFixture = {
  state,
  open(scenario) {
    const rows = document.querySelectorAll('[data-row-key="fixture-observations-' + scenario + '"]');
    if (rows.length !== 1) throw new Error('Expected one request row: ' + scenario);
    rows[0].click();
  },
  click(key) {
    const dialog = document.querySelector('[role="dialog"]');
    const label = i18n.t(key);
    const buttons = [...(dialog?.querySelectorAll('button') ?? [])].filter(button =>
      button.getAttribute('aria-label') === label || button.textContent.trim() === label);
    if (buttons.length !== 1) throw new Error('Expected one detail control: ' + key);
    buttons[0].click();
  },
  section(label) {
    const dialog = document.querySelector('[role="dialog"]');
    if (!dialog) throw new Error('Request detail dialog is not mounted');
    const labels = [...dialog.querySelectorAll('span')].filter(span => span.textContent === label);
    const rows = labels[0]?.nextElementSibling?.querySelectorAll(':scope > strong') ?? [];
    return { count: labels.length, rows: [...rows].map(row => ({ text: row.textContent, title: row.getAttribute('title') })) };
  },
};

// The real component owns all list/detail loading, tab selection, and rendering.
// The runner narrows only the services barrel to the real proxyGatewayApi module.
createRoot(document.getElementById('root')).render(
  <ConfigProvider>
    <App><main style={{ padding: 16 }}><GatewayRequestsView /></main></App>
  </ConfigProvider>,
);
