import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';import {createRequire}from'node:module';
export function extractPage({root,fixtureDirectory,variant,dependenciesRoot,baselineCommit}){
 const req=createRequire(path.join(dependenciesRoot,'_fixture.cjs')),ts=req('typescript');
 const files={page:'web/features/coding/claudedesktop/pages/ClaudeDesktopPage.tsx',form:'web/features/coding/claudedesktop/components/ClaudeDesktopProviderFormModal.tsx'};
 const source=Object.fromEntries(Object.entries(files).map(([k,f])=>[k,ts.createSourceFile(f,fs.readFileSync(path.join(root,f),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX)]));
 const spans=[];
 function find(k,p){let match;function visit(n){if(p(n)){if(match)throw new Error('Nonunique extraction');match=n;}ts.forEachChild(n,visit);}visit(source[k]);if(!match)throw new Error('Missing extraction');return match;}
 const variable=(k,name)=>find(k,n=>ts.isVariableDeclaration(n)&&(n.name.getText(source[k])===name||(ts.isArrayBindingPattern(n.name)&&n.name.elements.some(e=>e.name?.getText(source[k])===name))));
 const text=(k,n,label)=>{const s=source[k],v=n.getText(s);spans.push({file:files[k],label,startLine:s.getLineAndCharacterOfPosition(n.getStart(s)).line+1,endLine:s.getLineAndCharacterOfPosition(n.end).line+1,sha256:crypto.createHash('sha256').update(v).digest('hex')});return v;};
 const builders=['buildProviderSettingsConfig','normalizeTierAlias','buildClaudeDesktopModelRoutes','buildProviderMeta'].map(name=>text('page',find('page',n=>ts.isFunctionDeclaration(n)&&n.name?.text===name),name));
 const constants=['CLAUDE_DESKTOP_ROLE_ROUTE_IDS','CLAUDE_DESKTOP_ROLE_ROUTE_ORDER','TIER_ALIAS_VALUES'].map(name=>'const '+text('page',variable('page',name),name)+';');
 const submit=text('page',variable('page','handleProviderSubmit').initializer,'handleProviderSubmit');
 const importer=text('page',variable('page','handleImportFromCcSwitch').initializer.arguments[0],'handleImportFromCcSwitch');
 text('form',variable('form','handleSubmit').initializer,'real form submit (manifest only; not projected)');
 const effect=find('form',n=>ts.isCallExpression(n)&&n.expression.getText(source.form)==='React.useEffect'&&n.arguments[0]?.getText(source.form).includes('const nextProvider'));
 text('form',effect.arguments[0],'real form initializer (manifest only; not projected)');
 if(variant==='green'){
  for(const name of ['providerMode','categoryOverride','credentialRules'])text('form',variable('form',name),name);
  if(!source.form.text.includes('restoreOfficial: isOfficialMode')||source.form.text.includes('category: providerCategory'))throw new Error('GREEN form is stale');
 }
 const modulePaths=[...Object.values(files),'web/types/claudedesktop.ts','web/features/coding/claudecode/utils/claudeModelConfig.ts','web/features/coding/shared/providerHeaders/customHeadersUtils.ts','web/features/coding/shared/providerModelRewrites/modelRewritesUtils.ts','web/features/coding/shared/gateway/providerProfiles.ts','web/features/coding/shared/gateway/gatewayAggregateConfig.ts','web/features/coding/shared/ccSwitch/ImportFromCcSwitchModal.tsx','web/services/ccSwitchApi.ts','web/features/coding/shared/providerConfig/ProviderNotesCollapse.tsx','web/features/coding/shared/providerConfig/ProviderConfigCollapse.tsx','web/features/coding/shared/gateway/useGatewaySupportedCliKeys.ts','tauri/resources/gateway_provider_profiles.json','scripts/lib/browser-fixture.mjs','package.json','pnpm-lock.yaml'];
 if(variant==='green')modulePaths.push('web/features/coding/claudedesktop/utils/officialRestore.ts');
 const identity={variant,baselineCommit,scope:'Real React form/import modal, exact page callbacks and payload builders; not full Page/Rust/SQLite',spans,modules:modulePaths.map(file=>({file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(root,file))).digest('hex')}))};
 const generated=`
import {stripClaudeOneMMarker,hasClaudeOneMMarker}from'@/features/coding/claudecode/utils/claudeModelConfig';
import {CUSTOM_PROVIDER_PROFILE_ID,toGatewayProviderProfileReference}from'@/features/coding/shared/gateway/providerProfiles';
import {mergeCustomHeadersIntoMeta}from'@/features/coding/shared/providerHeaders/customHeadersUtils';
import {mergeModelRewritesIntoMeta}from'@/features/coding/shared/providerModelRewrites/modelRewritesUtils';
import {resolveGatewayReengageMode,toGatewayAggregateReengageConfig}from'@/features/coding/shared/gateway/gatewayAggregateConfig';
${variant==='green'?"import {isClaudeDesktopOfficialRestoreProvider,canPreserveImportedClaudeDesktopCredentials}from'@/features/coding/claudedesktop/utils/officialRestore';":''}
export const sourceIdentity=${JSON.stringify(identity)};
${constants.join('\n')}
${builders.join('\n')}
const clone=x=>structuredClone(x),noop=()=>{};
export function createPageBoundary(initialProviders=[],isCopyMode=false){
 let providers=clone(initialProviders),editingProvider=null;const gatewayCliStatus=null,creates=[],writes=[],errors=[],favorites=[];
 const message={success:noop,warning:noop,info:noop,error:x=>errors.push(String(x))},t=x=>x;
 const createClaudeDesktopProvider=async input=>{creates.push(clone(input));const p={...clone(input),id:'synthetic-import-'+creates.length,isApplied:false,isDisabled:false,createdAt:'2026-01-01T00:00:00Z',updatedAt:'2026-01-01T00:00:00Z'};providers.push(p);return clone(p);};
 const updateClaudeDesktopProvider=async p=>{if(p.isApplied||p.id==='claude-desktop-official')throw new Error('Applied/seed forbidden');writes.push(clone(p));providers=providers.map(x=>x.id===p.id?clone(p):x);return clone(p);};
 const deny=()=>{throw new Error('Gateway/runtime operation forbidden');};
 const restoreProxyGatewayCliDirect=deny,engageProxyGatewaySingle=deny,engageProxyGatewayFailover=deny,setGatewayCliStatus=deny;
 const saveProviderWithGatewayReengage=async o=>{if(o.gatewayMode!==null||o.aggregateConfig!==null)throw new Error('Non-direct fixture state');return o.saveProvider();};
 const setProviderModalOpen=noop,setIsCopyMode=noop,setCcSwitchImportModalOpen=noop,setEditingProvider=x=>{editingProvider=x;};
 const upsertDesktopFavoriteProvider=async p=>favorites.push(clone(p)),loadConfig=async()=>{},refreshTrayMenu=async()=>{};
 const handleProviderSubmit=${submit};
 const handleImportFromCcSwitch=${importer};
 return{importCandidates:handleImportFromCcSwitch,submit:handleProviderSubmit,edit:id=>{editingProvider=clone(providers.find(x=>x.id===id));if(!editingProvider)throw new Error('Unknown fixture record');return clone(editingProvider);},snapshot:()=>clone({providers,creates,writes,errors,favorites})};
}`;
 const result=ts.transpileModule(generated,{fileName:'page-boundary.generated.ts',reportDiagnostics:true,compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}});
 if(result.diagnostics.some(x=>x.category===ts.DiagnosticCategory.Error))throw new Error('Generated page boundary has syntax errors');
 fs.writeFileSync(path.join(fixtureDirectory,'page-boundary.generated.mjs'),result.outputText);
 fs.writeFileSync(path.join(fixtureDirectory,'source-identity.json'),JSON.stringify(identity,null,2)+'\n');
 return identity;
}
