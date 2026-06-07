import assert from 'node:assert/strict';
import test from 'node:test';
import type { ComponentType } from 'react';

import {
  getRouteChrome,
  getRouteScrollKey,
  matchRouteEntry,
  normalizeRouteChrome,
  shouldShowRouteAppHeader,
} from '../../app/routeMatching.ts';
import type { RouteEntry } from '../../app/routeConfig.ts';

const TestComponent = (() => null) as ComponentType;

test('matchRouteEntry returns the longest route match', () => {
  const routes: RouteEntry[] = [
    { path: '/coding/opencode', component: TestComponent },
    {
      path: '/coding/opencode/sessions/detail',
      component: TestComponent,
      chrome: { mode: 'secondary', contentPadding: 'compact' },
    },
  ];

  const matched = matchRouteEntry(routes, '/coding/opencode/sessions/detail/extra');

  assert.equal(matched?.path, '/coding/opencode/sessions/detail');
});

test('matchRouteEntry does not match partial sibling prefixes', () => {
  const routes: RouteEntry[] = [
    { path: '/settings', component: TestComponent },
  ];

  assert.equal(matchRouteEntry(routes, '/settings-panel'), undefined);
});

test('route chrome defaults to the standard app chrome', () => {
  assert.deepEqual(getRouteChrome(undefined), {
    mode: 'default',
    contentPadding: 'default',
  });
});

test('route chrome preserves secondary page metadata', () => {
  const matched = normalizeRouteChrome({
    mode: 'secondary',
    contentPadding: 'compact',
    ownerTabKey: 'codex',
    parentPath: '/coding/codex',
  });

  assert.deepEqual(matched, {
    mode: 'secondary',
    contentPadding: 'compact',
    ownerTabKey: 'codex',
    parentPath: '/coding/codex',
  });
});

test('secondary route chrome hides the app header', () => {
  assert.equal(shouldShowRouteAppHeader(normalizeRouteChrome(undefined)), true);
  assert.equal(
    shouldShowRouteAppHeader(normalizeRouteChrome({ mode: 'secondary' })),
    false,
  );
});

test('route scroll key keeps tab pages stable and isolates secondary page queries', () => {
  const parentRoute: RouteEntry = {
    path: '/coding/codex',
    component: TestComponent,
  };
  const secondaryRoute: RouteEntry = {
    path: '/coding/codex/sessions/detail',
    component: TestComponent,
    chrome: { mode: 'secondary' },
  };

  assert.equal(
    getRouteScrollKey(parentRoute, '/coding/codex', '?panel=sessions'),
    '/coding/codex',
  );
  assert.equal(
    getRouteScrollKey(secondaryRoute, '/coding/codex/sessions/detail', '?sourcePath=a'),
    '/coding/codex/sessions/detail?sourcePath=a',
  );
  assert.equal(
    getRouteScrollKey(undefined, '/missing', '?q=1'),
    '/missing?q=1',
  );
});
