import assert from 'node:assert/strict';
import test from 'node:test';

import {
  describeBackupFilename,
  parseBackupFilename,
} from '../../../../features/settings/utils/backupFilename.ts';

test('parses the current layout with optional host label and encryption suffix', () => {
  const plain = parseBackupFilename('ai-toolbox-backup-20260913-120000.zip');
  assert.ok(plain);
  assert.equal(plain.displayTime, '2026-09-13 12:00:00');
  assert.equal(plain.hostLabel, null);
  assert.equal(plain.encrypted, false);

  const withHost = parseBackupFilename('ai-toolbox-backup-20260913-120000_HomeNAS.zip');
  assert.ok(withHost);
  assert.equal(withHost.hostLabel, 'HomeNAS');

  const encrypted =
    parseBackupFilename('ai-toolbox-backup-20260913-120000-abc123ef_HomeNAS.zip.enc');
  assert.ok(encrypted);
  assert.equal(encrypted.hostLabel, 'HomeNAS');
  assert.equal(encrypted.encrypted, true);
});

test('parses new-format unique ids and legacy prefixed names', () => {
  const newFormat = parseBackupFilename('ai-toolbox-backup-20260913-120000-abc123ef.zip');
  assert.ok(newFormat);
  assert.equal(newFormat.displayTime, '2026-09-13 12:00:00');

  const legacy = parseBackupFilename('ai-toolbox-backup-app-1.2.3-20260102-030405.zip');
  assert.ok(legacy);
  assert.equal(legacy.displayTime, '2026-01-02 03:04:05');
  assert.equal(legacy.hostLabel, null);
});

test('rejects foreign filenames so they never appear in the restore list', () => {
  assert.equal(parseBackupFilename('configuration.aitsync'), null);
  assert.equal(parseBackupFilename('other-backup-20260913-120000.zip'), null);
  assert.equal(parseBackupFilename('ai-toolbox-backup-notatime.zip'), null);
  assert.equal(parseBackupFilename('ai-toolbox-backup-20260913-120000.zip.enc.enc'), null);
  assert.equal(parseBackupFilename('ai-toolbox-backup-20260913-120000-badid!.zip'), null);
});

test('host labels containing dashes and underscores round trip', () => {
  const parsed = parseBackupFilename('ai-toolbox-backup-20260913-120000-abc123ef_my-host_v2.zip');
  assert.ok(parsed);
  assert.equal(parsed.hostLabel, 'my-host_v2');
});

test('describeBackupFilename falls back to the raw name for unknown files', () => {
  const described = describeBackupFilename('unknown-file.bin');
  assert.equal(described.displayTime, 'unknown-file.bin');
  assert.equal(described.hostLabel, null);
  assert.equal(described.encrypted, false);
});

test('multibyte legacy prefixes and host labels parse like the Rust contract', () => {
  // Mirrors the Rust regression in filename.rs: a legacy prefix containing CJK
  // characters must anchor on the trailing timestamp, not on byte offsets.
  const legacy = parseBackupFilename('ai-toolbox-backup-Windows中文备份-20260102-030405.zip');
  assert.ok(legacy);
  assert.equal(legacy.displayTime, '2026-01-02 03:04:05');
  assert.equal(legacy.hostLabel, null);
  assert.equal(legacy.encrypted, false);

  const withHost =
    parseBackupFilename('ai-toolbox-backup-20260913-120000-abc123ef_工作机.zip.enc');
  assert.ok(withHost);
  assert.equal(withHost.hostLabel, '工作机');
  assert.equal(withHost.encrypted, true);

  // Foreign names that merely contain the prefix characters stay unmanaged.
  assert.equal(parseBackupFilename('ai-toolbox-backup-中文备份.zip'), null);
});
