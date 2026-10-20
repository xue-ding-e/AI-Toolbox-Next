import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyGatewayRequestExportOwnership } from '../web/test/features/coding/gateway/gatewayRequestExportOwnershipBrowserChecks.mjs';

await runBrowserFixture({
  fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/gateway/fixtures', import.meta.url)),
  fixtureFilename: 'GatewayRequestExportOwnershipFixture.jsx',
  artifactPrefix: 'gateway-request-export-ownership-',
  verify: verifyGatewayRequestExportOwnership,
  fixtureAliases: [{
    // Preserve the real component, dialog save wrapper, and export API.
    find: /^@\/services$/,
    replacement: fileURLToPath(new URL('../web/services/proxyGatewayApi.ts', import.meta.url)),
  }],
});
