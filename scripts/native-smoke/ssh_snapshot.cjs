// Observe only this disposable profile; never create or mutate a database.
const assert = require('node:assert/strict');
const { DatabaseSync } = require('node:sqlite');
const db = new DatabaseSync(process.argv[2], { readOnly: true });
try {
  const config = db.prepare("SELECT json_extract(data, '$.enabled') enabled, json_type(data, '$.enabled') enabled_type, json_extract(data, '$.active_connection_id') active, json_type(data, '$.active_connection_id') active_type FROM ssh_sync_config WHERE id = 'config'").get();
  if (config) {
    assert.ok(['true', 'false'].includes(config.enabled_type), 'Existing config must contain boolean enabled');
    assert.equal(config.active_type, 'text', 'Existing config must contain string active_connection_id');
  }
  const count = db.prepare('SELECT count(*) count FROM ssh_connection').get().count;
  // get_ssh_config_internal returns SSHSyncConfig::default() for no config row.
  // Missing/malformed fields IN an existing row are deliberately not defaulted.
  console.log(JSON.stringify({ sqliteVersion: db.prepare('select sqlite_version() v').get().v, configPresent: !!config, count, enabled: config ? config.enabled : 0, active: config ? config.active : '' }));
} finally { db.close(); }
