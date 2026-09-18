/**
 * Process-wide coordination for mutations that rewrite the aggregate gateway
 * manifest and Codex model catalog.
 *
 * Aggregate edits and provider-save re-engagement share this lane so a stale
 * snapshot cannot interleave with a newer aggregate edit.
 */

let aggregateMutationTail: Promise<void> = Promise.resolve();

export const runGatewayAggregateMutation = <T>(
  mutation: () => Promise<T>,
): Promise<T> => {
  const run = aggregateMutationTail.then(mutation);
  aggregateMutationTail = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
};

// Successful writes notify mounted editors to re-read the backend manifest.
let aggregateConfigVersion = 0;
const aggregateConfigListeners = new Set<() => void>();

export const getGatewayAggregateConfigVersion = () => aggregateConfigVersion;

export const subscribeGatewayAggregateConfig = (listener: () => void) => {
  aggregateConfigListeners.add(listener);
  return () => {
    aggregateConfigListeners.delete(listener);
  };
};

export const notifyGatewayAggregateConfigChanged = () => {
  aggregateConfigVersion += 1;
  for (const listener of aggregateConfigListeners) {
    listener();
  }
};
