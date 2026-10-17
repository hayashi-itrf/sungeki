import { expect, test, vi } from 'vitest';
import type { SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { LiveAudioCache } from './live-audio-cache';
import type { LiveCue } from './live-audio-plan';
const snapshot = { session_id: 's', revision: 7 } as SessionSnapshot;
const cue = { clip: { id: 'clip' }, assetId: 'audio' } as LiveCue;
test('bounded chunks reconstruct exactly once, decode once, and reuse the same clip', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'editing_audio_bytes', offset: 0, total: 4, base64: btoa('ab') })
    .mockResolvedValueOnce({ type: 'editing_audio_bytes', offset: 2, total: 4, base64: btoa('cd') });
  const buffer = { length: 100, numberOfChannels: 1 } as AudioBuffer;
  const decodeAudioData = vi.fn(async (bytes: ArrayBuffer) => { expect([...new Uint8Array(bytes)]).toEqual([97,98,99,100]); return buffer; });
  const cache = new LiveAudioCache({ session }, snapshot, { decodeAudioData } as unknown as AudioContext);
  const a = cache.load(cue), b = cache.load(cue);
  expect(a).toBe(b); expect(await a).toBe(buffer); expect(await cache.load(cue)).toBe(buffer);
  expect(session).toHaveBeenCalledTimes(2); expect(decodeAudioData).toHaveBeenCalledOnce();
  expect(session.mock.calls[1][0]).toMatchObject({ session_id: 's', expected_revision: 7, clip_id: 'clip', offset: 2 });
  cache.dispose();
});
test('malformed ranges are rejected before decoding', async () => {
  const decodeAudioData = vi.fn();
  for (const result of [{ offset: 1, total: 1, base64: 'YQ==' }, { offset: 0, total: 1, base64: '' }, { offset: 0, total: 1, base64: 'YWI=' }]) {
    const cache = new LiveAudioCache({ session: async () => ({ type: 'editing_audio_bytes', ...result }) }, snapshot, { decodeAudioData } as unknown as AudioContext);
    await expect(cache.load(cue)).rejects.toThrow(); cache.dispose();
  }
  expect(decodeAudioData).not.toHaveBeenCalled();
});
test('a removed editor cannot retain or play a late decoded response', async () => {
  let resolve!: (value: AudioBuffer) => void;
  const decoding = new Promise<AudioBuffer>(done => resolve = done);
  const cache = new LiveAudioCache({ session: async () => ({ type: 'editing_audio_bytes', offset: 0, total: 1, base64: 'YQ==' }) as SessionResponse }, snapshot,
    { decodeAudioData: () => decoding } as unknown as AudioContext);
  const loading = cache.load(cue); await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  cache.dispose(); resolve({ length: 1, numberOfChannels: 1 } as AudioBuffer);
  await expect(loading).rejects.toThrow(); expect(cache.get(cue.clip.id)).toBeUndefined();
});
