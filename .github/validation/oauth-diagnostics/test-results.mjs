import assert from 'node:assert/strict'
import {readFileSync,writeFileSync,statSync} from 'node:fs'
import {pathToFileURL} from 'node:url'
import {resolve} from 'node:path'

export function verifyTestOutput(text, expected, exitCode) {
  assert.equal(exitCode, 0, 'Cargo or native test process failed')
  assert(!text.includes('\x1b'), 'Unexpected ANSI output')
  const results = [...text.matchAll(/^test ([a-zA-Z0-9_:]+) \.\.\. (\S+)\r?$/gm)].map(m => ({name:m[1],status:m[2]}))
  assert.equal(results.length, expected.length, 'Unexpected or missing native test result')
  assert.deepEqual(results.map(x=>x.name).sort(), [...expected].sort())
  assert(results.every(x=>x.status==='ok'), 'Skipped or failed tests are not acceptance')
  const summaries=[...text.matchAll(/^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in [^\r\n]+\r?$/gm)]
  assert.equal(summaries.length,1,'Exactly one native --lib summary required')
  assert.equal(Number(summaries[0][1]),expected.length)
  for(const i of [2,3,4]) assert.equal(Number(summaries[0][i]),0)
  return {passed:expected.length,failed:0,ignored:0,filteredOut:Number(summaries[0][5]),names:expected}
}
if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const [manifestPath,label,logPath,exit,output]=process.argv.slice(2)
  const manifest=JSON.parse(readFileSync(manifestPath,'utf8'))
  assert(statSync(logPath).size<20*1024*1024,'Test output budget exceeded')
  const result=verifyTestOutput(readFileSync(logPath,'utf8').replace(/^\uFEFF/,''),manifest.tests[label],Number(exit))
  writeFileSync(output,JSON.stringify(result,null,2)+'\n')
}
