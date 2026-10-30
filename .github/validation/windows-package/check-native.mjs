import { readFileSync } from 'node:fs';
import { validateNativeResult } from './package.mjs';
const [log, exit, filter, count] = process.argv.slice(2);
validateNativeResult(readFileSync(log, 'utf8'), Number(exit), filter, Number(count));
