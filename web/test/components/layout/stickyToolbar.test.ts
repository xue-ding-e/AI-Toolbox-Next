import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const readSource = (relativePath: string) =>
  readFileSync(new URL(relativePath, import.meta.url), 'utf8');

const appCss = readSource('../../../App.css');
const skillsToolbar = readSource(
  '../../../features/coding/skills/pages/SkillsPage.module.less',
);
const mcpToolbar = readSource('../../../features/coding/mcp/pages/McpPage.module.less');
const sessionToolbar = readSource(
  '../../../features/coding/shared/sessionManager/SessionManagerPanel.module.less',
);
const gatewayHeader = readSource(
  '../../../features/coding/gateway/pages/GatewayPage.module.less',
);
const gatewayStats = readSource(
  '../../../features/coding/gateway/components/GatewayStatisticsView.module.less',
);
const gatewayRequests = readSource(
  '../../../features/coding/gateway/components/GatewayRequestsView.module.less',
);
const imageHeader = readSource(
  '../../../features/coding/image/pages/ImagePage.module.less',
);

const collapseBlock = appCss.slice(
  appCss.indexOf('/* Global Collapse card style'),
  appCss.indexOf('/* ProLayout dark mode support'),
);

test('page-level collapse overflow stays visible so sticky headers can pin', () => {
  assert.match(collapseBlock, /\.ant-collapse \{[\s\S]*?overflow:\s*visible;/);
  assert.doesNotMatch(
    collapseBlock.replace(/\.ant-modal[\s\S]*?overflow:\s*hidden;[\s\S]*?\}/g, ''),
    /\.ant-collapse \{[\s\S]*?overflow:\s*hidden;/,
  );
});

test('page-level collapse headers freeze at the content box, not under a second header offset', () => {
  // `main` already pads by `--content-top-offset`. Sticky top is relative to
  // that padded content box; repeating the offset leaves a blank band.
  assert.match(
    collapseBlock,
    /\.ant-collapse > \.ant-collapse-item > \.ant-collapse-header \{[\s\S]*?position:\s*sticky;[\s\S]*?top:\s*0;/,
  );
  assert.doesNotMatch(
    collapseBlock,
    /\.ant-collapse > \.ant-collapse-item > \.ant-collapse-header \{[\s\S]*?top:\s*var\(--content-top-offset/,
  );
  assert.doesNotMatch(
    collapseBlock,
    /\.ant-collapse > \.ant-collapse-item > \.ant-collapse-header \{[\s\S]*?position:\s*fixed;/,
  );
});

test('nested, modal and drawer collapse headers unfreeze instead of stacking', () => {
  assert.match(
    collapseBlock,
    /\.ant-collapse \.ant-collapse > \.ant-collapse-item > \.ant-collapse-header,[\s\S]*?position:\s*static;/,
  );
  assert.match(collapseBlock, /\.ant-modal \.ant-collapse,[\s\S]*?overflow:\s*hidden;/);
});

test('browse toolbars freeze at the content box and unfreeze with their page', () => {
  for (const source of [skillsToolbar, mcpToolbar, gatewayHeader, imageHeader]) {
    assert.match(source, /position:\s*sticky;/);
    assert.match(source, /top:\s*0;/);
    assert.doesNotMatch(source, /position:\s*fixed;/);
    assert.doesNotMatch(source, /top:\s*var\(--content-top-offset/);
  }

  assert.match(sessionToolbar, /\.toolbar \{[\s\S]*?position:\s*sticky;/);
  assert.match(sessionToolbar, /\.toolbar \{[\s\S]*?top:\s*48px;/);
  assert.doesNotMatch(sessionToolbar, /\.toolbar \{[\s\S]*?top:\s*calc\(var\(--content-top-offset/);
  assert.match(gatewayStats, /\.filterBar \{[\s\S]*?position:\s*sticky;/);
  assert.match(gatewayStats, /\.filterBar \{[\s\S]*?top:\s*64px;/);
  assert.match(gatewayRequests, /\.filterBar \{[\s\S]*?position:\s*sticky;/);
  assert.match(gatewayRequests, /\.filterBar \{[\s\S]*?top:\s*64px;/);
});
