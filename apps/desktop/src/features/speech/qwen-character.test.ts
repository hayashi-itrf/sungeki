import { qwenLanguageLabel } from './qwen-language';
import { expect, test, vi } from 'vitest';
import type { QwenProfileDetails, SessionResponse } from '@sungeki/api-client';
import { QwenCharacterDraft, qwenCharacter } from './qwen-character.svelte';
import { QwenProfiles } from './qwen-profiles.svelte';
const details: QwenProfileDetails = {
  profile_id: 'local', name: 'Qwen',
  identity: { backend: 'official', model_id: '/ssd/model', model_manifest_sha256: 'a'.repeat(64), engine_manifest_sha256: 'b'.repeat(64), voice_manifest_sha256: 'c'.repeat(64), sample_rate: 24000, channels: 1 },
  speakers: [{ id: 'Vivian', name: 'Vivian' }, { id: 'Ryan', name: 'Ryan' }], languages: ['Japanese', 'English'],
  default_settings: { model: 'qwen3-tts', voice: 'Vivian', language: 'Japanese', speed: 1, instruct: null },
};
const response = (d = details): SessionResponse => ({ type: 'qwen_profile', details: structuredClone(d) });
test('captures selected Qwen voice and identity with subtitle contrast', () => {
  const character = qwenCharacter(details, 'Ryan', 'Japanese', '語り手', '作者', '#ffffff');
  expect(character.voice).toMatchObject({ type: 'qwen', identity: details.identity, settings: { voice: 'Ryan', speed: 1 } });
  expect(character.credit).toBe('作者'); expect(character.subtitle_style).toEqual({ font_size: 64, color: '#000000', layout: { alignment: 'left', wrap: true }, outline: { width: 3, color: '#ffffff' } });
  expect(() => qwenCharacter(details, 'missing', 'Japanese', '語り手', '', '#ffffff')).toThrow();
  expect(() => qwenCharacter(details, 'Ryan', 'unknown', '語り手', '', '#ffffff')).toThrow();
  if (character.voice.type === 'qwen') character.voice.identity.model_id = 'changed';
  expect(details.identity.model_id).toBe('/ssd/model');
});
test('registration rechecks originals and rejects changed metadata', async () => {
  const session = vi.fn().mockResolvedValue(response());
  const draft = new QwenCharacterDraft({ session }); draft.select('local'); await draft.inspect();
  draft.speaker = 'Ryan';
  expect((await draft.character('語り手', '作者', '#fff'))?.voice).toMatchObject({ type: 'qwen', settings: { voice: 'Ryan' } });
  expect(session).toHaveBeenCalledTimes(2);
  const changed = structuredClone(details); changed.identity.voice_manifest_sha256 = 'd'.repeat(64);
  session.mockResolvedValueOnce(response(changed));
  expect(await draft.character('語り手', '', '#ffffff')).toBeNull();
  expect(draft.details).toBeNull(); expect(draft.error).toContain('変わりました');
  expect(session.mock.calls.every(([r]) => r.request.type === 'get_qwen_profile')).toBe(true);
});
test('profile switching and disposal discard delayed results', async () => {
  let resolve!: (value: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const draft = new QwenCharacterDraft({ session }); draft.select('local');
  const first = draft.inspect(); draft.select('other'); resolve(response()); await first;
  expect(draft.details).toBeNull(); expect(draft.busy).toBe(false);
  session.mockResolvedValueOnce(response()); draft.select('local'); await draft.inspect();
  const next = draft.character('語り手', '', '#ffffff'); draft.dispose(); resolve(response());
  expect(await next).toBeNull();
});
test('profile list auto-selects only one choice and clears stale selection on errors', async () => {
  const session = vi.fn().mockResolvedValue({ type: 'qwen_profiles', profiles: [{ id: 'local', name: 'Qwen' }] });
  const profiles = new QwenProfiles({ session }); await profiles.refresh();
  expect(profiles.ready).toBe(true); expect(profiles.selectedId).toBe('local');
  session.mockRejectedValueOnce(new Error('設定なし')); await profiles.refresh();
  expect(profiles.ready).toBe(false); expect(profiles.selectedId).toBe('');
});

test('lowercase model language values remain exact in captured settings', () => {
  const actual = structuredClone(details);
  actual.languages = ['auto', 'japanese']; actual.default_settings.language = 'japanese';
  const character = qwenCharacter(actual, 'Vivian', 'japanese', '語り手', '', '#ffffff');
  expect(character.voice).toMatchObject({ type: 'qwen', settings: { language: 'japanese' } });
});

test('language labels handle both model and fallback casing without changing unknown names', () => {
  expect(qwenLanguageLabel('japanese')).toBe('日本語');
  expect(qwenLanguageLabel('Japanese')).toBe('日本語');
  expect(qwenLanguageLabel('auto')).toBe('自動判定');
  expect(qwenLanguageLabel('NewLanguage')).toBe('NewLanguage');
});
