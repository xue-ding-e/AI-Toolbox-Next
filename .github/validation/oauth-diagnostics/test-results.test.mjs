import {test} from 'node:test'
import assert from 'node:assert/strict'
import {verifyTestOutput} from './test-results.mjs'
const names=['module::one','module::two']
const good='running 2 tests\ntest module::one ... ok\ntest module::two ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 500 filtered out; finished in 0.01s\n'
test('exact native success includes all named tests and one final summary',()=>{assert.equal(verifyTestOutput(good,names,0).passed,2)})
test('nonzero, missing, duplicate, ignored, failed, foreign and misleading summary fail closed',()=>{
 assert.throws(()=>verifyTestOutput(good,names,1))
 for(const text of [good.replace('test module::two ... ok\n',''),good.replace('module::two','module::one'),good.replace('module::two ... ok','module::two ... ignored'),good.replace('module::two ... ok','module::two ... FAILED'),good.replace('module::two','module::foreign'),good.replace('2 passed','1 passed'),good+good,good.replace('0 ignored','1 ignored')])assert.throws(()=>verifyTestOutput(text,names,0))
})
