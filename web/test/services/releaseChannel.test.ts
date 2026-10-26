import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { NEXT_GITHUB_REPO, NEXT_RELEASES_URL, UPSTREAM_GITHUB_REPO } from '../../services/releaseChannel.ts';

test('Next manual downloads include prereleases and never point upstream', () => {
  assert.equal(NEXT_GITHUB_REPO, 'xue-ding-e/AI-Toolbox-Next');
  assert.equal(NEXT_RELEASES_URL, 'https://github.com/xue-ding-e/AI-Toolbox-Next/releases');
  assert.notEqual(NEXT_GITHUB_REPO, UPSTREAM_GITHUB_REPO);
  const service = readFileSync(new URL('../../services/appApi.ts', import.meta.url), 'utf8');
  assert.match(service, /LATEST_RELEASE_URL = NEXT_RELEASES_URL/);
  assert.doesNotMatch(service, /releases\/latest|coulsontl\/ai-toolbox/);
});

test('Next installer identity and default channel do not reuse upstream', () => {
  const config = JSON.parse(readFileSync(new URL('../../../tauri/tauri.conf.json', import.meta.url), 'utf8'));
  assert.equal(config.productName, 'AI Toolbox Next');
  assert.equal(config.identifier, 'com.xue-ding-e.ai-toolbox-next');
  assert.deepEqual(config.plugins['deep-link'].desktop.schemes, ['aitoolbox-next']);
  assert.equal(config.bundle.createUpdaterArtifacts, false);
  assert.equal(config.plugins.updater, undefined);
  const backend = readFileSync(new URL('../../../tauri/src/update.rs', import.meta.url), 'utf8');
  assert.match(backend, /AI Toolbox Next uses manual updates/);
  assert.doesNotMatch(backend, /app\.updater\(|download_and_install\(/);
});
