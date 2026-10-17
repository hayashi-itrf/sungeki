import { expect, test, vi } from 'vitest';
import type { MarkdownScene, SessionResponse } from '@sungeki/api-client';
import { MarkdownInput, markdownScenesToForm, markdownAppendSummary } from './markdown-input.svelte';
import { newScene } from './scenes';
import { parseDialogue } from './dialogue';

const scenes: MarkdownScene[] = [{ name: '導入', lines: [
  { role: 'left', subtitle_text: '**本文**: https://example.test | 読み：そのまま', spoken_text: 'コーデックス', pose: null },
  { role: 'right', subtitle_text: '左：これも本文です。', spoken_text: null, pose: null },
] }];

test('form adaptation preserves literal text and pronunciation without sharing objects', () => {
  const before = JSON.stringify(scenes);
  const form = markdownScenesToForm(scenes);
  expect(parseDialogue(form[0]!.script)).toEqual(scenes[0]!.lines);
  expect(form[0]!.background).toBe('');
  expect(form[0]!.diagrams).toEqual([]);
  form[0]!.name = 'changed';
  expect(JSON.stringify(scenes)).toBe(before);
});

test('Unicode line and paragraph separators survive the Markdown-to-form round trip', () => {
  const input: MarkdownScene[] = [{ name: '貼り付けた台本', lines: [
    { role: 'left', subtitle_text: '前半\u2028後半\u2029結び', spoken_text: 'ぜんはん\u2028こうはん\u2029むすび', pose: null },
  ] }];
  const form = markdownScenesToForm(input);
  expect(parseDialogue(form[0]!.script)).toEqual(input[0]!.lines);
  expect(markdownAppendSummary([], form).speeches).toBe(1);
});

test('host errors retain the input context and do not create an applicable result', async () => {
  const session = vi.fn().mockRejectedValue({ code: 'VALIDATION_FAILED', message: '3行目: 未登録の役名です' });
  const input = new MarkdownInput({ session });
  await input.parse('## 導入\n\n- right: 本文', true);
  expect(session).toHaveBeenCalledWith({ type: 'parse_markdown_script', args: {
    source: '## 導入\n\n- right: 本文', roles: ['left'],
  } });
  expect(input.result).toBeNull(); expect(input.error).toContain('3行目'); expect(input.busy).toBe(false);
});

test('a late parse cannot replace a newer result or populate a closed form', async () => {
  const pending: ((r: SessionResponse) => void)[] = [];
  const session = vi.fn(() => new Promise<SessionResponse>(resolve => pending.push(resolve)));
  const input = new MarkdownInput({ session });
  const first = input.parse('old', false), second = input.parse('new', false);
  pending[1]!({ type: 'markdown_script', scenes }); await second;
  pending[0]!({ type: 'markdown_script', scenes: [] }); await first;
  expect(input.result?.source).toBe('new');
  const closing = input.parse('closing', true); input.dispose();
  pending[2]!({ type: 'markdown_script', scenes }); await closing;
  expect(input.result).toBeNull();
});

test('append counts the complete draft, excluding readings and blank lines, at the 200-speech boundary', () => {
  const existing = { ...newScene('既存'), script: '左：本文\n読み：ホンブン\n\n'.repeat(199) };
  const added = { ...newScene('追加'), script: '右：追加' };
  const before = JSON.stringify(existing);
  expect(markdownAppendSummary([existing], [added])).toEqual({ scenes: 2, speeches: 200, invalidScene: null, allowed: true });
  expect(markdownAppendSummary([existing], [added, added])).toEqual({ scenes: 3, speeches: 201, invalidScene: null, allowed: false });
  expect(JSON.stringify(existing)).toBe(before);
});

test('malformed existing script does not become a misleading zero speech count', () => {
  const invalid = { ...newScene('入力途中'), script: '入力中の本文' };
  expect(markdownAppendSummary([newScene('通常'), invalid], markdownScenesToForm(scenes)))
    .toEqual({ scenes: 3, speeches: null, invalidScene: 2, allowed: false });
});
