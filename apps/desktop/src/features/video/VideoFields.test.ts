import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { VideoPlayback, VideoProbe } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/video-clip-project.json';
import { locale } from '../../shared/i18n/locale.svelte';
import VideoFields from './VideoFields.svelte';
afterEach(() => { locale.current = 'ja'; });
const probe = () => structuredClone(fixture.assets[0]!.probe.info) as VideoProbe;
const playback = (): VideoPlayback => ({ source_start_us: 125000, source_duration_us: 750000,
  looped: true, audio_stream: 1, gain_db: -6 });
test.each([false, true])('video source settings preserve values across languages (production=%s)', production => {
  const info = probe(), input = playback();
  info.metadata.audio = [{ index: 1, codec: 'aac', channels: 2, sample_rate: 48000,
    clock: { time_base: { numerator: 1, denominator: 48000 }, start_pts: 0, duration_ts: 48000 } }];
  const original = structuredClone({ info, input });
  locale.current = 'en';
  const english = render(VideoFields, { props: { probe: info, playback: input, production } }).body;
  for (const text of ['Video source settings', 'Start within original (seconds)', 'Selected source duration (seconds)',
    'Repeat selected range', 'Original video audio', 'Source audio level (dB)', 'Hiding the video does not stop its source audio.']) expect(english).toContain(text);
  for (const value of ['0.125', '0.75', '-6']) expect(english).toContain(`value="${value}"`);
  expect(english).toContain('value="1" selected');
  expect(english).toContain(production ? 'Display duration follows' : 'Set the timeline duration');
  expect(english.includes('The editing preview is a reduced image')).toBe(!production);
  locale.current = 'ja';
  const japanese = render(VideoFields, { props: { probe: info, playback: input, production } }).body;
  expect(japanese).toContain('元の動画の音声'); expect(japanese).toContain('映像を非表示にしても元音声は再生されます');
  expect({ info, input }).toEqual(original);
});
test('unknown durations and unavailable audio remain explicit without enabling unavailable streams', () => {
  const info = probe(), input = { ...playback(), audio_stream: null };
  info.metadata.video.clock.duration_ts = null;
  locale.current = 'en';
  const silent = render(VideoFields, { props: { probe: info, playback: input } }).body;
  expect(silent).toContain('Unknown duration'); expect(silent).toContain('This video has no audio.');
  info.metadata.audio = [{ index: 2, codec: 'aac', channels: 2, sample_rate: 48000,
    clock: { time_base: { numerator: 1, denominator: 48000 }, start_pts: null, duration_ts: 48000 } }];
  const unavailable = render(VideoFields, { props: { probe: info, playback: input } }).body;
  expect(unavailable).toMatch(/<option value="2" disabled(?:="")?>Audio 2/);
  expect(unavailable).toContain('unavailable: unknown start time');
  expect(input.audio_stream).toBeNull();
});
