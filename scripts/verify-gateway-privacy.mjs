import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyGatewayPrivacy } from '../web/test/features/coding/gateway/gatewayPrivacyBrowserChecks.mjs';

const fixtureDirectory = fileURLToPath(new URL("../web/test/features/coding/gateway/fixtures", import.meta.url));
await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: "GatewayPrivacyFixture.jsx",
  artifactPrefix: "gateway-privacy-",
  verify: verifyGatewayPrivacy,
  fixtureAliases: [
  ],
});
