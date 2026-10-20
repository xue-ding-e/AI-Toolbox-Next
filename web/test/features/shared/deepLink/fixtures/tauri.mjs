// Synthetic service boundary only: no native import, real URL opening or writes.
const listeners = new Map();
let resolveReady, rejectReady, readyCalls = 0;
const ready = new Promise((resolve, reject) => { resolveReady = resolve; rejectReady = reject; });
export async function listen(name, callback) {
  listeners.set(name, callback);
  return () => listeners.delete(name);
}
export function markDeepLinkFrontendReady() { readyCalls++; return ready; }
export function request(name) {
  const fields = { resource: 'provider', app: 'codex', category: 'third_party', name,
    baseUrl: 'https://synthetic.invalid/v1', apiKey: 'synthetic-fixture-not-a-secret',
    model: 'fixture-model', apiFormat: 'openai_responses' };
  return { ...fields, rawUrl: 'aitoolbox://v1/import?' + new URLSearchParams(fields) };
}
export const boundary = {
  emit(name) { const callback = listeners.get('deep-link-import'); if (!callback) throw new Error('Missing listener'); callback({ payload: request(name) }); },
  resolve(name) { resolveReady(request(name)); },
  reject() { rejectReady(new Error('synthetic handshake failure')); },
  async settled() { await ready.catch(() => {}); },
  snapshot() { return { readyCalls, subscriptions: [...listeners.keys()].sort() }; },
};
