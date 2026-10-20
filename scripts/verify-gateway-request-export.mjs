import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyGatewayRequestExport } from '../web/test/features/coding/gateway/gatewayRequestExportBrowserChecks.mjs';

await runBrowserFixture({
  fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/gateway/fixtures', import.meta.url)),
  fixtureFilename: 'GatewayRequestExportFixture.jsx',
  artifactPrefix: 'gateway-request-export-',
  verify: verifyGatewayRequestExport,
  fixtureAliases: [{
    // Narrow only the services barrel. The dialog and export wrappers stay real.
    find: /^@\/services$/,
    replacement: fileURLToPath(new URL('../web/services/proxyGatewayApi.ts', import.meta.url)),
  }],
});
