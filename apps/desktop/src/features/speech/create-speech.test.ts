import { locale } from '../../shared/i18n/locale.svelte';
import { render } from 'svelte/server';
import SpeechFields from './SpeechFields.svelte';
import { expect, test, vi } from 'vitest';
import type { Project } from '@sungeki/api-client';
import { nextSpeechStart, speechCommands, type SpeechDraft } from './create-speech';
function project(): Project {
  return { format: 'vvm-project', schema_version: 1, id: 'project', title: 'test',
    canvas: { width: 1920, height: 1080, fps: { numerator: 30, denominator: 1 }, background: '#ffffff' },
    tracks: [], clips: [], groups: [], scenes: [], assets: [], pronunciation_dictionary: [], audio_mix: { normalization: null },
    characters: [{ id: 'character', name: '話者', credit: '検証', subtitle_style: { layout: null, outline: null, font_size: 64, color: '#000000' }, voice: {
      type: 'voicevox', engine_id: 'test', core_version: 'test', settings: { style_id: 1, speed_scale: 1, pitch_scale: 0,
        intonation_scale: 1, volume_scale: 1, pre_phoneme_length: .1, post_phoneme_length: .1, pause_length: null,
        pause_length_scale: 1, output_sampling_rate: 24000, output_stereo: false, enable_interrogative_upspeak: true },
    } }],
  };
}
const draft: SpeechDraft = { characterId: 'character', subtitle: '字幕\nABC', spoken: 'よみ', explicit: true,
  trackId: '', sceneId: '', start: 1.25, duration: 3.5 };
test('following speech uses the latest end on the selected track, excluding BGM and other tracks', () => {
  const source = project();
  const add = speechCommands(source, draft).commands.at(-1);
  if (add?.type !== 'add_clip') throw new Error('missing clip');
  source.clips = [
    { ...add.clip, track_id: 'voice', start_us: 2_910_000, duration_us: 3_125_334 },
    { ...add.clip, id: 'first', track_id: 'voice', start_us: 0, duration_us: 2_560_000 },
    { ...add.clip, id: 'other', track_id: 'other', start_us: 99_000_000 },
    { ...add.clip, id: 'bgm', track_id: 'voice', duration_us: 200_000_000,
      content: { type: 'audio', asset_id: 'music', playback: { source_start_us: 0,
        source_duration_us: 4_000_000, looped: true, gain_db: 0, volume_points: [], fade_in_us: 0, fade_out_us: 0, ducking: null } } },
  ];
  const before = structuredClone(source);
  expect(nextSpeechStart(source, 'voice')).toBe(6.385334);
  expect(nextSpeechStart(source, '')).toBeNull();
  expect(source).toEqual(before);
});
test('first speech track and clip are one batch and never change the input Project', () => {
  const source = project(), before = structuredClone(source);
  const result = speechCommands(source, draft);
  expect(result.commands.map(c => c.type)).toEqual(['add_track', 'add_clip']);
  const [track, add] = result.commands;
  if (track.type !== 'add_track' || add.type !== 'add_clip') throw new Error('missing batch');
  expect(add.clip.id).toBe(result.clipId);
  expect(add.clip.track_id).toBe(track.track.id);
  expect(track.track.kind).toBe('audio');
  expect(add.clip.start_us).toBe(1_250_000); expect(add.clip.duration_us).toBe(3_500_000);
  expect(add.clip.content).toEqual({ type: 'speech', gain_db: null, audio: null, character_id: 'character', subtitle_text: '字幕\nABC', spoken_text: 'よみ' });
  expect(source).toEqual(before);
});
test('explicit track/scene choices and reading removal are preserved without shifting anything', () => {
  const source = project();
  source.tracks.push({ id: 'audio', name: '音声', kind: 'audio', visible: false, muted: true });
  source.scenes.push({ id: 'scene', name: '導入', start_us: 0, duration_us: 5_000_000 });
  const result = speechCommands(source, { ...draft, trackId: 'audio', sceneId: 'scene', explicit: false });
  expect(result.commands).toHaveLength(1);
  const add = result.commands[0]; if (add.type !== 'add_clip') throw new Error('missing clip');
  expect(add.clip.track_id).toBe('audio'); expect(add.clip.scene_id).toBe('scene');
  expect(add.clip.content).toMatchObject({ spoken_text: null });
  expect(source.scenes[0].duration_us).toBe(5_000_000);
});
test('invalid or stale choices and unrepresentable times cannot produce an edit request', () => {
  for (const change of [{ characterId: 'missing' }, { trackId: 'missing' }, { sceneId: 'missing' },
    { start: NaN }, { duration: Infinity }, { duration: 0 }, { start: -1 }, { start: 604800, duration: .001 }]) {
    expect(() => speechCommands(project(), { ...draft, ...change })).toThrow();
  }
  const source = project(); source.tracks.push({ id: 'visual', name: '映像', kind: 'visual', visible: true, muted: false });
  expect(() => speechCommands(source, { ...draft, trackId: 'visual' })).toThrow();
  const result = speechCommands(project(), { ...draft, start: 604799.999999, duration: .000001 });
  const last = result.commands.at(-1); if (last?.type !== 'add_clip') throw new Error('missing clip');
  expect(last.clip.start_us + last.clip.duration_us).toBe(604800e6);
});

test('changing display language preserves speech commands and entered Japanese text', () => {
  const source = project(), before = structuredClone(source);
  const ids = vi.spyOn(crypto, 'randomUUID').mockReturnValue('11111111-1111-4111-8111-111111111111');
  try {
    locale.current = 'ja';
    const japanese = speechCommands(source, draft);
    locale.current = 'en';
    expect(speechCommands(source, draft)).toEqual(japanese);
    const html = render(SpeechFields, { props: { characters: source.characters, characterId: draft.characterId,
      subtitle: draft.subtitle, spoken: draft.spoken, explicit: true } }).body;
    expect(html).toContain('Specify reading text');
    expect(html).toContain('Text to speak');
    expect(html).toContain('字幕\nABC');
    expect(html).toContain('よみ');
    expect(html).toContain('話者');
    expect(() => speechCommands(source, { ...draft, characterId: 'gone' })).toThrow('Select a registered character.');
    expect(source).toEqual(before);
  } finally { locale.current = 'ja'; ids.mockRestore(); }
});
