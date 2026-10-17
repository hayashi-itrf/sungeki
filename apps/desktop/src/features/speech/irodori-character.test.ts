import { expect, test, vi } from 'vitest';
import type { IrodoriProfileDetails, SessionResponse } from '@sungeki/api-client';
import { IrodoriCharacterDraft, irodoriCharacter } from './irodori-character.svelte';
import { IrodoriProfiles } from './irodori-profiles.svelte';
const details: IrodoriProfileDetails = {
  profile_id: 'local', name: 'Irodori',
  identity: { checkpoint: 'local', model_manifest_sha256: 'a'.repeat(64), codec_manifest_sha256: 'd'.repeat(64), engine_manifest_sha256: 'b'.repeat(64), voice_manifest_sha256: 'c'.repeat(64), sample_rate: 48000, channels: 1 },
  speakers: [{ id: 'none', name: '参照なし' }, { id: 'registered', name: '登録音声' }],
  speaker_conditioning: true, caption_conditioning: true,
  default_settings: { voice_id: 'none', caption: '穏やか', cfg_scale: 3, num_steps: 40, seed: 42 },
};
const response = (d = details): SessionResponse => ({ type: 'irodori_profile', details: structuredClone(d) });
test('captures selected Irodori voice and identity with subtitle contrast', () => {
  const character = irodoriCharacter(details, 'registered', '語り手', '作者', '#ffffff');
  expect(character.voice).toMatchObject({ type: 'irodori', identity: details.identity, settings: { voice_id: 'registered', caption: '穏やか', cfg_scale: 3, num_steps: 40, seed: 42 } });
  expect(character.credit).toBe('作者'); expect(character.subtitle_style).toEqual({ font_size: 64, color: '#000000', layout: { alignment: 'left', wrap: true }, outline: { width: 3, color: '#ffffff' } });
  expect(() => irodoriCharacter(details, 'missing', '語り手', '', '#ffffff')).toThrow();
  if (character.voice.type === 'irodori') character.voice.identity.checkpoint = 'changed';
  expect(details.identity.checkpoint).toBe('local');
});
test('registration rechecks originals and rejects changed metadata', async () => {
  const session = vi.fn().mockResolvedValue(response());
  const draft = new IrodoriCharacterDraft({ session }); draft.select('local'); await draft.inspect();
  draft.speaker = 'registered';
  expect((await draft.character('語り手', '作者', '#fff'))?.voice).toMatchObject({ type: 'irodori', settings: { voice_id: 'registered' } });
  expect(session).toHaveBeenCalledTimes(2);
  const changed = structuredClone(details); changed.identity.voice_manifest_sha256 = 'd'.repeat(64);
  session.mockResolvedValueOnce(response(changed));
  expect(await draft.character('語り手', '', '#ffffff')).toBeNull();
  expect(draft.details).toBeNull(); expect(draft.error).toContain('変わりました');
  expect(session.mock.calls.every(([r]) => r.request.type === 'get_irodori_profile')).toBe(true);
});
test('profile switching and disposal discard delayed results', async () => {
  let resolve!: (value: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const draft = new IrodoriCharacterDraft({ session }); draft.select('local');
  const first = draft.inspect(); draft.select('other'); resolve(response()); await first;
  expect(draft.details).toBeNull(); expect(draft.busy).toBe(false);
  session.mockResolvedValueOnce(response()); draft.select('local'); await draft.inspect();
  const next = draft.character('語り手', '', '#ffffff'); draft.dispose(); resolve(response());
  expect(await next).toBeNull();
});
test('profile list auto-selects only one choice and clears stale selection on errors', async () => {
  const session = vi.fn().mockResolvedValue({ type: 'irodori_profiles', profiles: [{ id: 'local', name: 'Irodori' }] });
  const profiles = new IrodoriProfiles({ session }); await profiles.refresh();
  expect(profiles.ready).toBe(true); expect(profiles.selectedId).toBe('local');
  session.mockRejectedValueOnce(new Error('設定なし')); await profiles.refresh();
  expect(profiles.ready).toBe(false); expect(profiles.selectedId).toBe('');
});

