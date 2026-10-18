import assert from 'node:assert/strict';
import test from 'node:test';
import { useSkillsStore } from '../../../../../features/coding/skills/stores/skillsStore.ts';
import type { ManagedSkill, OnboardingPlan, ToolStatus } from '../../../../../features/coding/skills/types/index.ts';

const originalWindow = (globalThis as { window?: unknown }).window;
const initialState = useSkillsStore.getInitialState();
const skill: ManagedSkill = {
  id: 'synthetic-skill', name: 'Synthetic skill', source_type: 'central', source_ref: null,
  central_path: '/synthetic/central/skill', created_at: 1, updated_at: 1, last_sync_at: null,
  status: 'ok', sort_index: 0, user_group: null, group_id: null, user_note: null,
  management_enabled: true, disabled_previous_tools: [], tags: [], description: null,
  content_hash: null, source_health: 'ok', source_error: null, enabled_tools: [], targets: [],
};
const tools: ToolStatus = {
  tools: [
    { key: 'installed-tool', label: 'Installed', installed: true, skills_dir: '/synthetic/tool', icon_url: 'https://example.invalid/icon.svg' },
    { key: 'absent-tool', label: 'Absent', installed: false, skills_dir: '' },
  ],
  installed: ['installed-tool'], newly_installed: [],
};
const plan: OnboardingPlan = { total_tools_scanned: 2, total_skills_found: 0, groups: [] };
const group = { id: 'group-1', name: 'Synthetic group', note: null, sort_index: 0, created_at: 1, updated_at: 1 };

function mockInvoke(invoke: (command: string) => Promise<unknown>) {
  (globalThis as { window?: unknown }).window = { __TAURI_INTERNALS__: { invoke } };
}
function refreshResponse(command: string): unknown {
  switch (command) {
    case 'skills_get_tool_status': return tools;
    case 'skills_get_managed_skills': return [skill];
    case 'skills_get_groups': return [group];
    case 'skills_get_central_repo_path': return '/synthetic/central';
    default: throw new Error(`Unexpected command: ${command}`);
  }
}
test.beforeEach(() => useSkillsStore.setState(initialState, true));
test.afterEach(() => { (globalThis as { window?: unknown }).window = originalWindow; });

test('refresh reads central workspace without automatically scanning or importing skills', async () => {
  const commands: string[] = [];
  mockInvoke(async (command) => { commands.push(command); return refreshResponse(command); });
  await useSkillsStore.getState().refresh();
  assert.deepEqual(commands.sort(), [
    'skills_get_central_repo_path', 'skills_get_groups', 'skills_get_managed_skills', 'skills_get_tool_status',
  ]);
  const state = useSkillsStore.getState();
  assert.deepEqual(state.skills, [skill]);
  assert.deepEqual(state.toolStatus, tools);
  assert.deepEqual(state.groups, [group]);
  assert.equal(state.centralRepoPath, '/synthetic/central');
  assert.equal(state.onboardingPlan, null);
  assert.equal(state.loading, false);
  assert.equal(state.error, null);
});

test('failed skills read preserves current data while independent refresh reads complete', async (context) => {
  const oldSkill = { ...skill, id: 'previous-skill' };
  useSkillsStore.setState({ skills: [oldSkill] });
  context.mock.method(console, 'error', () => {});
  mockInvoke(async (command) => {
    if (command === 'skills_get_managed_skills') throw new Error('synthetic read failure');
    return refreshResponse(command);
  });
  await useSkillsStore.getState().refresh();
  const state = useSkillsStore.getState();
  assert.deepEqual(state.skills, [oldSkill]);
  assert.deepEqual(state.groups, [group]);
  assert.deepEqual(state.toolStatus, tools);
  assert.equal(state.centralRepoPath, '/synthetic/central');
  assert.equal(state.loading, false);
  assert.match(state.error ?? '', /synthetic read failure/);
});

test('a successful retry clears the skills read error and replaces the previous list', async (context) => {
  context.mock.method(console, 'error', () => {});
  mockInvoke(async () => { throw new Error('synthetic failure'); });
  await useSkillsStore.getState().loadSkills();
  assert.match(useSkillsStore.getState().error ?? '', /synthetic failure/);
  mockInvoke(async (command) => { assert.equal(command, 'skills_get_managed_skills'); return [skill]; });
  await useSkillsStore.getState().loadSkills();
  assert.equal(useSkillsStore.getState().error, null);
  assert.deepEqual(useSkillsStore.getState().skills, [skill]);
  assert.equal(useSkillsStore.getState().loading, false);
});

test('manual scan followed by closing import state does not import or alter managed skills', async () => {
  const commands: string[] = [];
  useSkillsStore.setState({ skills: [skill] });
  mockInvoke(async (command) => {
    commands.push(command);
    assert.equal(command, 'skills_get_onboarding_plan');
    return plan;
  });
  useSkillsStore.getState().setImportModalOpen(true);
  await useSkillsStore.getState().loadOnboardingPlan();
  assert.deepEqual(useSkillsStore.getState().onboardingPlan, plan);
  useSkillsStore.getState().setImportModalOpen(false);
  assert.equal(useSkillsStore.getState().isImportModalOpen, false);
  assert.deepEqual(useSkillsStore.getState().skills, [skill]);
  assert.deepEqual(commands, ['skills_get_onboarding_plan']);
});

test('installed-tool options filter unavailable tools while retaining path and icon metadata', async () => {
  mockInvoke(async (command) => { assert.equal(command, 'skills_get_tool_status'); return tools; });
  await useSkillsStore.getState().loadToolStatus();
  assert.deepEqual(useSkillsStore.getState().getInstalledTools(), [{
    id: 'installed-tool', label: 'Installed', installed: true,
    skillDir: '/synthetic/tool', iconUrl: 'https://example.invalid/icon.svg',
  }]);
  assert.deepEqual(useSkillsStore.getState().getAllTools()[1], {
    id: 'absent-tool', label: 'Absent', installed: false, skillDir: null, iconUrl: null,
  });
});
