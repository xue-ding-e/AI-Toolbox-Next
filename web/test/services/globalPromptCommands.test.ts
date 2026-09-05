import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const servicesDir = path.join(repoRoot, 'web', 'services');
const libRsPath = path.join(repoRoot, 'tauri', 'src', 'lib.rs');

test('every prompt api declares a disable command registered by the backend', async () => {
  const serviceFiles = (await readdir(servicesDir)).filter(
    (name) => name.endsWith('PromptApi.ts') && name !== 'globalPromptApi.ts',
  );

  assert.ok(
    serviceFiles.length >= 11,
    'expected every tool prompt api service file to exist',
  );

  const libRs = await readFile(libRsPath, 'utf-8');
  const problems: string[] = [];

  for (const filename of serviceFiles) {
    const content = await readFile(path.join(servicesDir, filename), 'utf-8');
    const disableMatch = content.match(/disable: '([a-z_]+)'/);
    if (!disableMatch) {
      problems.push(`${filename}: missing disable command name`);
      continue;
    }
    const commandName = disableMatch[1];
    if (!libRs.includes(`${commandName},`)) {
      problems.push(`${filename}: command '${commandName}' is not registered in tauri/src/lib.rs`);
    }
  }

  assert.deepEqual(problems, []);
});
