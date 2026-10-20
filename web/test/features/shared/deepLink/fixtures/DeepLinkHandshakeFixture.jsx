import React from 'react';
import { createRoot } from 'react-dom/client';
import { useDeepLinkImport } from '@/features/shared/deepLink/useDeepLinkImport';
import { boundary } from './tauri.mjs';
const onError = () => {};
let commitBarrier;
let completeBarrier;
let mounted = true;
function Fixture() {
  const { request, dismiss } = useDeepLinkImport(onError);
  const [barrier, setBarrier] = React.useState(0);
  commitBarrier = () => new Promise(resolve => {
    completeBarrier = resolve;
    // Same root and update priority as the hook's earlier normal state update.
    setBarrier(value => value + 1);
  });
  React.useLayoutEffect(() => { completeBarrier?.(); completeBarrier = undefined; }, [barrier]);
  return <div data-testid="commit-barrier" data-value={barrier}>
    <output data-testid="request">{request?.name || 'none'}</output>
    <button data-testid="dismiss" onClick={dismiss}>Dismiss</button></div>;
}
const root = createRoot(document.getElementById('root'));
root.render(<Fixture />);
window.deepLinkHandshakeFixture = {
  ...boundary,
  async settle() {
    // The hook registered its await first. After that continuation, enqueue a
    // same-root update and wait for its layout effect, not an arbitrary sleep.
    await boundary.settled();
    if (mounted) await commitBarrier();
  },
  unmount() { mounted = false; root.unmount(); },
  snapshot() { return { ...boundary.snapshot(), caseId: new URLSearchParams(location.search).get('case'), request: document.querySelector('[data-testid="request"]')?.textContent ?? null, barrier: Number(document.querySelector('[data-testid="commit-barrier"]')?.dataset.value ?? 0) }; },
};
