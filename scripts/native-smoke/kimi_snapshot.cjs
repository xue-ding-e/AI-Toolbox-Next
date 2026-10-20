// Read only narrowly selected non-secret fields. Never select data/settings_config.
const assert = require('node:assert/strict');
const { DatabaseSync } = require('node:sqlite');
const [file, expectedId = '', namesJson = '[]'] = process.argv.slice(2);
const allowedNames = JSON.parse(namesJson);
const db = new DatabaseSync(file, { readOnly: true });
try {
  const count = db.prepare('SELECT count(*) n FROM kimi_provider').get().n;
  const accounts = db.prepare('SELECT count(*) n FROM kimi_official_account').get().n;
  assert.equal(accounts, 0, 'Unexpected account exists; stop without reading it');
  assert.ok(count <= 1, 'Unexpected provider count; stop');
  const commonRoots = db.prepare("SELECT count(*) n FROM kimi_common_config WHERE json_type(data, '$.root_dir') IS NOT NULL").get().n;
  assert.equal(commonRoots, 0, 'Custom root field exists; stop without reading path');
  let row = null;
  if (count) {
    assert.ok(allowedNames.length, 'Existing provider before creation; stop');
    row = db.prepare("SELECT id, json_extract(data,'$.name') name, json_type(data,'$.name') name_type, json_extract(data,'$.category') category, json_type(data,'$.is_applied') applied_type FROM kimi_provider").get();
    assert.equal(row.name_type, 'text', 'Invalid name type');
    assert.ok(allowedNames.includes(row.name), 'Unexpected non-synthetic provider; stop');
    assert.equal(row.category, 'official', 'Unexpected category');
    assert.equal(row.applied_type, 'false', 'Provider applied or malformed; stop');
    assert.equal(typeof row.id, 'string'); assert.ok(/^[A-Za-z0-9_-]+$/.test(row.id), 'Unexpected identifier');
    if (expectedId) assert.equal(row.id, expectedId, 'Record identity changed');
  }
  console.log(JSON.stringify({ count, accounts, customRootFields: commonRoots, row:row ? {id:row.id,name:row.name,category:row.category,isApplied:false}:null }));
} finally { db.close(); }
