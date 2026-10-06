import { parse as parseToml } from 'smol-toml';
import { applyKimiSecondaryModelToml } from './web/features/coding/kimi/utils/secondaryModelForm.ts';

const a = `default_model = "a/b"\n\n[secondary_model] # swarm pool\ndefault_model = "old/alias"\n\n[loop_control]\nmax_attempts_per_step = 10\n`;
const out = applyKimiSecondaryModelToml(a, { defaultModel: 'new/x', models: [], force: false });
console.log('--- OUTPUT ---');
console.log(out);
try {
  const parsed = parseToml(out);
  console.log('--- PARSED OK ---', JSON.stringify(parsed));
} catch (e) {
  console.log('--- PARSE FAILED ---', String(e));
}

// Also: what does the OLD value look like after a save? Does the stale value survive?
console.log('=== Check whether the new value is the one the CLI sees ===');
