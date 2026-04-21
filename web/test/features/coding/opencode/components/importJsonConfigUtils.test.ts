/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  extractImportedConfigData,
  parseImportedConfigText,
  resolveSlimImportedAgents,
} from '../../../../../features/coding/opencode/components/importJsonConfigUtils.ts';

test('resolveSlimImportedAgents reads active preset agents for OMOS author preset configs', () => {
  const result = resolveSlimImportedAgents({
    preset: 'openai',
    presets: {
      openai: {
        orchestrator: { model: 'openai/gpt-5.4-fast', skills: ['*'] },
        oracle: { model: 'openai/gpt-5.4-fast', variant: 'high' },
      },
    },
  });

  assert.deepEqual(result, {
    orchestrator: { model: 'openai/gpt-5.4-fast', skills: ['*'] },
    oracle: { model: 'openai/gpt-5.4-fast', variant: 'high' },
  });
});

test('resolveSlimImportedAgents merges root agents over active preset agents', () => {
  const result = resolveSlimImportedAgents({
    preset: 'openai',
    presets: {
      openai: {
        orchestrator: { model: 'openai/gpt-5.4-fast', variant: 'medium', skills: ['plan'] },
        oracle: { model: 'openai/gpt-5.4-fast', variant: 'high' },
      },
    },
    agents: {
      orchestrator: { variant: 'high', mcps: ['websearch'] },
    },
  });

  assert.deepEqual(result, {
    orchestrator: {
      model: 'openai/gpt-5.4-fast',
      variant: 'high',
      skills: ['plan'],
      mcps: ['websearch'],
    },
    oracle: { model: 'openai/gpt-5.4-fast', variant: 'high' },
  });
});

test('extractImportedConfigData excludes slim preset metadata from otherFields while preserving real config fields', () => {
  const result = extractImportedConfigData({
    preset: 'openai',
    presets: {
      openai: {
        orchestrator: { model: 'openai/gpt-5.4-fast' },
      },
    },
    multiplexer: {
      type: 'auto',
      layout: 'main-vertical',
    },
    council: {
      master: { model: 'openai/gpt-5.4' },
    },
  }, 'omos');

  assert.deepEqual(result, {
    agents: {
      orchestrator: { model: 'openai/gpt-5.4-fast' },
    },
    categories: undefined,
    otherFields: {
      multiplexer: {
        type: 'auto',
        layout: 'main-vertical',
      },
      council: {
        master: { model: 'openai/gpt-5.4' },
      },
    },
  });
});

test('parseImportedConfigText parses the issue #151 OMOS author preset without returning empty content', () => {
  const raw = `{
  "preset": "openai",
  "presets": {
    "openai": { "orchestrator": { "model": "openai/gpt-5.4-fast", "skills": [ "*" ], "mcps": [ "*", "websearch"] },
        "oracle": { "model": "openai/gpt-5.4-fast", "variant": "high", "skills": [], "mcps": [] },
        "librarian": { "model": "openai/gpt-5.3-codex-spark", "variant": "low", "skills": [], "mcps": [ "websearch", "context7", "grep_app" ] },
        "explorer": { "model": "openai/gpt-5.3-codex-spark", "variant": "low", "skills": [], "mcps": [] },
        "designer": { "model": "github-copilot/gemini-3.1-pro-preview", "skills": [ "agent-browser" ], "mcps": [] },
        "fixer": { "model": "openai/gpt-5.3-codex-spark", "variant": "low", "skills": [], "mcps": [] }
    }
  },
  "multiplexer": {
    "type": "auto",
    "layout": "main-vertical",
    "main_pane_size": 60
  },
  "council": {
    "master": { "model": "openai/gpt-5.4" },
    "presets": {
      "default": {
        "alpha":  { "model": "github-copilot/claude-opus-4.6" },
        "beta": { "model": "github-copilot/gemini-3.1-pro-preview" },
        "gamma": { "model": "fireworks-ai/accounts/fireworks/routers/kimi-k2p5-turbo" }
      }
    }
  }
}`;

  const result = parseImportedConfigText(raw, 'omos');

  assert.ok(result);
  assert.equal(result?.agents?.orchestrator?.model, 'openai/gpt-5.4-fast');
  assert.equal(result?.agents?.oracle?.variant, 'high');
  assert.deepEqual(result?.otherFields, {
    multiplexer: {
      type: 'auto',
      layout: 'main-vertical',
      main_pane_size: 60,
    },
    council: {
      master: { model: 'openai/gpt-5.4' },
      presets: {
        default: {
          alpha: { model: 'github-copilot/claude-opus-4.6' },
          beta: { model: 'github-copilot/gemini-3.1-pro-preview' },
          gamma: { model: 'fireworks-ai/accounts/fireworks/routers/kimi-k2p5-turbo' },
        },
      },
    },
  });
});

