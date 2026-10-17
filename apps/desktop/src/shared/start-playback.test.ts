import { expect, test, vi } from 'vitest';
import { startPlayback } from './start-playback';

test('an interrupted pending start does not poison a later successful replay', async () => {
  let reject!: (error: unknown) => void;
  const media = { error: null, play: vi.fn()
    .mockImplementationOnce(() => new Promise<void>((_, fail) => { reject = fail; }))
    .mockResolvedValue(undefined) };
  const failed = vi.fn();
  const first = startPlayback(media, failed);
  // A pause interrupts the first start; a new play may begin before rejection arrives.
  await startPlayback(media, failed);
  reject(new DOMException('Playback paused', 'AbortError'));
  await first;
  expect(failed).not.toHaveBeenCalled();
  expect(media.play).toHaveBeenCalledTimes(2);
});

test.each(['NotAllowedError', 'NotSupportedError', 'NetworkError'])(
  '%s is still reported as a real failure', async name => {
    const failed = vi.fn();
    await startPlayback({ error: null, play: () => Promise.reject(new DOMException('failure', name)) }, failed);
    expect(failed).toHaveBeenCalledOnce();
  });

test('an AbortError with an actual media error is not suppressed', async () => {
  const failed = vi.fn();
  const error = { code: 3, message: 'decode failure' } as MediaError;
  await startPlayback({ error, play: () => Promise.reject(new DOMException('aborted', 'AbortError')) }, failed);
  expect(failed).toHaveBeenCalledOnce();
});

test('synchronous and unexpected failures retain the error path', async () => {
  const failed = vi.fn();
  await startPlayback({ error: null, play: () => { throw new Error('unavailable'); } }, failed);
  expect(failed).toHaveBeenCalledOnce();
});
