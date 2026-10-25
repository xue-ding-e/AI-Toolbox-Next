// One bounded supervised process, two distinct real-component fixtures. A
// failed editor batch must not silently omit the existing catalog controls.
const failures = [];
for (const specifier of ['./verify-grok-editor-readback.mjs', './verify-grok-fetched-models.mjs']) {
  try { await import(specifier); }
  catch (error) { failures.push(error); }
}
if (failures.length) throw new AggregateError(failures, 'Grok editor/catalog fixture batch failed');
