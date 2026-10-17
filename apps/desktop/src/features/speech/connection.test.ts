import { afterEach, expect, it, vi } from 'vitest';
import { rememberVoicevoxUrl, voicevoxUrl } from './connection';
afterEach(() => vi.unstubAllGlobals());
it('shares a discovered endpoint across newly opened voice and synthesis panels', () => {
  const values = new Map<string, string>();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  });
  expect(voicevoxUrl()).toBe('http://127.0.0.1:50021');
  rememberVoicevoxUrl('http://127.0.0.1:55021');
  expect(voicevoxUrl()).toBe('http://127.0.0.1:55021');
});
it('does not block editing when UI storage is unavailable', () => {
  vi.stubGlobal('localStorage', {
    getItem() { throw new Error('denied'); }, setItem() { throw new Error('denied'); },
  });
  expect(voicevoxUrl()).toBe('http://127.0.0.1:50021');
  expect(() => rememberVoicevoxUrl('http://127.0.0.1:55021')).not.toThrow();
});
