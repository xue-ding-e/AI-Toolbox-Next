import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const readSource = (relativePath: string) =>
  readFileSync(new URL(relativePath, import.meta.url), 'utf8');

const settingsSource = readSource(
  '../../../../../features/coding/gateway/components/GatewayAggregateSettings.tsx',
);
const settingsStyles = readSource(
  '../../../../../features/coding/gateway/components/GatewayAggregateSettings.module.less',
);

/**
 * Site priority has exactly one source: the CLI provider list order (`sort_index`,
 * the order the user drags in the provider list). The aggregate panel used to keep
 * a second, panel-local order with drag handles and up/down buttons, which made the
 * two lists disagree. The panel now only renders and writes the provider order.
 */
test('the aggregate panel never owns a local site order', () => {
  assert.match(
    settingsSource,
    /const orderedSiteIds = orderAggregateSiteIdsByCandidates\(nextSiteIds, candidates\);/,
  );

  // No panel-local reordering path may come back.
  assert.doesNotMatch(settingsSource, /@dnd-kit/);
  assert.doesNotMatch(settingsSource, /DndContext|SortableContext|useSortable/);
  assert.doesNotMatch(settingsSource, /moveAggregateSite/);
  assert.doesNotMatch(settingsStyles, /\.dragHandle \{/);
  assert.doesNotMatch(settingsStyles, /\.siteActions \{/);

  // The panel explains where the order actually comes from.
  assert.match(settingsSource, /t\('gateway\.aggregate\.orderHint'\)/);
});

/**
 * A site id is the provider's 32-character UUID primary key, which is unreadable
 * in the dense list. The rows show an eight-character display handle and keep the
 * full id in the tooltip; everything written to the backend keeps the full id.
 */
test('site rows show a short display handle instead of the raw provider id', () => {
  assert.match(settingsSource, /import \{[^}]*shortAggregateSiteId[^}]*\}/s);
  assert.match(
    settingsSource,
    /<code className=\{styles\.siteSlug\} title=\{candidate\.id\}>\s*\{shortAggregateSiteId\(candidate\.id\)\}/,
  );
  assert.doesNotMatch(settingsSource, /\{candidate\.id\}\s*<\/code>/);
});

/**
 * The backend refuses to change `primary_provider_id` while a manifest is
 * enabled, and aggregate names the first selected site as the primary. The panel
 * therefore has to restore direct mode *before* engaging whenever the first site
 * would change; otherwise the user sees the raw English guard sentence again.
 */
test('an engage that switches the primary restores direct first', () => {
  assert.match(settingsSource, /aggregateEngageRequiresDirectRestore/);
  assert.match(
    settingsSource,
    /if \(requiresDirectRestore\) \{[\s\S]*?await restoreProxyGatewayCliDirect\(cliKey\);[\s\S]*?\n\s*\}\n\s*return engageProxyGatewayAggregate\(/,
  );
  // The block case must be refused with the localized hint, never by writing a
  // direct config the CLI cannot use.
  assert.match(settingsSource, /setNotice\(\{ kind: 'error', text: restoreDirectBlockedHint \}\)/);
  assert.match(settingsSource, /aggregateEngageErrorNoticeKey/);
});
