import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { resolveMemoriesSourceMode } from '../../../../../../features/coding/codex/components/CodexMemoriesPanel/memoriesSource.ts';

test('WSL-only fallback remains selected after a list error', () => {
  const resolvedMode = resolveMemoriesSourceMode('local', [{ source: 'wsl' }]);
  assert.equal(resolvedMode, 'wsl');
  assert.equal(resolveMemoriesSourceMode(resolvedMode, undefined), 'wsl');
  assert.equal(resolveMemoriesSourceMode(resolvedMode, [{ source: 'wsl' }]), 'wsl');
});

test('unknown source availability does not force a source change', () => {
  assert.equal(resolveMemoriesSourceMode('local', undefined), 'local');
  assert.equal(resolveMemoriesSourceMode('local', []), 'local');
  assert.equal(resolveMemoriesSourceMode('wsl', []), 'wsl');
});

test('a local source and an explicit WSL selection are preserved', () => {
  assert.equal(resolveMemoriesSourceMode('local', [{ source: 'local' }, { source: 'wsl' }]), 'local');
  assert.equal(resolveMemoriesSourceMode('wsl', [{ source: 'local' }]), 'wsl');
});

test('panel persists an automatic fallback and keeps an unchanged selection loading', async () => {
  const panel = await readFile(new URL('../../../../../../features/coding/codex/components/CodexMemoriesPanel/CodexMemoriesPanel.tsx', import.meta.url), 'utf8');
  assert.match(panel, /rememberedMemoriesSourceMode = resolvedMode;\s+setSourceMode\(resolvedMode\);\s+setCurrentDir\(''\);/);
  const sourceChange = panel.slice(panel.indexOf('const handleSourceModeChange ='), panel.indexOf('const handleEntryClick ='));
  assert.match(sourceChange, /if \(mode === sourceMode && currentDir === ''\) \{\s+return;\s+\}\s+listRequestIdRef.current \+= 1;/);
});

test('panel rejects old navigation refreshes and guards saved file content', async () => {
  const panel = await readFile(new URL('../../../../../../features/coding/codex/components/CodexMemoriesPanel/CodexMemoriesPanel.tsx', import.meta.url), 'utf8');
  const listLoader = panel.slice(panel.indexOf('const loadList ='), panel.indexOf('loadList(sourceMode, currentDir);'));
  assert.match(listLoader, /navigationContextRef.current.sourceMode === mode &&\s+navigationContextRef.current.currentDir === dir/);
  assert.match(listLoader, /if \(!isCurrentList\(\)\) \{\s+return;\s+\}\s+const requestId =/);
  assert.match(listLoader, /requestId !== listRequestIdRef.current \|\| !isCurrentList\(\)/);
  const saveHandler = panel.slice(panel.indexOf('const handleSave ='), panel.indexOf('const handleCreateFile ='));
  assert.match(saveHandler, /currentSelection.sourceMode === sourceMode &&\s+currentSelection.selectedFilePath === selectedFilePath/);
  assert.match(saveHandler, /\) \{\s+setEditing\(false\);\s+setFileContent\(editValue\);\s+\}/);
});
