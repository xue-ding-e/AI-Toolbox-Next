// Only app/routes is substituted. Real GrokPage, modal, children and IPC stay intact.
// No navigation/share/deep-link behavior is supported or represented as covered.
const unexpected = [];
function reject(operation) {
  // Never retain navigation destinations, state or arguments in diagnostic evidence.
  unexpected.push(operation);
  throw new Error('Unexpected routing operation in Grok editor-only fixture: ' + operation);
}
const target = Object.freeze({ navigate() { return reject('navigate'); } });
export const router = new Proxy(target, {
  get(object, property) {
    if (property === 'navigate') return object.navigate;
    return reject('property-read');
  },
  set() { return reject('property-write'); },
  defineProperty() { return reject('property-definition'); },
  deleteProperty() { return reject('property-delete'); },
});
export function routingSnapshot() { return { unexpected: [...unexpected] }; }
