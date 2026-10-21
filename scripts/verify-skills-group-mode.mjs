import {fileURLToPath} from 'node:url';
import {runBrowserFixture} from './lib/browser-fixture.mjs';
import {verifySkillsGroupMode} from '../web/test/features/coding/skills/groupMode/skillsGroupModeBrowserChecks.mjs';
await runBrowserFixture({fixtureDirectory:fileURLToPath(new URL('../web/test/features/coding/skills/groupMode/fixtures/',import.meta.url)),fixtureFilename:'SkillsGroupModeFixture.jsx',artifactPrefix:'skills-group-mode-',verify:verifySkillsGroupMode});
