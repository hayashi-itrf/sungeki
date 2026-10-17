import { afterEach, expect, test, vi } from 'vitest';
import { readPreviewVolume, savePreviewVolume } from './preview-volume';
afterEach(() => vi.unstubAllGlobals());
test.each([0, 0.2, 1])('restores saved audition volume %s including mute', value => {
  const values = new Map<string, string>();
  vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value) });
  expect(readPreviewVolume()).toBe(1);
  savePreviewVolume(value); expect(readPreviewVolume()).toBe(value);
  savePreviewVolume(NaN); savePreviewVolume(-1); savePreviewVolume(2);
  expect(readPreviewVolume()).toBe(value);
});
test.each(['', 'NaN', 'Infinity', '-1', '2', 'broken'])('invalid stored volume %s uses default', raw => {
  vi.stubGlobal('localStorage', { getItem: () => raw });
  expect(readPreviewVolume()).toBe(1);
});
test('unavailable storage does not prevent playback', () => {
  vi.stubGlobal('localStorage', { getItem() { throw Error('denied'); }, setItem() { throw Error('denied'); } });
  expect(readPreviewVolume()).toBe(1); expect(() => savePreviewVolume(0.2)).not.toThrow();
});
