import { locale } from '../../shared/i18n/locale.svelte';
afterEach(() => { locale.current = 'ja'; });
import { afterEach, expect, test, vi } from 'vitest';
import type { BuildRecipe, Voice } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/recipes/build-dialogue.json';
import { buildProfiles, buildConnection, type BuildProfile } from './voice-settings';
function recipe() { return structuredClone(fixture) as unknown as BuildRecipe; }
const irodori: Voice = { type: 'irodori', settings: { voice_id: 'none', caption: null, cfg_scale: 3, num_steps: 40, seed: 42 },
  identity: { checkpoint: 'test', model_manifest_sha256: 'a'.repeat(64), codec_manifest_sha256: 'b'.repeat(64),
    engine_manifest_sha256: 'c'.repeat(64), voice_manifest_sha256: 'd'.repeat(64), sample_rate: 24000, channels: 1 } };
test('mixed recipe binds profiles by character and excludes unused narration participants', () => {
  const r = recipe(); r.roles[0]!.character.voice = irodori;
  const people = r.roles.map(role => ({ character: role.character.id, profile: { engine: 'irodori', id: 'local' } as BuildProfile }));
  const before = JSON.stringify(r);
  expect(buildProfiles(r, people)).toEqual({ [people[0]!.character]: 'local' });
  expect(JSON.stringify(r)).toBe(before);
  r.roles = [r.roles[1]!];
  expect(buildProfiles(r, people)).toEqual({});
});
test('missing profile or a selection from a different engine is rejected', () => {
  const r = recipe(); r.roles[0]!.character.voice = irodori;
  for (const profile of [undefined, { engine: 'qwen', id: 'same-id' }, { engine: 'irodori', id: '' }, { engine: 'irodori', id: '../invalid' }] as (BuildProfile | undefined)[]) {
    expect(() => buildProfiles(r, [{ character: r.roles[0]!.character.id, profile }])).toThrow('起動設定');
  }
});
test('managed-only recipe never discovers VOICEVOX or transmits its URL', async () => {
  const r = recipe(); for (const role of r.roles) role.character.voice = irodori;
  const session = vi.fn();
  expect(await buildConnection({ session }, r, 'http://unused:50021')).toEqual({ base_url: '', engine_version: '' });
  expect(session).not.toHaveBeenCalled();
});
test('mixed recipe discovers and validates only the VOICEVOX participants', async () => {
  const r = recipe(); r.roles[0]!.character.voice = irodori;
  const voice = r.roles[1]!.character.voice;
  if (voice.type !== 'voicevox') throw new Error('fixture');
  const session = vi.fn().mockResolvedValue({ type: 'voicevox_engine', engine: {
    engine_id: voice.engine_id, core_versions: [voice.core_version], version: 'test' } });
  expect(await buildConnection({ session }, r, 'http://localhost:50021')).toEqual({ base_url: 'http://localhost:50021', engine_version: 'test' });
  voice.engine_id = 'different';
  await expect(buildConnection({ session }, r, 'http://localhost:50021')).rejects.toThrow('一致');
});
test('English voice setup errors preserve literal speaker names and reject mismatched discovery', async () => {
  locale.current = 'en';
  const r = recipe(); r.roles[0]!.character.voice = irodori;
  r.roles[0]!.character.name = 'Name $& $`';
  expect(() => buildProfiles(r, [])).toThrow('Select a launch profile for Name $& $`.');
  const session = vi.fn().mockResolvedValue({ type: 'sessions', session_ids: [] });
  await expect(buildConnection({ session }, r, 'http://localhost:50021')).rejects.toThrow('Could not verify VOICEVOX.');
  session.mockResolvedValue({ type: 'voicevox_engine', engine: { engine_id: 'wrong', core_versions: [], version: 'test' } });
  await expect(buildConnection({ session }, r, 'http://localhost:50021')).rejects.toThrow("speaker's VOICEVOX settings do not match");
});

test('complete explicit reuse uses recorded identity without discovering a stopped engine', async () => {
  const r = recipe();
  const lines = r.scenes.flatMap(scene => scene.lines);
  const source = { job_id: 'source', title: 'Saved', base_url: 'http://127.0.0.1:55021', engine_version: 'recorded',
    lines: lines.map((line, index) => {
      const character = r.roles.find(role => role.role === line.role)!.character;
      return { index, character_id: character.id, character_name: character.name, subtitle_text: line.subtitle_text,
        spoken_text: line.spoken_text, has_saved_audio: true };
    }) };
  const reuse = { parent_job_id: 'source', speech: Object.fromEntries(lines.map((_, index) => [String(index), index])) };
  const session = vi.fn().mockRejectedValue(new Error('engine offline'));
  expect(await buildConnection({ session }, r, 'http://unneeded:50021', reuse, source))
    .toEqual({ base_url: source.base_url, engine_version: source.engine_version });
  expect(session).not.toHaveBeenCalled();
  // Partial, unrelated, missing or changed audio must not skip discovery.
  const partial = structuredClone(reuse); delete partial.speech['0'];
  for (const selected of [undefined, partial, { ...reuse, parent_job_id: 'different' }]) {
    await expect(buildConnection({ session }, r, '', selected, source)).rejects.toThrow('engine offline');
  }
  for (const changed of [
    { ...source, engine_version: '' },
    { ...source, lines: source.lines.map(line => ({ ...line, has_saved_audio: false })) },
    { ...source, lines: source.lines.map(line => ({ ...line, spoken_text: 'changed' })) },
  ]) await expect(buildConnection({ session }, r, '', reuse, changed)).rejects.toThrow('engine offline');
});
