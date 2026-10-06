/**
 * Guards the layout of the coding provider / model form modals.
 *
 * Every one of these modals lays its fields out horizontally, with labels in a
 * left column (`layout="horizontal"` + `labelCol` / `wrapperCol`). That is the
 * shared shape the SOP calls for, and it is what makes a new tab look like its
 * siblings instead of like a different product.
 *
 * The failure this catches is quiet: `layout="vertical"` still compiles, still
 * renders, and still works — it just puts labels above the inputs. During the
 * ZCode migration the same mistake was made twice (the provider form, then the
 * model form), each time discovered only by looking at the screen.
 *
 * Scope is deliberately narrow: only the files named `*ProviderFormModal.tsx`
 * and `*ModelFormModal.tsx`. Elsewhere in the app — single-field config modals,
 * settings forms — `layout="vertical"` is a legitimate choice.
 */
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = fileURLToPath(new URL('../web', import.meta.url));
const codingRoot = path.join(webRoot, 'features', 'coding');

/** Directory names skipped while walking; none currently hold form modals. */
const SKIPPED_DIRECTORIES = new Set(['node_modules', '__tests__', 'fixtures']);

const isGovernedFile = (filename) =>
  filename.endsWith('ProviderFormModal.tsx') || filename.endsWith('ModelFormModal.tsx');

async function collectGovernedFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const found = [];

  for (const entry of entries) {
    if (entry.isDirectory()) {
      if (SKIPPED_DIRECTORIES.has(entry.name)) {
        continue;
      }
      found.push(...(await collectGovernedFiles(path.join(directory, entry.name))));
      continue;
    }
    if (isGovernedFile(entry.name)) {
      found.push(path.join(directory, entry.name));
    }
  }

  return found;
}

const files = (await collectGovernedFiles(codingRoot)).sort();
const violations = [];

for (const file of files) {
  const source = await readFile(file, 'utf8');
  const lines = source.split('\n');

  lines.forEach((line, index) => {
    if (line.includes('layout="vertical"')) {
      violations.push({
        file: path.relative(webRoot, file),
        line: index + 1,
        text: line.trim(),
      });
    }
  });
}

if (files.length === 0) {
  console.error('No provider/model form modals found — the scan path is wrong.');
  process.exit(1);
}

if (violations.length > 0) {
  console.error('Form modals must use layout="horizontal" (labels in a left column).\n');
  for (const violation of violations) {
    console.error(`  ${violation.file}:${violation.line}`);
    console.error(`    ${violation.text}\n`);
  }
  console.error(
    `Fix: use <Form layout="horizontal" labelCol={labelCol} wrapperCol={wrapperCol}>, picking\n` +
      'the label span from the longest label in that form. See docs/new-cli-onboarding-sop.md §4.0.2-A.',
  );
  process.exit(1);
}

console.log(`Form modal layout check passed (${files.length} files).`);
