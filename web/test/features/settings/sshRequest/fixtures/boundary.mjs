const connections=['A','B'].map(id=>({id,name:'Connection '+id,host:id.toLowerCase()+'.invalid',port:22,username:'synthetic',authMethod:'none',password:'',privateKeyPath:'',privateKeyContent:'',passphrase:'',sortOrder:id==='A'?0:1}));
let config={enabled:true,activeConnectionId:'A',syncMcp:false,syncSkills:false,lastSyncStatus:'never',connections,fileMappings:[{id:'synthetic-disabled',name:'Synthetic mapping',module:'claude',enabled:false,localPath:'/synthetic/source',remotePath:'/synthetic/target',isDirectory:false,isPattern:false,directoryExcludes:[],cleanupPaths:[]}],moduleStatuses:[]};
const listeners=new Map();const calls=[];let forbidden=0;let revision=0;let reads=0;
export const useSettingsStore=Object.assign(()=>({visibleTabs:[]}),{getState:()=>({visibleTabs:[]})});
export async function listen(name,callback){const set=listeners.get(name)||new Set();set.add(callback);listeners.set(name,set);return()=>set.delete(callback);}
function emit(){revision++;for(const cb of listeners.get('ssh-config-changed')||[])cb({payload:null});}
export async function sshGetConfig(){reads++;return structuredClone(config);}
export async function sshGetStatus(){reads++;return {sshAvailable:true,activeConnectionName:config.activeConnectionId,lastSyncStatus:'',lastSyncError:'',lastSyncWarnings:[]};}
export async function sshSetActiveConnection(id){config={...config,activeConnectionId:id};emit();}
export async function sshSaveConfig(value){config=structuredClone(value);emit();}
export function sshTestConnection(conn){return new Promise((resolve,reject)=>calls.push({id:conn.id,resolve,reject,settled:false}));}
const deny=()=>{forbidden++;throw Error('Unexpected non-test SSH operation');};
export const sshGetDefaultMappings=deny,sshSync=deny,sshDeleteFileMapping=deny,sshResetFileMappings=deny,sshCreateConnection=deny,sshUpdateConnection=deny,sshDeleteConnection=deny,sshAddFileMapping=deny,sshUpdateFileMapping=deny;
export const invoke=deny;
export const boundary={
 snapshot:()=>({revision,reads,calls:calls.map(({id,settled})=>({id,settled})),forbidden}),
 settle(indices,reject=false){const pending=indices.map(i=>calls[i]);if(!pending.length||pending.some(c=>!c||c.settled))throw Error('Invalid pending call indices');for(const c of pending){c.settled=true;if(reject)c.reject(new Error('synthetic-'+c.id+'-error'));else c.resolve({connected:true,serverInfo:c.id+'-result'});}return Promise.resolve();},
};
