/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  parseHelpInline,
  parseHelpText,
} from '../../../../components/common/FieldHelp/parseHelpText.ts';

test('a blank line separates paragraphs', () => {
  assert.deepEqual(parseHelpText('第一段。\n\n第二段。'), [
    { kind: 'paragraph', text: '第一段。' },
    { kind: 'paragraph', text: '第二段。' },
  ]);
});

test('a single newline stays inside its paragraph', () => {
  // The copy relies on this: a two-line sentence is one paragraph, and the
  // renderer turns the newline into a line break rather than a new block.
  assert.deepEqual(parseHelpText('第一行。\n第二行。'), [
    { kind: 'paragraph', text: '第一行。\n第二行。' },
  ]);
});

test('a block of "- " lines becomes one bullet list', () => {
  assert.deepEqual(parseHelpText('- 文本：接收文本内容。\n- 图片：接收图片内容。'), [
    {
      kind: 'list',
      items: ['文本：接收文本内容。', '图片：接收图片内容。'],
    },
  ]);
});

test('a paragraph followed by a bullet list keeps both blocks in order', () => {
  const blocks = parseHelpText('设置模型能够接收的内容类型：\n\n- 文本\n- 图片');

  assert.deepEqual(blocks, [
    { kind: 'paragraph', text: '设置模型能够接收的内容类型：' },
    { kind: 'list', items: ['文本', '图片'] },
  ]);
});

test('surrounding whitespace does not create empty blocks', () => {
  assert.deepEqual(parseHelpText('\n\n  一段话。  \n\n'), [
    { kind: 'paragraph', text: '一段话。' },
  ]);
});

test('bold markers become their own run', () => {
  assert.deepEqual(parseHelpInline('支持**结构化输出**能力'), [
    { kind: 'text', text: '支持' },
    { kind: 'bold', text: '结构化输出' },
    { kind: 'text', text: '能力' },
  ]);
});

test('backtick markers become their own run', () => {
  assert.deepEqual(parseHelpInline('将 `reasoningLevel` 映射'), [
    { kind: 'text', text: '将 ' },
    { kind: 'code', text: 'reasoningLevel' },
    { kind: 'text', text: ' 映射' },
  ]);
});

test('a line with no markers is one text run', () => {
  assert.deepEqual(parseHelpInline('普通说明文字。'), [
    { kind: 'text', text: '普通说明文字。' },
  ]);
});

test('the real capabilities copy parses into a paragraph plus three bullets', () => {
  // Guards the actual shipped string: a translation edit that drops a "- " or
  // collapses a blank line would silently render as one run-on line.
  const copy =
    '- **结构化输出**：支持通过 JSON Schema 约束模型输出的字段、类型和结构。\n' +
    '- **原生联网搜索**：支持使用模型接口内置的联网搜索能力。\n' +
    '- **对话中系统消息**：支持在对话中途插入系统指令。\n\n' +
    '请勿勾选模型不支持的能力。';

  const blocks = parseHelpText(copy);

  assert.equal(blocks.length, 2);
  assert.equal(blocks[0].kind, 'list');
  assert.equal(blocks[0].kind === 'list' && blocks[0].items.length, 3);
  assert.deepEqual(blocks[1], {
    kind: 'paragraph',
    text: '请勿勾选模型不支持的能力。',
  });

  // Each bullet leads with a bold term.
  const firstItem = blocks[0].kind === 'list' ? blocks[0].items[0] : '';
  assert.deepEqual(parseHelpInline(firstItem)[0], {
    kind: 'bold',
    text: '结构化输出',
  });
});