test('parseImportedConfigText parses issue #154 OMOS thirty-dollars preset by falling back to the only preset entry', () => {
  const raw = `{
  "preset": "thirtydollars",
  "presets": {
    "best": { "orchestrator": { "model": "openai/gpt-5.4", "skills": [ "*" ], "mcps": [ "*", "websearch"] },
        "oracle": { "model": "openai/gpt-5.4", "variant": "high", "skills": [], "mcps": [] },
        "librarian": { "model": "openai/gpt-5.4-mini", "variant": "low", "skills": [], "mcps": [ "websearch", "context7", "grep_app" ] },
        "explorer": { "model": "openai/gpt-5.4-mini", "variant": "low", "skills": [], "mcps": [] },
        "designer": { "model": "github-copilot/gemini-3.1-pro-preview", "skills": [ "agent-browser" ], "mcps": [] },
        "fixer": { "model": "openai/gpt-5.4-mini", "variant": "low", "skills": [], "mcps": [] }
    }
  },
  "council": {
    "master": { "model": "openai/gpt-5.4" },
    "presets": {
      "default": {
        "alpha":  { "model": "github-copilot/claude-opus-4.6" },
        "beta": { "model": "github-copilot/gemini-3.1-pro-preview" },
        "gamma": { "model": "openai/gpt-5.4" }
      }
    }
  }
}`;

  const result = parseImportedConfigText(raw, 'omos');

  assert.ok(result);
  assert.equal(result?.agents?.orchestrator?.model, 'openai/gpt-5.4');
  assert.equal(result?.agents?.oracle?.variant, 'high');
  assert.deepEqual(result?.otherFields, {
    council: {
      master: { model: 'openai/gpt-5.4' },
      presets: {
        default: {
          alpha: { model: 'github-copilot/claude-opus-4.6' },
          beta: { model: 'github-copilot/gemini-3.1-pro-preview' },
          gamma: { model: 'openai/gpt-5.4' },
        },
      },
    },
  });
});

test('parseImportedConfigText parses issue #154 OMOS thirty-dollars preset after aligning preset name with preset key', () => {
  const raw = `{
  "preset": "best",
  "presets": {
    "best": { "orchestrator": { "model": "openai/gpt-5.4", "skills": [ "*" ], "mcps": [ "*", "websearch"] },
        "oracle": { "model": "openai/gpt-5.4", "variant": "high", "skills": [], "mcps": [] },
        "librarian": { "model": "openai/gpt-5.4-mini", "variant": "low", "skills": [], "mcps": [ "websearch", "context7", "grep_app" ] },
        "explorer": { "model": "openai/gpt-5.4-mini", "variant": "low", "skills": [], "mcps": [] },
        "designer": { "model": "github-copilot/gemini-3.1-pro-preview", "skills": [ "agent-browser" ], "mcps": [] },
        "fixer": { "model": "openai/gpt-5.4-mini", "variant": "low", "skills": [], "mcps": [] }
    }
  },
  "council": {
    "master": { "model": "openai/gpt-5.4" },
    "presets": {
      "default": {
        "alpha":  { "model": "github-copilot/claude-opus-4.6" },
        "beta": { "model": "github-copilot/gemini-3.1-pro-preview" },
        "gamma": { "model": "openai/gpt-5.4" }
      }
    }
  }
}`;

  const result = parseImportedConfigText(raw, 'omos');

  assert.ok(result);
  assert.equal(result?.agents?.orchestrator?.model, 'openai/gpt-5.4');
  assert.equal(result?.agents?.oracle?.variant, 'high');
  assert.deepEqual(result?.otherFields, {
    council: {
      master: { model: 'openai/gpt-5.4' },
      presets: {
        default: {
          alpha: { model: 'github-copilot/claude-opus-4.6' },
          beta: { model: 'github-copilot/gemini-3.1-pro-preview' },
          gamma: { model: 'openai/gpt-5.4' },
        },
      },
    },
  });
});
