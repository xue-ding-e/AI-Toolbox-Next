import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const readSource = (relativePath: string) =>
  readFileSync(new URL(relativePath, import.meta.url), 'utf8');

const modalSource = readSource('../../../../components/common/CodexConfigPreviewModal/index.tsx');
const typesSource = readSource('../../../../types/codex.ts');
const backendCommandsSource = readSource('../../../../../tauri/src/coding/codex/commands.rs');
const backendTypesSource = readSource('../../../../../tauri/src/coding/codex/types.rs');
const zhLocale = JSON.parse(readSource('../../../../i18n/locales/zh-CN.json')) as {
  codex: { preview: Record<string, string> };
};
const enLocale = JSON.parse(readSource('../../../../i18n/locales/en-US.json')) as {
  codex: { preview: Record<string, string> };
};

/**
 * The Codex config preview modal renders one tab per live file the backend
 * read. The model catalog tab must appear whenever the backend saw a catalog
 * signal (`modelCatalog` content, or `modelCatalogActive` for a dangling
 * pointer) and stay hidden only when neither exists — a leftover catalog file
 * without a `model_catalog_json` pointer is exactly the state users need to
 * diagnose, so it shows with an "inactive" hint instead of disappearing.
 */
test('the model catalog tab is conditional and read-only', () => {
  assert.match(
    modalSource,
    /const showModelCatalogTab = modelCatalogValue !== null \|\| modelCatalogActive === true;/,
  );
  assert.match(modalSource, /if \(showModelCatalogTab\) \{/);
  assert.match(
    modalSource,
    /key: 'modelCatalog',\s*\n\s*label: t\('codex\.preview\.modelCatalogJsonTitle'\)/,
  );
  // The catalog tab shows the raw file text, not a re-serialized object.
  const tabBlock = modalSource.slice(modalSource.indexOf("key: 'modelCatalog'"));
  const editorBlock = tabBlock.slice(0, tabBlock.indexOf('}));'));
  assert.match(editorBlock, /value=\{modelCatalogValue\}/);
  assert.match(editorBlock, /readOnly/);
  assert.match(editorBlock, /mode="text"/);
  // Inactive leftover content gets a hint that Codex is not reading it; a
  // dangling pointer gets an explicit missing-file empty state.
  assert.match(editorBlock, /modelCatalogActive === false/);
  assert.match(editorBlock, /t\('codex\.preview\.modelCatalogInactiveHint'\)/);
  assert.match(editorBlock, /t\('codex\.preview\.modelCatalogMissing'\)/);
});

test('the modelCatalog locale keys exist in every locale', () => {
  assert.equal(zhLocale.codex.preview.modelCatalogJsonTitle, 'model_catalog_json');
  assert.equal(enLocale.codex.preview.modelCatalogJsonTitle, 'model_catalog_json');
  assert.match(zhLocale.codex.preview.modelCatalogInactiveHint, /model_catalog_json/);
  assert.match(enLocale.codex.preview.modelCatalogInactiveHint, /model_catalog_json/);
  assert.match(zhLocale.codex.preview.modelCatalogMissing, /model_catalog_json/);
  assert.match(enLocale.codex.preview.modelCatalogMissing, /model_catalog_json/);
});

test('the frontend type mirrors the backend CodexSettings model_catalog fields', () => {
  assert.match(typesSource, /modelCatalog\?: string;/);
  assert.match(typesSource, /modelCatalogActive\?: boolean;/);
  assert.match(backendTypesSource, /pub model_catalog: Option<String>,/);
  assert.match(backendTypesSource, /pub model_catalog_active: Option<bool>,/);
  // Wire contract: CodexSettings must serialize camelCase, otherwise the
  // frontend `modelCatalog` read is silently undefined and the tab never
  // renders (auth/config are single words, so only model_catalog exposes
  // a snake_case/camelCase mismatch).
  assert.match(
    backendTypesSource,
    /#\[serde\(rename_all = "camelCase"\)\]\s*pub struct CodexSettings \{/,
  );
  // The backend read path fills the fields by following the config.toml
  // pointer with a leftover-file fallback.
  assert.match(
    backendCommandsSource,
    /model_catalog: catalog_preview\.content,/,
  );
  assert.match(
    backendCommandsSource,
    /model_catalog_active,/,
  );
});
