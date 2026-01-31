import type { TFunction } from 'i18next';

/**
 * Git error codes and their i18n keys
 */
const GIT_ERROR_CODES = {
  GIT_NOT_FOUND: 'skills.errors.gitNotFound',
  GIT_TIMEOUT: 'skills.errors.gitTimeout',
  GIT_COMMAND_FAILED: 'skills.errors.gitCommandFailed',
  GIT_FETCH_FAILED: 'skills.errors.gitFetchFailed',
  GIT_CLONE_FAILED: 'skills.errors.gitCloneFailed',
  GIT_CHECKOUT_FAILED: 'skills.errors.gitCheckoutFailed',
  GIT_RESET_FAILED: 'skills.errors.gitResetFailed',
  GIT_REVPARSE_FAILED: 'skills.errors.gitRevParseFailed',
} as const;

type GitErrorCode = keyof typeof GIT_ERROR_CODES;

interface ParsedGitError {
  code: GitErrorCode | null;
  params: Record<string, string>;
  details: string;
}

/**
 * Parse git error message from backend
 * Format: ERROR_CODE|param1|param2|...
 */
export function parseGitError(errorMsg: string): ParsedGitError {
  const parts = errorMsg.split('|');
  const code = parts[0] as GitErrorCode;

  if (!(code in GIT_ERROR_CODES)) {
    return { code: null, params: {}, details: errorMsg };
  }

  const params: Record<string, string> = {};
  let details = '';

  switch (code) {
    case 'GIT_TIMEOUT':
      // GIT_TIMEOUT|seconds|stderr
      params.seconds = parts[1] || '';
      details = parts.slice(2).join('|');
      break;
    case 'GIT_CLONE_FAILED':
      // GIT_CLONE_FAILED|url|stderr
      params.url = parts[1] || '';
      details = parts.slice(2).join('|');
      break;
    case 'GIT_CHECKOUT_FAILED':
      // GIT_CHECKOUT_FAILED|branch|stderr
      params.branch = parts[1] || '';
      details = parts.slice(2).join('|');
      break;
    case 'GIT_COMMAND_FAILED':
    case 'GIT_FETCH_FAILED':
    case 'GIT_RESET_FAILED':
    case 'GIT_REVPARSE_FAILED':
      // ERROR_CODE|stderr
      details = parts.slice(1).join('|');
      break;
    case 'GIT_NOT_FOUND':
      // No params
      break;
  }

  return { code, params, details };
}

/**
 * Format git error for display
 */
export function formatGitError(errorMsg: string, t: TFunction): string {
  const { code, params, details } = parseGitError(errorMsg);

  if (!code) {
    // Not a known git error code, return as-is
    return errorMsg;
  }

  const i18nKey = GIT_ERROR_CODES[code];
  let message = t(i18nKey, params);

  // Append stderr details if available
  if (details && details.trim()) {
    message += `\n\n---\n${details.trim()}`;
  }

  return message;
}

/**
 * Check if error is a git-related error
 */
export function isGitError(errorMsg: string): boolean {
  const code = errorMsg.split('|')[0];
  return code in GIT_ERROR_CODES;
}
