import { expect, test, vi } from 'vitest';
import type { SessionResponse } from '@sungeki/api-client';
import { RenderStill } from './render-still.svelte';
const reply = (time: number, job = 'movie'): SessionResponse => ({ type: 'render_frame', image: {
  job_id: job, time_us: time, frame_index: 0, width: 960, height: 540, png_base64: 'png',
} });
const settle = async () => { await Promise.resolve(); await Promise.resolve(); };

test('coalesces paused seeks, rejects stale pixels, and keeps only the latest request', async () => {
  const pending: ((response: SessionResponse) => void)[] = [];
  const session = vi.fn(() => new Promise<SessionResponse>(resolve => pending.push(resolve)));
  const still = new RenderStill({ session }, 'movie');
  still.load(1); still.load(2); still.load(3);
  expect(session).toHaveBeenCalledTimes(1);
  pending[0](reply(1_000_000)); await settle();
  expect(still.image).toBeNull();
  expect(session).toHaveBeenLastCalledWith({ type: 'render_job', request: { type: 'frame',
    args: { job_id: 'movie', time_us: 3_000_000 } } });
  pending[1](reply(3_000_000)); await settle();
  expect(still.image?.time_us).toBe(3_000_000); expect(still.busy).toBe(false);
  still.load(3); expect(session).toHaveBeenCalledTimes(2);
});

test('starting playback or closing the player discards outstanding replies', async () => {
  for (const close of [false, true]) {
    let resolve!: (response: SessionResponse) => void;
    const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
    const still = new RenderStill({ session }, 'movie');
    still.load(1);
    if (close) still.dispose(); else still.clear();
    resolve(reply(1_000_000)); await settle();
    expect(still.image).toBeNull(); expect(still.busy).toBe(false);
    expect(session).toHaveBeenCalledTimes(1);
    if (close) {
      // A media error can be followed by queued pause/canplay events.
      still.load(2); still.clear(); still.load(3);
      await settle();
      expect(session).toHaveBeenCalledTimes(1);
      expect(still.image).toBeNull();
    }
  }
});

test('rejects mismatched output and permits a read-only retry after error', async () => {
  const session = vi.fn().mockResolvedValueOnce(reply(0, 'other')).mockResolvedValue(reply(0));
  const still = new RenderStill({ session }, 'movie');
  still.load(0); await settle();
  expect(still.image).toBeNull(); expect(still.error).not.toBe('');
  still.load(0); await settle();
  expect(still.image?.job_id).toBe('movie'); expect(still.error).toBe('');
});

test('a new pause at the same time cannot accept a response from before playback', async () => {
  const pending: ((response: SessionResponse) => void)[] = [];
  const session = vi.fn(() => new Promise<SessionResponse>(resolve => pending.push(resolve)));
  const still = new RenderStill({ session }, 'movie');
  still.load(1); still.clear(); still.load(1);
  pending[0](reply(1_000_000)); await settle();
  expect(still.image).toBeNull();
  expect(session).toHaveBeenCalledTimes(2);
  pending[1](reply(1_000_000)); await settle();
  expect(still.image?.time_us).toBe(1_000_000);
  expect(still.busy).toBe(false);
});
