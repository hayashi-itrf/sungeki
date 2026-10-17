import { locale } from '../../shared/i18n/locale.svelte';
import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Character } from '@sungeki/api-client';
import { tuneSbv2, tuningFields } from './sbv2-tuning';
import Sbv2TuningFields from './Sbv2TuningFields.svelte';
const character: Character = {
  id: 'narrator', name: '語り手', credit: '制作者', subtitle_style: { font_size: 64, color: '#000000', outline: null, layout: null },
  voice: { type: 'sbv2', identity: { engine_revision: 'engine', model_sha256: 'a'.repeat(64), config_sha256: 'b'.repeat(64), style_vectors_sha256: 'c'.repeat(64), text_encoder_fingerprint: 'd'.repeat(64) },
    settings: { model_id: 2, speaker_id: 4, language: 'JP', style: 'Happy', style_weight: 1, length: 1, sdp_ratio: .2, noise: .6, noisew: .8, auto_split: true, split_interval: .5 } },
};
const settings = () => { if (character.voice.type !== 'sbv2') throw new Error('fixture'); return structuredClone(character.voice.settings); };
test('tuning creates a common edit preserving model identity, voice selection and non-voice fields', () => {
  const original = structuredClone(character), input = { ...settings(), length: 1.5, style_weight: 5, auto_split: false, model_id: 99 };
  const command = tuneSbv2(character, input);
  expect(command).toMatchObject({ type: 'set_character', character: { ...original, voice: { ...original.voice, settings: { ...settings(), length: 1.5, style_weight: 5, auto_split: false } } } });
  input.length = 2;
  expect(character).toEqual(original);
  expect(command).toMatchObject({ character: { voice: { settings: { length: 1.5, model_id: 2, speaker_id: 4, style: 'Happy' } } } });
});
test('non-finite, empty and out of range values cannot enter a reviewed command', () => {
  for (const field of tuningFields) {
    for (const value of [undefined, NaN, Infinity, -Infinity, field.min - .01, field.max + .01]) {
      expect(() => tuneSbv2(character, { ...settings(), [field.key]: value })).toThrow();
    }
    for (const value of [field.min, field.max]) expect(() => tuneSbv2(character, { ...settings(), [field.key]: value })).not.toThrow();
  }
  expect(() => tuneSbv2(character, { ...settings(), auto_split: undefined } as unknown as ReturnType<typeof settings>)).toThrow();
  expect(() => tuneSbv2({ ...character, voice: { type: 'voicevox' } } as Character, settings())).toThrow();
});
test('rendered controls explain duration direction, splitting and carry validation bounds', () => {
  const html = render(Sbv2TuningFields, { props: { tuning: settings() } }).body;
  expect(html).toContain('大きいほどゆっくり');
  expect(html).toContain('文ごとに分割');
  expect(html.match(/type="number"/g)).toHaveLength(6);
  expect(html).toContain('max="100"');
  expect(html).toContain('max="60"');
});


test('display locale changes errors without changing the voice command or Japanese content', () => {
  const before = structuredClone(character);
  const originalLocale = locale.current;
  try {
    locale.current = 'ja';
    const japanese = tuneSbv2(character, settings());
    locale.current = 'en';
    expect(tuneSbv2(character, settings())).toEqual(japanese);
    expect(() => tuneSbv2(character, { ...settings(), length: 0 })).toThrow('Set Duration');
    expect(character).toEqual(before);
  } finally { locale.current = originalLocale; }
});
