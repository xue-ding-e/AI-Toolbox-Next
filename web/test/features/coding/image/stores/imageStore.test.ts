import assert from 'node:assert/strict';
import test from 'node:test';

import type {
  CreateImageJobInput,
  DeleteImageJobInput,
  ImageChannel,
  ImageJob,
  ImageWorkspace,
  UpsertImageChannelInput,
} from '../../../../../features/coding/image/services/imageApi.ts';
import { createImageStore } from '../../../../../features/coding/image/stores/imageStore.ts';

const sampleJobInput: CreateImageJobInput = {
  mode: 'text_to_image',
  prompt: 'test prompt',
  channel_id: 'channel-1',
  model_id: 'gpt-image-2',
  params: {
    size: '1024x1024',
    quality: 'high',
    output_format: 'png',
    output_compression: null,
    moderation: 'low',
  },
  references: [],
};

const sampleJob: ImageJob = {
  id: 'job-1',
  mode: 'text_to_image',
  prompt: 'test prompt',
  channel_id: 'channel-1',
  channel_name_snapshot: 'Channel 1',
  model_id: 'gpt-image-2',
  model_name_snapshot: 'GPT Image 2',
  params_json: '{}',
  status: 'done',
  error_message: null,
  request_url: null,
  request_headers_json: null,
  request_body_json: null,
  input_assets: [],
  output_assets: [],
  created_at: 100,
  finished_at: 120,
  elapsed_ms: 20,
};

const sampleWorkspace: ImageWorkspace = {
  channels: [],
  jobs: [],
};

function createTestDependencies(overrides?: Partial<{
  createImageJob: (input: CreateImageJobInput) => Promise<ImageJob>;
  deleteImageJob: (input: DeleteImageJobInput) => Promise<void>;
  deleteImageChannel: (id: string) => Promise<void>;
  getImageWorkspace: () => Promise<ImageWorkspace>;
  listImageJobs: (limit?: number) => Promise<ImageJob[]>;
  reorderImageChannels: (orderedIds: string[]) => Promise<ImageChannel[]>;
  updateImageChannel: (input: UpsertImageChannelInput) => Promise<ImageChannel>;
}>) {
  return {
    createImageJob: async (_input: CreateImageJobInput) => sampleJob,
    deleteImageJob: async (_input: DeleteImageJobInput) => {},
    deleteImageChannel: async (_id: string) => {},
    getImageWorkspace: async () => sampleWorkspace,
    listImageJobs: async (_limit = 50) => [sampleJob],
    reorderImageChannels: async (_orderedIds: string[]) => [],
    updateImageChannel: async (_input: UpsertImageChannelInput) => {
      throw new Error('not implemented');
    },
    ...overrides,
  };
}

test('submitJob keeps successful job in store when follow-up refresh fails', async () => {
  const imageStore = createImageStore(createTestDependencies({
    listImageJobs: async () => {
      throw new Error('temporary list failure');
    },
  }));

  const originalConsoleWarn = console.warn;
  console.warn = () => {};

  try {
    const result = await imageStore.getState().submitJob(sampleJobInput);
    const state = imageStore.getState();

    assert.equal(result.id, sampleJob.id);
    assert.equal(state.jobs.length, 1);
    assert.equal(state.jobs[0]?.id, sampleJob.id);
    assert.equal(state.lastJobId, sampleJob.id);
    assert.equal(state.submitting, false);
  } finally {
    console.warn = originalConsoleWarn;
  }
});

