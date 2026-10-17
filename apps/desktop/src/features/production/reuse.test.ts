import { render } from 'svelte/server';
import BuildReusePicker from './BuildReusePicker.svelte';
import { expect, test, vi } from 'vitest';
import type { BuildRecipe, BuildReuseSource } from '@sungeki/api-client';
import { BuildReuseSelection, matchingAudio } from './reuse.svelte';
import fixture from '../../../../../tests/fixtures/recipes/build-dialogue.json';
const recipe = fixture as unknown as BuildRecipe;
const line = recipe.scenes[0]!.lines[0]!;
const role = recipe.roles.find(r => r.role === line.role)!;
const source: BuildReuseSource = { base_url: 'http://127.0.0.1:50021', engine_version: 'test', job_id: 'old', title: '元の制作', lines: [{ index: 0,
  character_id: role.character.id, character_name: role.character.name,
  subtitle_text: line.subtitle_text, spoken_text: line.spoken_text, has_saved_audio: true }] };

test('selection is explicit and copied into the public reuse request', async () => {
  const session = vi.fn().mockResolvedValue({ type: 'build_reuse_source', source });
  const selection = new BuildReuseSelection({ session }); selection.sync(recipe);
  await selection.load('old'); expect(selection.request(recipe)).toBeUndefined();
  selection.choose(recipe, 0, '0');
  const request = selection.request(recipe)!;
  expect(request).toEqual({ parent_job_id: 'old', speech: { '0': 0 } });
  request.speech['0'] = 99; expect(selection.choices['0']).toBe(0);
  expect(session).toHaveBeenCalledExactlyOnceWith({ type: 'build_video', request: { type: 'read_reuse_source', job_id: 'old' } });
});

test('script/settings edits invalidate prior selections before a new start', async () => {
  const selection = new BuildReuseSelection({ session: vi.fn().mockResolvedValue({ type: 'build_reuse_source', source }) });
  await selection.load('old'); selection.choose(recipe, 0, '0');
  const changed = { ...recipe, title: '変更後' };
  expect(() => selection.request(changed)).toThrow();
  selection.sync(changed); expect(selection.choices).toEqual({}); expect(selection.changed).toBe(true);
  selection.choose(changed, 0, '0'); expect(selection.changed).toBe(false);
});

test('missing saved record, different speaker or pronunciation cannot be selected', () => {
  for (const change of [{ has_saved_audio: false }, { character_id: 'other' }, { spoken_text: '別の読み' }, { subtitle_text: '別の字幕' }]) {
    expect(matchingAudio(recipe, 0, { ...source, lines: [{ ...source.lines[0]!, ...change }] })).toEqual([]);
  }
});

test('late source response never replaces a newer selection', async () => {
  let resolve!: (value: unknown) => void;
  const session = vi.fn().mockImplementationOnce(() => new Promise(r => { resolve = r; }))
    .mockResolvedValueOnce({ type: 'build_reuse_source', source: { ...source, job_id: 'new' } });
  const selection = new BuildReuseSelection({ session }); const first = selection.load('old');
  await selection.load('new'); resolve({ type: 'build_reuse_source', source }); await first;
  expect(selection.source?.job_id).toBe('new');
});

test('picker shows script and matching saved audio without automatically selecting it', async () => {
  const selection = new BuildReuseSelection({ session: vi.fn().mockResolvedValue({ type: 'build_reuse_source', source }) });
  selection.sync(recipe); await selection.load('old');
  const html = render(BuildReusePicker, { props: { selection, recipe, history: [], disabled: false } }).body;
  expect(html).toContain(line.subtitle_text);
  expect(html).toContain('このセリフを合成する');
  expect(html).toContain('元のセリフ');
  expect(selection.request(recipe)).toBeUndefined();
});
