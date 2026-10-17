import { locale } from '../../shared/i18n/locale.svelte';
import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Character, IrodoriSettings } from '@sungeki/api-client';
import { tuneIrodori } from './irodori-tuning';
import IrodoriTuningFields from './IrodoriTuningFields.svelte';
const settings: IrodoriSettings = { voice_id: 'registered', caption: null, cfg_scale: 3, num_steps: 40, seed: 42 };
const character: Character = {
  id: 'narrator', name: '語り手', credit: '作者', subtitle_style: { font_size: 64, color: '#000000', outline: null, layout: null },
  voice: { type: 'irodori', settings, identity: { checkpoint: 'model', model_manifest_sha256: 'a'.repeat(64),
    codec_manifest_sha256: 'b'.repeat(64), engine_manifest_sha256: 'c'.repeat(64), voice_manifest_sha256: 'd'.repeat(64), sample_rate: 48000, channels: 1 } },
};
test('tuning preserves selected voice and identity without modifying current character', () => {
  const before = structuredClone(character);
  const input = { ...settings, voice_id: 'other', caption: '穏やか', seed: 99 };
  const command = tuneIrodori(character, input);
  input.caption = 'changed';
  expect(character).toEqual(before);
  expect(command).toMatchObject({ type: 'set_character', character: { ...before,
    voice: { ...before.voice, settings: { ...settings, caption: '穏やか', seed: 99 } } } });
});
test('rejects empty, non-finite, fractional integer and out-of-range values before review', () => {
  for (const key of ['cfg_scale', 'num_steps', 'seed'] as const) {
    for (const value of [undefined, NaN, Infinity, -Infinity, -1])
      expect(() => tuneIrodori(character, { ...settings, [key]: value })).toThrow();
  }
  for (const bad of [{ cfg_scale: 31 }, { num_steps: 0 }, { num_steps: 201 }, { num_steps: 1.5 }, { seed: 4294967296 }, { seed: .5 }])
    expect(() => tuneIrodori(character, { ...settings, ...bad })).toThrow();
  for (const caption of ['', '  ', '\0', 'あ'.repeat(1366)])
    expect(() => tuneIrodori(character, { ...settings, caption })).toThrow();
  expect(() => tuneIrodori(character, { ...settings, cfg_scale: 0, num_steps: 1, seed: 0 })).not.toThrow();
  expect(() => tuneIrodori(character, { ...settings, cfg_scale: 30, num_steps: 200, seed: 4294967295, caption: 'あ'.repeat(1365) })).not.toThrow();
});
test('controls explain inherited caption and expose integer bounds', () => {
  const html = render(IrodoriTuningFields, { props: { tuning: settings } }).body;
  expect(html).toContain('空欄では登録音声の説明を使用');
  expect(html).toContain('max="4294967295"');
  expect(html).toContain('max="200"');
  expect(html.match(/type="number"/g)).toHaveLength(3);
});


test('display locale changes errors without changing the voice command or Japanese content', () => {
  const before = structuredClone(character);
  const originalLocale = locale.current;
  try {
    locale.current = 'ja';
    const japanese = tuneIrodori(character, { ...settings, caption: "日本語の説明" });
    locale.current = 'en';
    expect(tuneIrodori(character, { ...settings, caption: "日本語の説明" })).toEqual(japanese);
    expect(() => tuneIrodori(character, { ...settings, seed: -1 })).toThrow('The seed must');
    expect(character).toEqual(before);
  } finally { locale.current = originalLocale; }
});
