// Temporary synthetic fixture only. Run before building the native application.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { DatabaseSync } = require('node:sqlite');
assert.equal(process.version, process.argv[2], 'Unexpected Node version');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ai-sqlite-preflight-'));
const file = path.join(root, 'fixture.db');
let db;
try {
  db = new DatabaseSync(file);
  db.exec('CREATE TABLE ssh_sync_config(id TEXT PRIMARY KEY, data BLOB); CREATE TABLE ssh_connection(id TEXT PRIMARY KEY, data BLOB)');
  const observe = () => spawnSync(process.execPath, [path.join(__dirname, 'ssh_snapshot.cjs'), file], { encoding:'utf8', timeout:10000 });
  const check = expected => { const r=observe(); assert.equal(r.status,0,r.stderr); const value=JSON.parse(r.stdout); for(const [k,v] of Object.entries(expected)) assert.equal(value[k],v); };
  check({configPresent:false,count:0,enabled:0,active:''});
  const put = data => db.prepare("INSERT OR REPLACE INTO ssh_sync_config VALUES ('config',jsonb(?))").run(JSON.stringify(data));
  put({enabled:true,active_connection_id:''}); check({configPresent:true,count:0,enabled:1,active:''});
  put({enabled:false,active_connection_id:'synthetic-nonempty'}); check({active:'synthetic-nonempty'});
  for (const bad of [{enabled:'false',active_connection_id:''},{enabled:false},{enabled:false,active_connection_id:null}]) {
    put(bad); assert.notEqual(observe().status,0,'Malformed existing fields must fail');
  }
  db.prepare("UPDATE ssh_sync_config SET data = x'FF'").run();
  assert.notEqual(observe().status,0,'Invalid JSONB must fail');
  const readOnly = new DatabaseSync(file,{readOnly:true});
  try { assert.throws(()=>readOnly.exec("INSERT INTO ssh_connection VALUES ('forbidden',NULL)"), /readonly/i); }
  finally { readOnly.close(); }
  db.exec('DROP TABLE ssh_connection');
  assert.notEqual(observe().status,0,'Missing schema must fail');
  const version=db.prepare('select sqlite_version() v').get().v;
  db.close(); db=null;
  const absent=path.join(root,'must-not-be-created.db');
  assert.throws(()=>new DatabaseSync(absent,{readOnly:true})); assert.equal(fs.existsSync(absent),false);
  console.log(JSON.stringify({node:process.version,sqlite:version,passed:['missing config defaults','JSONB valid types','nonempty active preserved','invalid types rejected','invalid JSONB rejected','missing schema rejected','write rejected','missing database not created']}));
} finally { if(db) db.close(); fs.rmSync(root,{recursive:true,force:true}); }
