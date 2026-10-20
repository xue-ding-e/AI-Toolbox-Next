// Synthetic SQLite JSONB observer controls, before native build. No app IPC.
const assert=require('node:assert/strict'), fs=require('node:fs'), os=require('node:os'), path=require('node:path');
const {spawnSync}=require('node:child_process'), {DatabaseSync}=require('node:sqlite');
assert.equal(process.version, process.argv[2]);
const root=fs.mkdtempSync(path.join(os.tmpdir(),'ai-kimi-observer-')), file=path.join(root,'fixture.db');
let db;
try {
 db=new DatabaseSync(file);
 db.exec('CREATE TABLE kimi_provider(id TEXT PRIMARY KEY,data BLOB); CREATE TABLE kimi_official_account(id TEXT PRIMARY KEY,data BLOB); CREATE TABLE kimi_common_config(id TEXT PRIMARY KEY,data BLOB)');
 const observe=(id='',names=[])=>spawnSync(process.execPath,[path.join(__dirname,'kimi_snapshot.cjs'),file,id,JSON.stringify(names)],{encoding:'utf8',timeout:10000});
 const ok=(id='',names=[])=>{const r=observe(id,names);assert.equal(r.status,0,'Observer rejected synthetic control');assert.ok(!r.stdout.includes('must-not-be-output'),'Secret fixture leaked to stdout');return JSON.parse(r.stdout);};
 const bad=(id='',names=[])=>assert.notEqual(observe(id,names).status,0,'Unsafe/malformed fixture must fail');
 const put=value=>db.prepare("INSERT OR REPLACE INTO kimi_provider VALUES('synthetic-id',jsonb(?))").run(JSON.stringify(value));
 assert.equal(ok().count,0);
 put({name:'synthetic-A',category:'official',is_applied:false,settings_config:'must-not-be-output'});
 bad(); assert.equal(ok('synthetic-id',['synthetic-A']).row.name,'synthetic-A'); bad('wrong-id',['synthetic-A']); bad('', ['other-name']);
 for(const applied of [true,0,'false',null]){put({name:'synthetic-A',category:'official',is_applied:applied});bad('', ['synthetic-A']);}
 db.exec('DELETE FROM kimi_provider');
 db.prepare("INSERT INTO kimi_common_config VALUES('common',jsonb(?))").run(JSON.stringify({config:'must-not-be-output',updated_at:'synthetic'}));ok();db.exec('DELETE FROM kimi_common_config');
 db.prepare("INSERT INTO kimi_common_config VALUES('common',jsonb(?))").run(JSON.stringify({root_dir:null}));bad();db.exec('DELETE FROM kimi_common_config');
 db.prepare("INSERT INTO kimi_common_config VALUES('common',jsonb(?))").run(JSON.stringify({root_dir:'/synthetic-only'}));bad();db.exec('DELETE FROM kimi_common_config');
 db.exec("INSERT INTO kimi_official_account VALUES('synthetic-account',NULL)");bad();db.exec('DELETE FROM kimi_official_account');
 db.exec("INSERT INTO kimi_provider VALUES('malformed',x'FF')");bad('', ['synthetic-A']);db.exec('DELETE FROM kimi_provider');
 const ro=new DatabaseSync(file,{readOnly:true});try{assert.throws(()=>ro.exec('DELETE FROM kimi_provider'),/readonly/i);}finally{ro.close();}
 db.exec('DROP TABLE kimi_provider');bad();
 console.log(JSON.stringify({node:process.version,passed:['empty','synthetic record','identity guard','name guard','applied type guard','root field guard','root omitted accepted','root null rejected','secret stdout exclusion','account guard','invalid JSONB','readonly','missing schema']}));
} finally {if(db)db.close();fs.rmSync(root,{recursive:true,force:true});}
