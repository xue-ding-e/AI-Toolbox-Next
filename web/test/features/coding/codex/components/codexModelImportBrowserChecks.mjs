import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function verifyCodexModelImport({ send, evaluate, baseUrl, artifactRoot }) {
  const checks = [];
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name); checks.push(name); console.log('PASS ' + name);
  };
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out: ' + expression);
  };
  const openFixture = async parameters => {
    const runId = randomUUID();
    await send('Page.navigate', { url: baseUrl + '/?' + parameters + '&runId=' + runId });
    await waitFor('window.modelImportFixture?.state.runId === ' + JSON.stringify(runId) + ' && !!document.querySelector(".ant-modal button")');
    await delay(200);
  };
  const action = expression => evaluate('modelImportFixture.' + expression);
  const openImport = async () => {
    await action('click("codex.provider.modelMappingImport")');
    await action('click("opencode.fetchModels.fetch")');
  };
  const save = async () => { await action('click("common.save")'); await waitFor('!!modelImportFixture.state.saved'); };
  const screenshot = async name => {
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(path.join(artifactRoot, name + '.png'), Buffer.from(data, 'base64'));
  };
  const expectedClaudeModel = {
    model: 'claude-opus-4-8', displayName: 'Claude Opus 4.8', contextWindow: 1000000,
    reasoningLevels: ['low', 'medium', 'high', 'xhigh', 'max'], defaultReasoningLevel: 'high',
  };

  await openFixture('scenario=preset');
  await openImport();
  await action('select("claude-opus-4-8")');
  await action('confirmImport()');
  await screenshot('preset-import');
  await save();
  check('import fills the bundled preset name, context and reasoning fields', await action('submittedSettings().modelCatalog.models'), [expectedClaudeModel]);
  await action('reopenSaved()');
  await save();
  check('preset fields survive saving and reopening the real provider form', await action('submittedSettings().modelCatalog.models'), [expectedClaudeModel]);
  check('import keeps the default request model independent', (await action('submittedSettings().config')).includes('model = "default-model"'));

  await openFixture('scenario=manual');
  await action('click("codex.provider.modelMapping")');
  await action('click("codex.provider.modelMappingAdd")');
  await action('editMapping(0, "claude-opus-4-8")');
  await save();
  check('manually entering the same model uses identical preset defaults', await action('submittedSettings().modelCatalog.models'), [expectedClaudeModel]);

  await openFixture('scenario=builtin');
  const initialModels = await action('mappingModels()');
  await openImport(); await action('select("model-alpha")'); await action('confirmImport()'); await save();
  check('built-in endpoints save imported rows instead of replacing them with defaults', (await action('submittedSettings().modelCatalog.models')).map(model => model.model), [...initialModels, 'model-alpha']);
  await action('reopenSaved()'); await action('removeAllMappings()'); await save();
  check('clearing a built-in endpoint catalog remains cleared after save', await action('submittedSettings().modelCatalog === undefined'));

  await openFixture('scenario=whitespace');
  await action('editMapping(0, " gpt-6-astra ")'); await openImport();
  check('whitespace-normalized existing model IDs remain disabled in the picker', await action('isModelDisabled("gpt-6-astra")'));
  await action('select("model-alpha")'); await action('confirmImport()'); await save();
  await action('reopenSaved()'); await save();
  check('custom context, reasoning, speed and image metadata survive import and reopen', (await action('submittedSettings().modelCatalog.models'))[0], {
    model: 'gpt-6-astra', displayName: 'My GPT', contextWindow: 64000, reasoningLevels: ['low'],
    defaultReasoningLevel: 'low', serviceTiers: ['priority'], supportsImage: false,
  });

  await openFixture('scenario=custom'); await openImport();
  await action('select("model-alpha")'); await action('search("model-beta")'); await action('select("model-beta")');
  await action('confirmImport()'); await save();
  check('selecting across different searches retains every selected model', (await action('submittedSettings().modelCatalog.models')).map(model => model.model), ['gpt-6-astra', 'model-alpha', 'model-beta']);

  await openFixture('scenario=custom');
  await action('setModels([{id:"model-alpha",name:"API Alpha"}])'); await openImport();
  await action('select("model-alpha")'); await action('confirmImport()'); await save();
  check('a fluctuating upstream list does not implicitly remove existing mappings', (await action('submittedSettings().modelCatalog.models')).map(model => model.model), ['model-alpha', 'gpt-6-astra']);
  await action('reopenSaved()'); await openImport(); await action('optIntoCleanup()'); await action('confirmImport()'); await save();
  check('explicit cleanup removes only the models missing from the fetched list', (await action('submittedSettings().modelCatalog.models')).map(model => model.model), ['model-alpha']);

  await openFixture('scenario=google');
  await action('click("codex.fetchModels.button")');
  check('the single-model fetch also uses the current native protocol', await action('state.requests.filter(item=>item.command==="fetch_provider_models").at(-1).args.request.apiType'), 'native');
  await openImport();
  let request = await action('state.requests.filter(item=>item.command==="fetch_provider_models").at(-1).args.request');
  check('Gemini import uses the native SDK, versioned path and query key', [request.sdkType, request.apiType, request.customUrl], ['@ai-sdk/google', 'native', 'https://models.example.test/v1beta/models?key=fixture-key']);
  await action('setDiscoveryMode("openai_compat")'); await action('cancelImport()'); await openImport();
  check('reopening a native provider resets the picker to its native discovery mode', await action('state.requests.filter(item=>item.command==="fetch_provider_models").at(-1).args.request.apiType'), 'native');
  await action('cancelImport()'); await action('switchProtocol("anthropic_messages")'); await openImport();
  request = await action('state.requests.filter(item=>item.command==="fetch_provider_models").at(-1).args.request');
  check('changing provider protocol updates the mounted picker SDK and mode', [request.sdkType, request.apiType], ['@ai-sdk/anthropic', 'native']);

  await openFixture('scenario=custom'); await action('deferNextFetch()'); await openImport();
  await action('cancelImport()'); await action('setModels([{id:"current-model"}])'); await openImport();
  await action('resolvePending([{id:"stale-model"}])');
  check('a response from a closed picker cannot replace the current model list', await action('listedModels()'), ['current-model']);

  await openFixture('scenario=custom'); await action('setModels([])'); await openImport();
  check('an empty model response cannot be applied without cleanup opt-in', await evaluate('Array.from(document.querySelectorAll(".ant-modal-footer .ant-btn-primary")).at(-1).disabled'));
  await action('cancelImport()'); await action('failNextFetch("Fixture discovery failed")'); await openImport();
  check('a failed discovery displays an error instead of reporting successful results', await evaluate('document.body.textContent.includes("Fixture discovery failed")'));

  for (const displayTheme of ['light', 'dark', 'system']) {
    await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
    await openFixture('scenario=preset&theme=' + displayTheme + '&language=' + (displayTheme === 'light' ? 'zh-CN' : 'en-US'));
    await openImport(); await action('select("claude-opus-4-8")'); await action('confirmImport()');
    await screenshot('preset-' + displayTheme);
    check(displayTheme + ' theme renders the imported mapping without page overflow', await evaluate('document.documentElement.scrollWidth <= innerWidth && modelImportFixture.mappingModels().includes("claude-opus-4-8")'));
  }
  return checks;
}
