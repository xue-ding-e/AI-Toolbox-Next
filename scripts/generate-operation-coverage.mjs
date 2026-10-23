import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { SOURCE_PATH, serializeInventory } from './lib/operation-inventory.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'scripts/generated/operation-coverage.json');
const args = process.argv.slice(2);
if (args.length > 1 || (args.length === 1 && !['--check', '--require-complete'].includes(args[0]))) {
  console.error('Usage: node scripts/generate-operation-coverage.mjs [--check|--require-complete]');
  process.exitCode = 1;
} else {
  try {
    const serialized = serializeInventory(await readFile(path.join(root, SOURCE_PATH), 'utf8'));
    if (args[0] === '--require-complete') {
      // Deliberately not a release/completeness gate yet: inventory is not implementation.
      console.error('INCOMPLETE: CLI/MCP business coverage is not implemented; only the handler inventory is available.');
      process.exitCode = 2;
    } else if (args[0] === '--check') {
      if (await readFile(output, 'utf8') !== serialized) throw new Error('Operation inventory is stale; regenerate and review source changes.');
      console.log('Handler inventory is current. Full business coverage remains incomplete.');
    } else {
      await mkdir(path.dirname(output), { recursive: true });
      await writeFile(output, serialized);
      console.log('Generated handler inventory. Full business coverage remains incomplete.');
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
