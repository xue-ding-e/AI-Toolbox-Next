export const hasCompleteModelLimitPair = (
  contextLimit: number | undefined,
  outputLimit: number | undefined,
): boolean => (contextLimit === undefined) === (outputLimit === undefined);
