import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyGatewayRequestObservations } from '../web/test/features/coding/gateway/gatewayRequestObservationsBrowserChecks.mjs';

await runBrowserFixture({
  fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/gateway/fixtures', import.meta.url)),
  fixtureFilename: 'GatewayRequestObservationsFixture.jsx',
  artifactPrefix: 'gateway-request-observations-',
  verify: verifyGatewayRequestObservations,
  fixtureAliases: [{
    // Keep the real gateway command wrappers without initializing unrelated APIs.
    find: /^@\/services$/,
    replacement: fileURLToPath(new URL('../web/services/proxyGatewayApi.ts', import.meta.url)),
  }],
});
