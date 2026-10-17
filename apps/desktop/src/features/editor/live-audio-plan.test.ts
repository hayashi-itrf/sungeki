import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import { cueGain, cueOffset, liveAudioPlan } from './live-audio-plan';
function project() {
  const p = structuredClone(fixture.project) as Project;
  p.clips = [p.clips[0]];
  p.clips[0].start_us = 2e6; p.clips[0].duration_us = 10e6;
  p.clips[0].content = { type: 'audio', asset_id: 'a', playback: {
    source_start_us: 1e6, source_duration_us: 3e6, looped: true, gain_db: -6,
    volume_points: [{ at_us: 0, level: 1 }, { at_us: 10e6, level: .5 }],
    fade_in_us: 1e6, fade_out_us: 1e6, ducking: null,
  } };
  p.tracks.forEach(t => t.muted = false); return p;
}
test('selection loops within the selected source and gain/fades/points remain relative to clip', () => {
  const { cues } = liveAudioPlan(project()), c = cues[0];
  expect(cueOffset(c, 2)).toBe(1); expect(cueOffset(c, 6)).toBe(2);
  expect(cueGain(c, 2, cues)).toBe(0);
  expect(cueGain(c, 7, cues)).toBeCloseTo(10 ** (-6 / 20) * .75);
  expect(cueGain(c, 12, cues)).toBe(0);
});
test('muted tracks have no voices and missing synthesis is reported', () => {
  const p = project(); p.tracks.forEach(t => t.muted = true);
  expect(liveAudioPlan(p).cues).toEqual([]);
  p.tracks.forEach(t => t.muted = false);
  p.clips[0].content = { type: 'speech', character_id: 'c', subtitle_text: 'test', spoken_text: null, audio: null, gain_db: null };
  expect(liveAudioPlan(p)).toEqual({ cues: [], missing: 1 });
});
test('video stream selection joins playback; explicit muted video does not', () => {
  const p = project(); p.clips[0].content = { type: 'video', asset_id: 'v', playback: {
    source_start_us: 1e6, source_duration_us: 3e6, looped: true, gain_db: -3, audio_stream: 2 } };
  expect(liveAudioPlan(p).cues).toHaveLength(1);
  p.clips[0].content.playback.audio_stream = null;
  expect(liveAudioPlan(p).cues).toHaveLength(0);
});
