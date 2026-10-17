import { expect, test, vi } from 'vitest';
import type { Sbv2ProfileDetails, SessionResponse } from '@sungeki/api-client';
import { Sbv2CharacterDraft, sbv2Character } from './sbv2-character.svelte';
const details: Sbv2ProfileDetails = {
  profile_id: 'local-jp', name: '日本語の声',
  identity: { engine_revision: 'engine', model_sha256: 'a'.repeat(64), config_sha256: 'b'.repeat(64), style_vectors_sha256: 'c'.repeat(64), text_encoder_fingerprint: 'd'.repeat(64) },
  speakers: [{ id: 0, name: '声' }, { id: 4, name: '別の声' }], styles: ['Neutral', 'Happy'], languages: ['JP'],
  default_settings: { model_id: 2, speaker_id: 0, language: 'JP', style: 'Neutral', style_weight: 1, length: 1, sdp_ratio: .2, noise: .6, noisew: .8, auto_split: true, split_interval: .5 },
};
const response = (value = details): SessionResponse => ({ type: 'sbv2_profile', details: structuredClone(value) });
test('character retains measured identity, explicit selection and credit with light/dark subtitle contrast', () => {
  const before = structuredClone(details);
  const character = sbv2Character(details, 4, 'Happy', 'JP', 'ナレーター', '作者のクレジット', '#ffffff');
  expect(character.voice).toMatchObject({ type: 'sbv2', identity: details.identity, settings: { model_id: 2, speaker_id: 4, style: 'Happy', language: 'JP', length: 1 } });
  expect(character.credit).toBe('作者のクレジット'); expect(character.subtitle_style).toEqual({ font_size: 64, color: '#000000', layout: { alignment: 'left', wrap: true }, outline: { width: 3, color: '#ffffff' } });
  expect(sbv2Character(details, 0, 'Neutral', 'JP', '声', '', '#000000').subtitle_style.color).toBe('#ffffff');
  expect(details).toEqual(before);
  if (character.voice.type !== 'sbv2') throw new Error('fixture');
  character.voice.identity.model_sha256 = 'e'.repeat(64);
  expect(details.identity.model_sha256).toBe(before.identity.model_sha256);
  for (const args of [[99, 'Neutral', 'JP', '声'], [0, 'Missing', 'JP', '声'], [0, 'Neutral', 'EN', '声'], [0, 'Neutral', 'JP', ' ']] as const) {
    expect(() => sbv2Character(details, args[0], args[1], args[2], args[3], '', '#ffffff')).toThrow();
  }
});
test('registration remeasures profile and returns only command input, without editing or synthesizing', async () => {
  const session = vi.fn().mockResolvedValue(response());
  const draft = new Sbv2CharacterDraft({ session });
  draft.select('local-jp'); await draft.inspect();
  expect(draft.speaker).toBe(0); expect(draft.style).toBe('Neutral');
  draft.speaker = 4; draft.style = 'Happy';
  const character = await draft.character('声', 'credit', '#ffffff');
  expect(character?.voice).toMatchObject({ type: 'sbv2', settings: { speaker_id: 4, style: 'Happy' } });
  expect(session).toHaveBeenCalledTimes(2);
  for (const [request] of session.mock.calls) expect(request).toEqual({ type: 'speech_job', request: { type: 'get_sbv2_profile', args: { profile_id: 'local-jp' } } });
  expect(draft.error).toBe('');
});
test('changed model or selection metadata invalidates registration until reinspection', async () => {
  const changed = structuredClone(details); changed.identity.model_sha256 = 'f'.repeat(64);
  const session = vi.fn().mockResolvedValueOnce(response()).mockResolvedValueOnce(response(changed));
  const draft = new Sbv2CharacterDraft({ session }); draft.select('local-jp'); await draft.inspect();
  expect(await draft.character('声', '', '#ffffff')).toBeNull();
  expect(draft.details).toBeNull(); expect(draft.error).toContain('変わりました');
  expect(await draft.character('声', '', '#ffffff')).toBeNull();
  expect(session).toHaveBeenCalledTimes(2);
});
test('changing profile or disposing drops late inspection and registration results', async () => {
  let resolve!: (r: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const draft = new Sbv2CharacterDraft({ session }); draft.select('local-jp');
  const pending = draft.inspect(); draft.select('other'); resolve(response()); await pending;
  expect(draft.details).toBeNull(); expect(draft.busy).toBe(false);
  session.mockResolvedValueOnce(response()); draft.select('local-jp'); await draft.inspect();
  const registration = draft.character('声', '', '#ffffff'); draft.dispose(); resolve(response());
  expect(await registration).toBeNull();
});
test('wrong profile response and missing originals do not produce registration input', async () => {
  const wrong = { ...details, profile_id: 'other' };
  const session = vi.fn().mockResolvedValueOnce(response(wrong)).mockRejectedValueOnce({ message: 'モデル原本がありません' });
  const draft = new Sbv2CharacterDraft({ session }); draft.select('local-jp'); await draft.inspect();
  expect(draft.details).toBeNull(); expect(draft.error).toContain('一致しません');
  await draft.inspect(); expect(draft.error).toContain('原本がありません');
  expect(await draft.character('声', '', '#ffffff')).toBeNull();
});
