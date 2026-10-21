import {mockIPC} from '@tauri-apps/api/mocks';
// Restrict this synthetic fixture before dynamically importing the application page.
const policy=document.createElement('meta');policy.httpEquiv='Content-Security-Policy';policy.content="default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self' data:; object-src 'none'; base-uri 'none'; form-action 'none'";document.head.append(policy);
const scenario=new URLSearchParams(location.search).get('scenario')||'auto-empty';
const caseToken=new URLSearchParams(location.search).get('caseToken');
let stored=scenario==='explicit-custom'?'custom':scenario==='explicit-source'||scenario==='save-failure'?'source':'auto';
const calls=[],writes=[],listeners=new Map();let forbidden=0,readCount=0,initialResolve;const pendingWrites=new Map(),writeOutcomes=[];
const skill={id:'synthetic-skill',name:'Synthetic Skill',source_type:'local',source_ref:'/synthetic/source-fixture',central_path:'/synthetic/skills/item',created_at:1,updated_at:1,last_sync_at:null,status:'active',sort_index:0,user_group:null,group_id:null,user_note:null,management_enabled:true,disabled_previous_tools:[],tags:[],description:'Synthetic fixture only',content_hash:null,source_health:'ok',source_error:null,enabled_tools:[],targets:[]};
const groups=scenario==='auto-group'||scenario==='explicit-source'?[{id:'synthetic-group',name:'Synthetic Group',note:null,sort_index:0,created_at:1,updated_at:1}]:[];
if(groups.length)skill.group_id=groups[0].id;
window.open=()=>deny('window.open');
document.addEventListener('securitypolicyviolation',()=>{forbidden++;});
function deny(command){forbidden++;throw Error('Forbidden fixture IPC: '+command);}
mockIPC(async(command,args)=>{
 calls.push(command);
 const exactKeys=keys=>args!==null&&typeof args==='object'&&!Array.isArray(args)&&JSON.stringify(Object.keys(args).sort())===JSON.stringify([...keys].sort());
 const reads=['skills_get_default_view_mode','skills_get_group_mode','skills_get_tool_status','skills_get_managed_skills','skills_get_groups','skills_get_central_repo_path','skills_get_preferred_tools','skills_get_limit_add_more_to_preferred_tools'];
 if(reads.includes(command)&&args!==undefined&&!exactKeys([]))return deny(command);
 if(command==='plugin:event|listen'){
  if(!exactKeys(['event','target','handler'])||args.event!=='skills-changed'||args.target?.kind!=='Any'||Object.keys(args.target).length!==1||!Number.isInteger(args.handler))return deny(command);
  listeners.set(args.handler,args.event);return args.handler;
 }
 if(command==='plugin:event|unlisten'){
  if(!exactKeys(['event','eventId'])||args.event!=='skills-changed'||!listeners.has(args.eventId))return deny(command);
  listeners.delete(args.eventId);return null;
 }
 switch(command){
  case 'skills_get_default_view_mode':return 'grouped';
  case 'skills_get_group_mode':
   readCount++;
   if((scenario==='delayed-read'||scenario==='unmount-read')&&readCount===1)return new Promise(resolve=>{initialResolve=()=>resolve('source');});
   return stored;
  case 'skills_set_group_mode':
   if(!exactKeys(['mode'])||!['custom','source'].includes(args.mode))return deny(command);
   writes.push(args.mode);
   if(scenario==='deferred-save')return new Promise((resolve,reject)=>{pendingWrites.set(writes.length,{mode:args.mode,resolve,reject});});
   if(scenario==='save-failure'){writeOutcomes.push({id:writes.length,mode:args.mode,ok:false});throw Error('Synthetic preference save failure');}
   writeOutcomes.push({id:writes.length,mode:args.mode,ok:true});stored=args.mode;return null;
  case 'skills_get_tool_status':return {tools:[],installed:[],newly_installed:[]};
  case 'skills_get_managed_skills':return [structuredClone(skill)];
  case 'skills_get_groups':return structuredClone(groups);
  case 'skills_get_central_repo_path':return '/synthetic/skills';
  case 'skills_get_preferred_tools':return [];
  case 'skills_get_limit_add_more_to_preferred_tools':return false;
  default:return deny(command);
 }
});
export const boundary={snapshot:()=>({scenario,caseToken,stored,calls:[...calls],writes:[...writes],forbidden,readCount,listeners:listeners.size,pendingInitial:!!initialResolve,pendingWrites:[...pendingWrites.keys()],writeOutcomes:structuredClone(writeOutcomes)}),settleWrite(id,ok){const item=pendingWrites.get(id);if(!item)throw Error('Unknown pending write');pendingWrites.delete(id);writeOutcomes.push({id,mode:item.mode,ok});if(ok){stored=item.mode;item.resolve(null);}else item.reject(Error('Synthetic deferred save failure'));},resolveInitial(){if(!initialResolve)throw Error('No pending initial read');const resolve=initialResolve;initialResolve=undefined;resolve();}};
