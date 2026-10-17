import { expect, test } from 'vitest';
import type { AudioAnalysis, AudioProbe, Clip } from '@sungeki/api-client';
import { materialWaveformPath, materialWaveformRange } from './material-waveform';
const probe = { frames: 16, sample_rate: 8, channels: 1 } as AudioProbe;
const clip = { id: 'clip', track_id: 'track', scene_id: null, transform: { x: 0, y: 0, width: 1, height: 1, opacity: 1 }, start_us: 0, duration_us: 1000000, content: { type: 'audio', asset_id: 'a', playback: {
  source_start_us: 250000, source_duration_us: 500000, looped: true,
  gain_db: -12, volume_points: [], fade_in_us: 100000, ducking: null, fade_out_us: 100000,
} } } as Clip;
function analysis(first: number, count: number): AudioAnalysis {
  return { start_bucket: first, peaks: Array.from({length: count}, (_,i) => [(first+i)/10, (first+i)/10]) } as AudioAnalysis;
}
test('loop waveform starts at selected sample and repeats independently of view offset', () => {
  const r = materialWaveformRange(clip,probe,0,1)!;
  expect([r.first,r.end,r.period,r.base,r.level,r.count]).toEqual([2,6,4,1,0,4]);
  const full = materialWaveformPath(analysis(2,4),r).split('M').slice(1);
  expect(full).toHaveLength(256);
  expect(full[0].split(',')[1]).toBe('8.20V8.20');
  for(let i=0;i<128;i++) expect(full[i].split(',')[1]).toBe(full[i+128].split(',')[1]);
  const shifted=materialWaveformRange(clip,probe,.25,.5)!;
  expect(materialWaveformPath(analysis(2,4),shifted).split('M')[1]).toContain(',6.40V6.40');
});
test('non-looping source is not stretched and invisible clips fetch nothing', () => {
  const nonloop=structuredClone(clip);
  if(nonloop.content.type!=='audio')throw Error();nonloop.content.playback.looped=false;
  const range=materialWaveformRange(nonloop,probe,0,1)!;
  expect(range.fraction).toBe(.5);
  expect(materialWaveformRange(nonloop,probe,.75,.2)).toBeNull();
  expect(materialWaveformRange(clip,probe,2,1)).toBeNull();
});
test('tiny periods on long timelines stay bounded and show the envelope of all loop samples', () => {
  const long={...clip,duration_us:604800000000};
  const r=materialWaveformRange(long,probe,0,604800)!;
  const path=materialWaveformPath(analysis(2,4),r);
  expect(path.split('M')).toHaveLength(257);
  expect(path).toContain(',5.50V8.20');
  const large=materialWaveformRange(clip,{...probe,frames:64000000},0,1)!;
  expect(Math.ceil(64000000/large.base)).toBeLessThanOrEqual(262144);
  expect(large.count).toBeLessThanOrEqual(258);
});
test('sample boundary rounding matches the renderer, not rounded microsecond loop durations', () => {
  const fractional=structuredClone(clip);
  if(fractional.content.type!=='audio')throw Error();
  fractional.content.playback.source_start_us=1;
  fractional.content.playback.source_duration_us=100;
  const r=materialWaveformRange(fractional,{...probe,sample_rate:44100,frames:44100},0,1)!;
  expect([r.first,r.end,r.period]).toEqual([1,5,4]);
});

test('two-hour material selects a bounded base resolution across full and zoomed views', () => {
  const long = structuredClone(clip);
  if (long.content.type !== 'audio') throw Error();
  long.duration_us = 7200e6;
  long.content.playback.source_start_us = 0;
  long.content.playback.source_duration_us = 7200e6;
  long.content.playback.looped = false;
  const p = {...probe, sample_rate: 48000, frames: 7200 * 48000};
  for (const [start, span] of [[0, 7200], [7199, 1], [10, .01]]) {
    const range = materialWaveformRange(long, p, start, span)!;
    expect(Math.ceil(p.frames / range.base)).toBeLessThanOrEqual(262144);
    expect(range.base).toBeLessThanOrEqual(65536);
    expect(range.count).toBeLessThanOrEqual(258);
    expect(range.count).toBeGreaterThan(0);
    expect(range.end).toBe(p.frames);
  }
});
