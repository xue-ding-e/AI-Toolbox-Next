import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import {execFileSync}from'node:child_process';
import {createRequire}from'node:module';
import {parseArgs}from'node:util';
import {fileURLToPath,pathToFileURL}from'node:url';
import {extractPage}from'./extract-page.mjs';
import {makeVerifier}from'./verify.mjs';
import {counts}from'./scenarios.mjs';
const BASELINE='f3a84498b5e192f1e78cb2a5312be012095de76c';
const {values}=parseArgs({options:{'prepare-only':{type:'boolean'},'red-green':{type:'boolean'},browser:{type:'string'},output:{type:'string'}}});
const own=path.dirname(fileURLToPath(import.meta.url));
const sourceRoot=process.env.AI_OFFICIAL_EDIT_SOURCE_ROOT||path.resolve(own,'../../../../../..');
const overlayRoot=process.env.AI_OFFICIAL_EDIT_OVERLAY_ROOT;
const deps=process.env.AI_OFFICIAL_EDIT_DEPENDENCIES_ROOT||path.join(sourceRoot,'node_modules');
const variants=values['red-green']?['red','green']:['green'];
fs.mkdirSync(values.output||os.tmpdir(),{recursive:true});
const output=fs.mkdtempSync(path.join(values.output||os.tmpdir(),'official-edit-review-'));
const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const harnessFiles=['run.mjs','extract-page.mjs','verify.mjs','scenarios.mjs','transport.mjs','Fixture.jsx'].map(file=>({file,sha256:sha(path.join(own,file))}));
const packageJson=JSON.parse(fs.readFileSync(path.join(sourceRoot,'package.json'),'utf8'));
const needed=[...Object.keys(packageJson.dependencies),'typescript','vite','less'];
const missing=needed.filter(name=>!fs.existsSync(path.join(deps,name,'package.json')));
const versions=Object.fromEntries(needed.filter(name=>!missing.includes(name)).map(name=>[name,JSON.parse(fs.readFileSync(path.join(deps,name,'package.json'),'utf8')).version]));
// Resolve transitive packages from their actual parents, including pnpm layouts.
// Do not require indirect dependencies to be hoisted to root node_modules.
const validationChain=[];
let validationChainError=null;
try{
 let parent=path.join(deps,'_fixture.cjs');
 for(const[name,expected]of [['antd','6.4.3'],['@rc-component/form','1.8.6'],['@rc-component/async-validator','6.0.0']]){
  const req=createRequire(parent);
  const packagePath=req.resolve(name+'/package.json');
  const actual=JSON.parse(fs.readFileSync(packagePath,'utf8')).version;
  validationChain.push({name,expected,actual,packagePath});
  if(actual!==expected)throw new Error(name+' validation dependency drift: expected '+expected+', got '+actual);
  parent=packagePath;
 }
}catch(error){validationChainError=String(error);}
const preflight={status:missing.length||validationChainError?'environment-incomplete':'dependencies-present',missing,versions,validationChain,validationChainError,counts,variants,baseline:values['red-green']?BASELINE:null,sourceRoot,overlayRoot:overlayRoot||null,dependenciesRoot:deps,prepareOnly:Boolean(values['prepare-only']),noNetwork:true};
fs.writeFileSync(path.join(output,'preflight.json'),JSON.stringify(preflight,null,2)+'\n');
console.log('Review artifacts: '+output);
if(missing.includes('typescript'))throw new Error('Environment not executed: TypeScript unavailable for exact source extraction.');
if(missing.length&&!values['prepare-only'])throw new Error('Environment not executed: missing dependencies '+missing.join(', '));
if(validationChainError&&!values['prepare-only'])throw new Error('Environment not executed: '+validationChainError);
const expectedFiles=[
 'web/features/coding/claudedesktop/components/ClaudeDesktopProviderFormModal.tsx',
 'web/features/coding/claudedesktop/pages/ClaudeDesktopPage.tsx',
 'web/features/coding/claudedesktop/utils/officialRestore.ts',
 'web/types/claudedesktop.ts',
 'web/test/features/coding/claudedesktop/utils/officialRestore.test.ts',
 'web/features/coding/claudedesktop/AGENTS.md',
 'package.json',
 ...harnessFiles.map(({file})=>'web/test/features/coding/claudedesktop/officialEdit/'+file),
];
const staged=[];
// Only explicitly requested isolated comparison reads the historical object.
// Ordinary regression has no dependency on Git or the old baseline commit.
if(values['red-green'])execFileSync('git',['-C',sourceRoot,'cat-file','-e',BASELINE+'^{commit}']);
for(const variant of variants){
 const root=path.join(output,variant);fs.mkdirSync(root);
 if(variant==='red'){
  const archive=execFileSync('git',['-C',sourceRoot,'archive','--format=tar',BASELINE,'web','tauri/resources','scripts/lib/browser-fixture.mjs','package.json','pnpm-lock.yaml'],{maxBuffer:64*1024*1024});
  execFileSync('tar',['-xf','-','-C',root],{input:archive});
 }else{
  for(const name of ['web','tauri/resources','scripts/lib/browser-fixture.mjs','package.json','pnpm-lock.yaml']){
   fs.mkdirSync(path.dirname(path.join(root,name)),{recursive:true});fs.cpSync(path.join(sourceRoot,name),path.join(root,name),{recursive:true});
  }
 }
 if(variant==='green'&&overlayRoot){for(const file of expectedFiles){const from=path.join(overlayRoot,file);if(!fs.existsSync(from))throw new Error('Incomplete candidate overlay: '+file);fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});fs.copyFileSync(from,path.join(root,file));}}
 fs.symlinkSync(deps,path.join(root,'node_modules'),'dir');
 const fixtureDirectory=path.join(root,'official-edit-fixture');fs.mkdirSync(fixtureDirectory);
 for(const file of ['scenarios.mjs','transport.mjs','Fixture.jsx'])fs.copyFileSync(path.join(own,file),path.join(fixtureDirectory,file));
 const identity=extractPage({root,fixtureDirectory,variant,dependenciesRoot:deps,baselineCommit:variant==='red'?BASELINE:null});
 const module=pathToFileURL(path.join(root,'scripts/lib/browser-fixture.mjs')).href;
 const files=expectedFiles.filter(file=>fs.existsSync(path.join(root,file))).map(file=>({file,sha256:sha(path.join(root,file))}));
 staged.push({variant,root,fixtureDirectory,identity,module,files,generatedBoundarySha256:sha(path.join(fixtureDirectory,'page-boundary.generated.mjs'))});
}
if(values['red-green'])assertDistinct();
function assertDistinct(){
 const [red,green]=staged;
 for(const label of ['handleProviderSubmit','buildProviderSettingsConfig','buildProviderMeta','real form submit (manifest only; not projected)','real form initializer (manifest only; not projected)']){
  if(red.identity.spans.find(x=>x.label===label)?.sha256===green.identity.spans.find(x=>x.label===label)?.sha256)throw new Error('RED/GREEN source not distinct: '+label);
 }
}
fs.writeFileSync(path.join(output,'prepared-manifest.json'),JSON.stringify({counts,variants,harnessFiles,stages:staged,limits:{scenarioMs:30000,verifyPerVariantMs:480000,bundles:variants.length,browsers:variants.length},scope:'Real form and import modal; page callbacks/builders are freshly extracted per variant; no full Page, Rust, DB or real runtime configuration.'},null,2)+'\n');
if(values['prepare-only']){
 console.log(JSON.stringify({status:'prepared-not-executed',counts,missingDependencies:missing,manifest:path.join(output,'prepared-manifest.json')}));
}else{
 // The shared runner has its own small argument parser. Preserve only its flags.
 process.argv=[process.argv[0],process.argv[1],...(values.browser?['--browser',values.browser]:[]),'--output',output];
 const results=[];
 for(const stage of staged){
  const {runBrowserFixture}=await import(stage.module);
  await runBrowserFixture({fixtureDirectory:stage.fixtureDirectory,fixtureFilename:'Fixture.jsx',artifactPrefix:'official-edit-'+stage.variant+'-',fixtureAliases:[],verify:async context=>{
   const checks=await makeVerifier(stage.variant,stage.identity)(context);
   const bundles=[];function walk(directory){for(const entry of fs.readdirSync(directory,{withFileTypes:true})){const full=path.join(directory,entry.name);if(entry.isDirectory())walk(full);else bundles.push({file:path.relative(context.artifactRoot,full),sha256:sha(full)});}}walk(path.join(context.artifactRoot,'dist'));
   results.push({variant:stage.variant,count:checks.length,checks,bundles,artifactRoot:context.artifactRoot,sourceIdentity:stage.identity});return checks;
  }});
 }
 if(results.length!==variants.length||results.some(x=>x.count!==counts[x.variant]))throw new Error('Incomplete strict matrix');
 const status=values['red-green']?'strict-red-green-pass':'green-regression-pass';
 fs.writeFileSync(path.join(output,'results.json'),JSON.stringify({status,counts,variants,results},null,2)+'\n');
 console.log(JSON.stringify({status,counts,variants,results:path.join(output,'results.json')}));
}
