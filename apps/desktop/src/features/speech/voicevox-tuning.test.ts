import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, SessionSnapshot } from '@sungeki/api-client';
import { EditorController } from '../editor/controller.svelte';
import { captureVoiceTuning } from './voice-tuning-draft.svelte';
import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Character, VoicevoxSettings } from '@sungeki/api-client';
import { tuneVoicevox, voicevoxTuningFields } from './voicevox-tuning';
import VoicevoxTuningFields from './VoicevoxTuningFields.svelte';
const settings: VoicevoxSettings = { style_id: 42, speed_scale: 1, pitch_scale: 0, intonation_scale: 1, volume_scale: 1,
  pre_phoneme_length: .1, post_phoneme_length: .1, pause_length: null, pause_length_scale: 1,
  output_sampling_rate: 48000, output_stereo: true, enable_interrogative_upspeak: true };
const character: Character = { id: 'speaker', name: 'ナレーター', credit: 'VOICEVOX:話者', subtitle_style: { font_size: 64, color: '#000000', outline: null, layout: null },
  voice: { type: 'voicevox', engine_id: 'engine', core_version: 'core', settings } };
test('VOICEVOX tuning preserves engine/core/style/output format and non-voice data', () => {
  const input = { ...settings, speed_scale: 1.2, pitch_scale: -.05, pause_length: 0, style_id: 99, output_sampling_rate: 1, output_stereo: false };
  const before = structuredClone(character), command = tuneVoicevox(character, input);
  expect(command).toEqual({ type: 'set_character', character: { ...before, voice: { ...before.voice,
    settings: { ...settings, speed_scale: 1.2, pitch_scale: -.05, pause_length: 0 } } } });
  input.speed_scale = 2;
  expect(character).toEqual(before);
  expect(command).toMatchObject({ character: { voice: { settings: { speed_scale: 1.2 } } } });
});
test('invalid numbers, zero speed and invalid optional pause never reach a review plan', () => {
  for (const field of voicevoxTuningFields) {
    for (const value of [NaN, Infinity, -Infinity, undefined]) expect(() => tuneVoicevox(character, { ...settings, [field.key]: value })).toThrow();
    if (field.min !== undefined) expect(() => tuneVoicevox(character, { ...settings, [field.key]: -.1 })).toThrow();
  }
  expect(() => tuneVoicevox(character, { ...settings, speed_scale: 0 })).toThrow();
  for (const pause_length of [NaN, Infinity, -1, undefined]) expect(() => tuneVoicevox(character, { ...settings, pause_length } as VoicevoxSettings)).toThrow();
  expect(() => tuneVoicevox(character, { ...settings, enable_interrogative_upspeak: undefined } as unknown as VoicevoxSettings)).toThrow();
  expect(() => tuneVoicevox({ ...character, voice: { type: 'sbv2' } } as Character, settings)).toThrow();
});
test('engine default pauses remain different from explicit zero and settings can be reset', () => {
  for (const pause_length of [null, 0, .25]) {
    expect(tuneVoicevox(character, { ...settings, pause_length, enable_interrogative_upspeak: false })).toMatchObject({ character: { voice: { settings: { pause_length, enable_interrogative_upspeak: false } } } });
  }
  expect(tuneVoicevox(character, settings)).toEqual({ type: 'set_character', character });
});
test('rendered controls distinguish speed and optional silence without implying no regeneration', () => {
  const normal = render(VoicevoxTuningFields, { props: { tuning: { ...settings } } }).body;
  expect(normal).toContain('大きいほど速い');
  expect(normal).toContain('疑問文の語尾');
  expect(normal.match(/type="number"/g)).toHaveLength(7);
  const explicit = render(VoicevoxTuningFields, { props: { tuning: { ...settings, pause_length: 0 } } }).body;
  expect(explicit.match(/type="number"/g)).toHaveLength(8);
});

test('native editor proxy state is captured before cloning a voice or reviewing edits', () => {
  const editor = new EditorController({} as HostApi);
  editor.snapshot = { session_id: 'session', revision: 1, project: { characters: [structuredClone(character)] } } as SessionSnapshot;
  // The SSR test compiler does not create browser proxies; inject the same
  // structuredClone-incompatible boundary explicitly. Native QA covers Svelte.
  editor.snapshot.project.characters[0]!.voice = new Proxy(structuredClone(character.voice), {});
  expect(() => structuredClone(editor.snapshot!.project.characters[0]!.voice)).toThrow();
  const observed = captureVoiceTuning(editor.snapshot)!;
  const selected = observed.project.characters[0]!;
  expect(() => structuredClone(selected.voice)).not.toThrow();
  expect(() => tuneVoicevox(selected, { ...settings, speed_scale: 1.2 })).not.toThrow();
  editor.snapshot.project.characters[0]!.name = 'changed elsewhere';
  expect(selected.name).toBe('ナレーター');
  expect(captureVoiceTuning(null)).toBeNull();
});


test('display locale changes errors without changing the voice command or Japanese content', () => {
  const before = structuredClone(character);
  const originalLocale = locale.current;
  try {
    locale.current = 'ja';
    const japanese = tuneVoicevox(character, settings);
    locale.current = 'en';
    expect(tuneVoicevox(character, settings)).toEqual(japanese);
    expect(() => tuneVoicevox(character, { ...settings, speed_scale: 0 })).toThrow('Check the value');
    expect(character).toEqual(before);
  } finally { locale.current = originalLocale; }
});
