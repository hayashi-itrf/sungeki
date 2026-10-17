import { expect, test } from 'vitest';
import { parseSampling } from './sampling';
test('parses explicit decimal seconds without losing microseconds and preserves requested order', () => {
  expect(parseSampling('12, 0.000001、3.5', 3, 5)).toEqual({ max_frames: 3, text_safe_margin_percent: 5, requested_times_us: [12000000, 1, 3500000] });
  expect(parseSampling(' ', 12, 5).requested_times_us).toEqual([]);
});
test('rejects ambiguous times, fractional limits and unsafe numbers', () => {
  for (const input of ['-1', '1e3', '1:23', '3.0000001', '9007199254740991', '2,']) expect(() => parseSampling(input, 12, 5)).toThrow();
  for (const limit of [undefined, 0, 121, 1.5, NaN]) expect(() => parseSampling('', limit, 5)).toThrow();
  expect(() => parseSampling(Array(121).fill('0').join(','), 12, 5)).toThrow();
});

test("validates the configured safe margin", () => {
  expect(parseSampling("", 1, 0).text_safe_margin_percent).toBe(0);
  expect(parseSampling("", 1, 25).text_safe_margin_percent).toBe(25);
  for (const margin of [undefined, -1, 26, 1.5, NaN]) expect(() => parseSampling("", 1, margin)).toThrow();
});
