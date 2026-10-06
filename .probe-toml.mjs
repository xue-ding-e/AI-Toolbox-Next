import {
  applyKimiSecondaryModelToml,
  parseKimiSecondaryModelConfig,
} from './web/features/coding/kimi/utils/secondaryModelForm.ts';

// Case A: inline comment on the section header
const a = `default_model = "a/b"\n\n[secondary_model] # swarm pool\ndefault_model = "old/alias"\n\n[loop_control]\nmax_attempts_per_step = 10\n`;
console.log('--- CASE A (inline comment on header) ---');
console.log(JSON.stringify(applyKimiSecondaryModelToml(a, { defaultModel: 'new/x', models: [], force: false })));

// Case B: quoted header
const b = `[secondary_model]\ndefault_model = "old/alias"\n\n[loop_control]\nmax_attempts_per_step = 10\n`;
console.log('--- CASE B (plain header) ---');
console.log(JSON.stringify(applyKimiSecondaryModelToml(b, { defaultModel: 'new/x', models: [], force: false })));

// Case C: array-of-tables sub-table
const c = `[secondary_model]\ndefault_model = "old/alias"\n\n[[secondary_model.models]]\nkey = "x"\n\n[loop_control]\nx = 1\n`;
console.log('--- CASE C (array-of-tables) ---');
console.log(JSON.stringify(applyKimiSecondaryModelToml(c, { defaultModel: 'new/x', models: [], force: false })));

// Case D: parse an array-of-tables models key (what the CLI schema allows?)
console.log('--- CASE D (parse array-of-tables) ---');
console.log(JSON.stringify(parseKimiSecondaryModelConfig(c)));
