/**
 * Shared backup filename parsing for the remote backup list UI (WebDAV/repository).
 *
 * Mirrors the backend contract in `tauri/src/settings/backup/filename.rs`:
 * - new:      `ai-toolbox-backup-<YYYYMMDD>-<HHMMSS>-<unique8>[_<host>].zip[.enc]`
 * - current:  `ai-toolbox-backup-<YYYYMMDD>-<HHMMSS>[_<host>].zip`
 * - legacy:   `ai-toolbox-backup-<anything>-<YYYYMMDD>-<HHMMSS>.zip`
 */

export const BACKUP_FILENAME_PREFIX = 'ai-toolbox-backup-';
const TIMESTAMP_LENGTH = 15;

export interface ParsedBackupFilename {
  displayTime: string;
  hostLabel: string | null;
  encrypted: boolean;
}

function isTimestamp(value: string): boolean {
  if (value.length !== TIMESTAMP_LENGTH || value[8] !== '-') {
    return false;
  }
  return value.split('').every((character, index) => index === 8 || /\d/.test(character));
}

function formatDisplayTime(timestamp: string): string {
  return `${timestamp.slice(0, 4)}-${timestamp.slice(4, 6)}-${timestamp.slice(6, 8)} ${timestamp.slice(9, 11)}:${timestamp.slice(11, 13)}:${timestamp.slice(13, 15)}`;
}

export function parseBackupFilename(filename: string): ParsedBackupFilename | null {
  const encrypted = filename.endsWith('.zip.enc');
  const stem = encrypted
    ? filename.slice(0, -'.zip.enc'.length)
    : filename.endsWith('.zip')
      ? filename.slice(0, -'.zip'.length)
      : null;
  if (stem === null || !stem.startsWith(BACKUP_FILENAME_PREFIX)) {
    return null;
  }
  const body = stem.slice(BACKUP_FILENAME_PREFIX.length);

  // New/current layouts open with the fixed-width timestamp.
  if (body.length >= TIMESTAMP_LENGTH && isTimestamp(body.slice(0, TIMESTAMP_LENGTH))) {
    const timestamp = body.slice(0, TIMESTAMP_LENGTH);
    const rest = body.slice(TIMESTAMP_LENGTH);
    if (rest === '') {
      return { displayTime: formatDisplayTime(timestamp), hostLabel: null, encrypted };
    }
    if (rest.startsWith('-')) {
      // New layout: -<unique8>[_<host>]
      const suffix = rest.slice(1);
      const separatorIndex = suffix.indexOf('_');
      const unique = separatorIndex === -1 ? suffix : suffix.slice(0, separatorIndex);
      if (!/^[0-9a-fA-F]{8}$/.test(unique)) {
        return null;
      }
      const host =
        separatorIndex === -1 ? null : suffix.slice(separatorIndex + 1).trim() || null;
      return { displayTime: formatDisplayTime(timestamp), hostLabel: host, encrypted };
    }
    if (rest.startsWith('_')) {
      const host = rest.slice(1).trim() || null;
      return { displayTime: formatDisplayTime(timestamp), hostLabel: host, encrypted };
    }
    return null;
  }

  // Legacy layout: <anything>-<timestamp>
  if (body.length > TIMESTAMP_LENGTH + 1) {
    const timestamp = body.slice(body.length - TIMESTAMP_LENGTH);
    const prefix = body.slice(0, body.length - TIMESTAMP_LENGTH);
    if (prefix.endsWith('-') && prefix.slice(0, -1).length > 0 && isTimestamp(timestamp)) {
      return { displayTime: formatDisplayTime(timestamp), hostLabel: null, encrypted };
    }
  }
  return null;
}

/**
 * Fallback for unknown names: show the raw filename with no metadata.
 */
export function describeBackupFilename(filename: string): ParsedBackupFilename {
  return (
    parseBackupFilename(filename) ?? { displayTime: filename, hostLabel: null, encrypted: false }
  );
}
