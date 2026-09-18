import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MINI_BROWSER_MAX_URL_LENGTH,
  normaliseMiniBrowserUrl,
} from '../../services/miniBrowserApi';

// ---- address normalisation -------------------------------------------------

test('mini browser adds https to a bare relay host', () => {
  assert.equal(normaliseMiniBrowserUrl('relay.example.com'), 'https://relay.example.com/');
  assert.equal(
    normaliseMiniBrowserUrl('  relay.example.com/console  '),
    'https://relay.example.com/console',
  );
  // A local dashboard keeps its explicit scheme and port.
  assert.equal(
    normaliseMiniBrowserUrl('http://127.0.0.1:3000/balance'),
    'http://127.0.0.1:3000/balance',
  );
});

test('mini browser keeps an explicit scheme and query', () => {
  assert.equal(
    normaliseMiniBrowserUrl('https://api.example.com/usage?tab=credits'),
    'https://api.example.com/usage?tab=credits',
  );
});

test('mini browser refuses schemes that would escape the web sandbox', () => {
  // These are the strings an <iframe> or a naive shell-open would execute.
  assert.equal(normaliseMiniBrowserUrl('javascript:alert(1)'), null);
  assert.equal(normaliseMiniBrowserUrl('file:///C:/Windows/win.ini'), null);
  assert.equal(normaliseMiniBrowserUrl('data:text/html,<h1>x</h1>'), null);
  assert.equal(normaliseMiniBrowserUrl('ms-settings:privacy'), null);
});

test('mini browser refuses empty, overlong and control-character input', () => {
  assert.equal(normaliseMiniBrowserUrl(''), null);
  assert.equal(normaliseMiniBrowserUrl('   '), null);
  assert.equal(normaliseMiniBrowserUrl('https://ok.example.com/\nheader'), null);
  assert.equal(
    normaliseMiniBrowserUrl(`https://example.com/${'a'.repeat(MINI_BROWSER_MAX_URL_LENGTH)}`),
    null,
  );
});
