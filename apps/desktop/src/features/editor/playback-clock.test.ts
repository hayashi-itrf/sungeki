import { expect, test } from 'vitest';
import { PlaybackClock, playbackMatches } from './playback-clock';
import type { RenderJob, SessionSnapshot } from '@sungeki/api-client';
test('media clock does not turn published playback positions into seek requests', () => {
  const clock = new PlaybackClock();
  expect(clock.seek(2, 10)).toBe(2);
  expect(clock.sample(0.5, false)).toBeNull();
  expect(clock.seek(2, 10)).toBeNull();
  expect(clock.seeked(2)).toBe(2);
  for (const time of [2.01, 2.05, 2.9]) {
    expect(clock.sample(time, false)).toBe(time);
    expect(clock.seek(time, 10)).toBeNull();
  }
  expect(clock.seek(1, 10)).toBe(1);
  expect(clock.sample(3, false)).toBeNull();
  expect(clock.sample(1, true)).toBeNull();
  expect(clock.seeked(1)).toBe(1);
  expect(clock.sample(1.03, false)).toBe(1.03);
});
test('head and end seeks clamp, invalid clock data does not enter the editor', () => {
  const clock = new PlaybackClock();
  expect(clock.seek(Infinity, 10)).toBeNull();
  expect(clock.seek(1, NaN)).toBeNull();
  expect(clock.seek(-2, 10)).toBe(0);
  expect(clock.seeked(0)).toBe(0);
  expect(clock.seek(15, 10)).toBe(10);
  expect(clock.seeked(10)).toBe(10);
  expect(clock.sample(NaN, false)).toBeNull();
  expect(clock.sample(-1, false)).toBeNull();
});
test('only completed video from the exact current session and revision may drive editing time', () => {
  const snapshot = { session_id: 'session', revision: 4 } as SessionSnapshot;
  const job = { session_id: 'session', source_revision: 4, phase: 'succeeded', stale: false, output: {} } as RenderJob;
  expect(playbackMatches(job, snapshot, false)).toBe(true);
  for (const changed of [null, { ...job, session_id: 'other' }, { ...job, source_revision: 3 },
    { ...job, stale: true }, { ...job, output: null }, { ...job, phase: 'running' as const }]) {
    expect(playbackMatches(changed, snapshot, false)).toBe(false);
  }
  expect(playbackMatches(job, snapshot, true)).toBe(false);
  expect(playbackMatches(job, null, false)).toBe(false);
});
test('reopening observes a live preparation without adopting other sessions or replaying stale movies', async () => {
  const { recoverPlayback } = await import('./playback-clock');
  const snapshot = { session_id: 'session', revision: 4 } as SessionSnapshot;
  const ready = { job_id: 'ready', session_id: 'session', source_revision: 4, phase: 'succeeded', stale: false, output: {} } as RenderJob;
  const running = { ...ready, job_id: 'running', source_revision: 3, phase: 'running' as const, output: null, stale: true };
  expect(recoverPlayback([ready, running], snapshot)).toBe(running);
  expect(playbackMatches(running, snapshot, false)).toBe(false);
  expect(recoverPlayback([{ ...running, phase: 'failed' }, ready], snapshot)).toBe(ready);
  expect(recoverPlayback([{ ...running, session_id: 'other' }, { ...ready, stale: true }], snapshot)).toBeNull();
  expect(recoverPlayback([ready], null)).toBeNull();
  expect(recoverPlayback([{ ...running, phase: 'cancelling' }], snapshot)?.phase).toBe('cancelling');
});

test('returning to the last displayed position overrides an unfinished seek', () => {
  const clock = new PlaybackClock();
  expect(clock.sample(1, false)).toBe(1);
  expect(clock.seek(8, 10)).toBe(8);
  // Before the browser finishes seeking, the user returns to the visible position.
  expect(clock.seek(1, 10)).toBe(1);
  expect(clock.sample(8, false)).toBeNull();
  expect(clock.seek(1, 10)).toBeNull();
  expect(clock.seeked(1)).toBe(1);
  expect(clock.seek(1, 10)).toBeNull();
});
