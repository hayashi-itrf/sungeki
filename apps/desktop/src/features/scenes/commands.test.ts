import { afterEach, expect, test } from 'vitest';
import { timeUs } from './commands';
import { locale } from '../../shared/i18n/locale.svelte';
afterEach(() => { locale.current = 'ja'; });

test('time inputs keep microsecond precision and the seven day boundary', () => {
  expect(timeUs(1.234567)).toBe(1234567);
  expect(timeUs(604800)).toBe(604800000000);
  for (const value of [NaN, Infinity, -1, 604800.000001]) expect(() => timeUs(value)).toThrow();
});

test('invalid time guidance uses the current display language', () => {
  locale.current = 'en';
  expect(() => timeUs(-1)).toThrow('Enter a time between 0 and 604800 seconds.');
  locale.current = 'ja';
  expect(() => timeUs(-1)).toThrow('時刻は0〜604800秒で指定してください。');
});
