import { expect, test } from 'vitest';
import type { AudioAnalysis, AudioProbe, Clip, AnalyzeAudioArgs } from '@sungeki/api-client';
import { waveformRange, waveformPath, validateWaveform, queueWaveform } from './waveform-view';
const clip = { start_us: 10_000_000, duration_us: 10_000_000 } as Clip;
const probe = { frames: 120000, sample_rate: 24000, channels: 1, sha256: 'hash' } as AudioProbe;
test('visible source range is bounded and short speech does not stretch to the clip end', () => {
  const full = waveformRange(clip, probe, 0, 30)!;
  expect(full.from).toBe(0); expect(full.to).toBe(120000); expect(full.fraction).toBe(.5);
  const partial = waveformRange(clip, probe, 12, 10)!;
  expect(partial.from).toBe(48000); expect(partial.to).toBe(120000); expect(partial.fraction).toBe(3/8);
  expect(partial.count).toBeLessThanOrEqual(258);
  expect(waveformRange(clip, probe, 16, 3)).toBeNull();
  expect(waveformRange(clip, probe, 0, 2)).toBeNull();
  const long = waveformRange({ ...clip, start_us: 0, duration_us: 7_000_000_000 },
    { ...probe, frames: 33_000_000 }, 0, 7000)!;
  expect(long.count).toBeLessThanOrEqual(258); expect(long.level).toBeGreaterThan(0);
});
function fixture() {
  const args: AnalyzeAudioArgs = { session_id: 's', expected_revision: 2, asset_id: 'a', base_frames: 256, level: 0, channel: 0, start_bucket: 0, count: 2 };
  const result = { session_id: 's', source_revision: 2, asset_id: 'a', source_sha256: 'hash', version: 1,
    signal: 'source_pre_gain', sample_rate: 24000, channels: 1, source_frames: 120000,
    level: 0, channel: 0, start_bucket: 0, start_frame: 0, end_frame: 512, peaks: [[-.5,.25],[0,0]] } as AudioAnalysis;
  return {args,result};
}
test('different identity, revision, samples or window cannot be displayed as the current waveform', () => {
  const {args,result} = fixture(); expect(() => validateWaveform(result,args,probe)).not.toThrow();
  for (const changes of [{ source_sha256: 'different' }, { source_revision: 3 }, { channel: 1 },
    { end_frame: 510 }, { peaks: [[1,-1],[0,0]] }, { peaks: [[NaN,1],[0,0]] }]) {
    expect(() => validateWaveform({ ...result, ...changes } as AudioAnalysis,args,probe)).toThrow();
  }
});
test('path preserves signed amplitude and clamps overrange only for drawing', () => {
  const {result} = fixture(); const range = { from: 128, to: 512, width: 256, level: 0, first: 0, count: 2, fraction: 1 };
  expect(waveformPath(result,range)).toBe('M166.67,7.75V14.50M666.67,10.00V10.00');
  result.peaks[0] = [-2,2]; expect(waveformPath(result,range)).toContain('1.00V19.00');
});
test('requests are serialized and a rejected request does not strand the queue', async () => {
  const calls: string[] = []; let release!: () => void;
  const first = queueWaveform(async () => { calls.push('first'); await new Promise<void>(r => { release = r; }); throw Error('failed'); });
  const caught = first.catch(() => {});
  const second = queueWaveform(async () => { calls.push('second'); });
  await Promise.resolve(); expect(calls).toEqual(['first']); release(); await caught; await second;
  expect(calls).toEqual(['first','second']);
});