test('submitJob prefers refreshed jobs list when background refresh succeeds', async () => {
  const refreshedJob: ImageJob = {
    ...sampleJob,
    status: 'error',
    error_message: 'refreshed status',
    created_at: 200,
  };
  const imageStore = createImageStore(createTestDependencies({
    listImageJobs: async () => [refreshedJob],
  }));

  const result = await imageStore.getState().submitJob(sampleJobInput);
  const state = imageStore.getState();

  assert.equal(result.id, sampleJob.id);
  assert.equal(state.jobs.length, 1);
  assert.equal(state.jobs[0]?.id, refreshedJob.id);
  assert.equal(state.jobs[0]?.status, 'error');
  assert.equal(state.jobs[0]?.error_message, 'refreshed status');
  assert.equal(state.lastJobId, sampleJob.id);
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

test('refresh started before successful deletion cannot restore the deleted job', async () => {
  const response = deferred<ImageJob[]>();
  const store = createImageStore(createTestDependencies({ listImageJobs: () => response.promise }));
  store.setState({ jobs: [sampleJob], lastJobId: sampleJob.id });
  const refresh = store.getState().refreshJobs();
  await store.getState().removeJob(sampleJob.id, false);
  response.resolve([sampleJob]);
  await refresh;
  assert.deepEqual(store.getState().jobs, []);
  assert.equal(store.getState().lastJobId, null);
});

test('stale workspace jobs are ignored without dropping channel state or loading cleanup', async () => {
  const response = deferred<ImageWorkspace>();
  const channel: ImageChannel = {
    id: 'channel-1', name: 'Synthetic channel', provider_kind: 'openai_compatible',
    base_url: 'https://example.invalid', api_key: '', enabled: true, sort_order: 0,
    models: [], created_at: 1, updated_at: 1,
  };
  const store = createImageStore(createTestDependencies({ getImageWorkspace: () => response.promise }));
  store.setState({ jobs: [sampleJob], lastJobId: sampleJob.id });
  const workspace = store.getState().loadWorkspace();
  assert.equal(store.getState().loading, true);
  await store.getState().removeJob(sampleJob.id, false);
  response.resolve({ channels: [channel], jobs: [sampleJob] });
  await workspace;
  assert.deepEqual(store.getState().jobs, []);
  assert.equal(store.getState().lastJobId, null);
  assert.deepEqual(store.getState().channels, [channel]);
  assert.equal(store.getState().editingChannelId, channel.id);
  assert.equal(store.getState().loading, false);
});

test('failed deletion does not invalidate a pending legitimate refresh', async () => {
  const response = deferred<ImageJob[]>();
  const store = createImageStore(createTestDependencies({
    listImageJobs: () => response.promise,
    deleteImageJob: async () => { throw new Error('delete failed'); },
  }));
  const refresh = store.getState().refreshJobs();
  await assert.rejects(store.getState().removeJob(sampleJob.id, false), /delete failed/);
  response.resolve([sampleJob]);
  await refresh;
  assert.deepEqual(store.getState().jobs, [sampleJob]);
  assert.equal(store.getState().lastJobId, sampleJob.id);
});

test('the latest history refresh wins when responses arrive out of order', async () => {
  const older = deferred<ImageJob[]>();
  const newer = deferred<ImageJob[]>();
  let reads = 0;
  const store = createImageStore(createTestDependencies({
    listImageJobs: () => ++reads === 1 ? older.promise : newer.promise,
  }));
  const first = store.getState().refreshJobs();
  const second = store.getState().refreshJobs();
  newer.resolve([]);
  await second;
  older.resolve([sampleJob]);
  await first;
  assert.deepEqual(store.getState().jobs, []);
  assert.equal(store.getState().lastJobId, null);
});

test('submit follow-up refresh cannot resurrect a successfully deleted job', async () => {
  const response = deferred<ImageJob[]>();
  const started = deferred<void>();
  const store = createImageStore(createTestDependencies({
    listImageJobs: () => { started.resolve(); return response.promise; },
  }));
  const submit = store.getState().submitJob(sampleJobInput);
  await started.promise;
  assert.equal(store.getState().lastJobId, sampleJob.id);
  await store.getState().removeJob(sampleJob.id, false);
  response.resolve([sampleJob]);
  await submit;
  assert.deepEqual(store.getState().jobs, []);
  assert.equal(store.getState().lastJobId, null);
  assert.equal(store.getState().submitting, false);
});

test('a new refresh after deletion can still apply backend state', async () => {
  const survivor = { ...sampleJob, id: 'job-2' };
  const store = createImageStore(createTestDependencies({ listImageJobs: async () => [survivor] }));
  store.setState({ jobs: [sampleJob], lastJobId: sampleJob.id });
  await store.getState().removeJob(sampleJob.id, false);
  await store.getState().refreshJobs();
  assert.deepEqual(store.getState().jobs, [survivor]);
  assert.equal(store.getState().lastJobId, survivor.id);
});

test('an older refresh cannot hide a successful generation before its follow-up resolves', async () => {
  const older = deferred<ImageJob[]>();
  const followUp = deferred<ImageJob[]>();
  const started = deferred<void>();
  let reads = 0;
  const store = createImageStore(createTestDependencies({
    listImageJobs: () => {
      if (++reads === 1) return older.promise;
      started.resolve();
      return followUp.promise;
    },
  }));
  const refresh = store.getState().refreshJobs();
  const submit = store.getState().submitJob(sampleJobInput);
  await started.promise;
  older.resolve([]);
  await refresh;
  assert.deepEqual(store.getState().jobs, [sampleJob]);
  assert.equal(store.getState().lastJobId, sampleJob.id);
  followUp.resolve([sampleJob]);
  await submit;
  assert.deepEqual(store.getState().jobs, [sampleJob]);
});
