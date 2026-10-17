import { expect, test } from 'vitest';
import { isBlankScriptLine } from './script-input';
test('Unicode whitespace matches the host and BOM remains content', () => {
  for (const text of ['', ' \t\r', '\u0085', '\u00a0', '\u1680', '\u2000\u200a', '\u2028\u2029', '\u202f\u205f\u3000']) {
    expect(isBlankScriptLine(text)).toBe(true);
  }
  for (const text of ['\ufeff', '\u200b', '　セリフ　', '\u0085本文']) {
    expect(isBlankScriptLine(text)).toBe(false);
  }
});
