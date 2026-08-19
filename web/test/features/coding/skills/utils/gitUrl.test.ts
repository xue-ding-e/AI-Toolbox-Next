import assert from 'node:assert/strict';
import test from 'node:test';

import {
  normalizeGitUrlToHttps,
  parseGitRepo,
} from '../../../../../features/coding/skills/utils/gitUrl.ts';

test('parseGitRepo parses HTTPS URLs with and without .git suffix', () => {
  assert.deepEqual(parseGitRepo('https://github.com/anthropics/skills'), {
    host: 'github.com',
    owner: 'anthropics',
    repo: 'skills',
  });
  assert.deepEqual(parseGitRepo('https://github.com/anthropics/skills.git'), {
    host: 'github.com',
    owner: 'anthropics',
    repo: 'skills',
  });
});

test('parseGitRepo parses SCP-style URLs from GitHub and custom hosts', () => {
  assert.deepEqual(parseGitRepo('git@github.com:anthropics/skills.git'), {
    host: 'github.com',
    owner: 'anthropics',
    repo: 'skills',
  });
  assert.deepEqual(parseGitRepo('git@gitlab.com:team/super-skills.git'), {
    host: 'gitlab.com',
    owner: 'team',
    repo: 'super-skills',
  });
});

test('parseGitRepo parses ssh:// scheme URLs with optional port', () => {
  assert.deepEqual(parseGitRepo('ssh://git@github.com/anthropics/skills.git'), {
    host: 'github.com',
    owner: 'anthropics',
    repo: 'skills',
  });
  assert.deepEqual(parseGitRepo('ssh://git@gitlab.com:2222/team/skills.git'), {
    host: 'gitlab.com',
    owner: 'team',
    repo: 'skills',
  });
});

test('parseGitRepo preserves GitLab subgroup paths for every remote shape', () => {
  const expected = {
    host: 'gitlab.com',
    owner: 'group/subgroup',
    repo: 'skills',
  };
  assert.deepEqual(parseGitRepo('https://gitlab.com/group/subgroup/skills.git'), expected);
  assert.deepEqual(parseGitRepo('ssh://git@gitlab.com:2222/group/subgroup/skills.git'), expected);
  assert.deepEqual(parseGitRepo('git@gitlab.com:group/subgroup/skills.git'), expected);
});

test('parseGitRepo ignores paths after an explicit .git repository segment', () => {
  assert.deepEqual(parseGitRepo('git@gitlab.com:team/skills.git/sub/dir'), {
    host: 'gitlab.com',
    owner: 'team',
    repo: 'skills',
  });
});

test('parseGitRepo rejects non-git inputs', () => {
  assert.equal(parseGitRepo('C:\\Users\\ralph\\skills'), null);
  assert.equal(parseGitRepo('/local/path/skill'), null);
  assert.equal(parseGitRepo(''), null);
  assert.equal(parseGitRepo(null), null);
  assert.equal(parseGitRepo(undefined), null);
  assert.equal(parseGitRepo('not a url'), null);
});

test('normalizeGitUrlToHttps converts all URL shapes to HTTPS web URLs', () => {
  assert.equal(normalizeGitUrlToHttps('git@gitlab.com:team/super-skills.git'), 'https://gitlab.com/team/super-skills');
  assert.equal(normalizeGitUrlToHttps('ssh://git@github.com/anthropics/skills.git'), 'https://github.com/anthropics/skills');
  assert.equal(normalizeGitUrlToHttps('ssh://git@gitlab.com:2222/team/skills.git'), 'https://gitlab.com/team/skills');
  assert.equal(normalizeGitUrlToHttps('https://github.com/anthropics/skills.git'), 'https://github.com/anthropics/skills');
  assert.equal(normalizeGitUrlToHttps('https://gitlab.com/group/subgroup/skills.git'), 'https://gitlab.com/group/subgroup/skills');
});

test('normalizeGitUrlToHttps returns null for non-git inputs', () => {
  assert.equal(normalizeGitUrlToHttps('C:\\Users\\ralph\\skills'), null);
  assert.equal(normalizeGitUrlToHttps('/local/path/skill'), null);
  assert.equal(normalizeGitUrlToHttps('not a url'), null);
  assert.equal(normalizeGitUrlToHttps(null), null);
});
