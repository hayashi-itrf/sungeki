import { expect, test } from 'vitest';
import { forewordFromForm } from './foreword';
test('optional foreword preserves text and converts seconds into the common recipe', () => {
  expect(forewordFromForm()).toBeNull();
  expect(forewordFromForm({ enabled: false, text: '', seconds: undefined })).toBeNull();
  expect(forewordFromForm({ enabled: true, text: ' 前書き\r\n二行目\r三行目 ', seconds: 8.1 })).toEqual({
    preset_id: 'foreword-wide-v1', text: ' 前書き\n二行目\n三行目 ', duration_us: 8100000,
  });
  for (const seconds of [1.6, 60]) expect(forewordFromForm({ enabled: true, text: '前書き', seconds })?.duration_us).toBe(seconds * 1000000);
});
test('enabled foreword rejects missing duration, nonfinite values and invalid text before start', () => {
  for (const seconds of [undefined, NaN, Infinity, 1.59, 60.01]) expect(() => forewordFromForm({ enabled: true, text: '前書き', seconds })).toThrow('表示時間');
  for (const text of ['', '  \n', 'a\0b', '一\n二\n三\n四\n五', 'あ'.repeat(1366)]) expect(() => forewordFromForm({ enabled: true, text, seconds: 8 })).toThrow('前書き');
  expect(forewordFromForm({ enabled: true, text: 'あ'.repeat(1365), seconds: 8 })).not.toBeNull();
});
