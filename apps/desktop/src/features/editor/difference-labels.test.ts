import { afterEach, expect, test } from 'vitest';
import type { ProjectDifference, ValuePreview } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { differenceLabel, differenceValue } from './difference-labels';
const entry: ProjectDifference = { collection: 'characters', entity_id: null, field: '/voice/settings/speed_scale', kind: 'changed', before: null, after: null };
const preview = (value: unknown): ValuePreview => ({ json: JSON.stringify(value), truncated: false });
afterEach(() => { locale.current = 'ja'; });

test('difference labels distinguish engine voice from the selected speaker', () => {
  locale.current = 'en';
  expect(differenceLabel(entry)).toBe('Characters / Voice / Settings / Speech speed: Changed');
  expect(differenceLabel({ ...entry, field: '/voice/settings/voice' })).toBe('Characters / Voice / Settings / Speaker: Changed');
  locale.current = 'ja';
  expect(differenceLabel(entry)).toBe('キャラクターの音声・設定・話速：変更');
});

test('difference values translate guidance but preserve project text and numeric precision', () => {
  locale.current = 'en';
  expect(differenceValue(null, entry)).toBe('None');
  expect(differenceValue(preview(null), entry)).toBe('Not set');
  expect(differenceValue(preview(true), entry)).toBe('Enabled');
  expect(differenceValue(preview(false), entry)).toBe('Disabled');
  expect(differenceValue(preview('残す本文'), entry)).toBe('残す本文');
  expect(differenceValue(preview(1234567), { ...entry, field: '/duration_us' })).toBe('1.234567 s');
  expect(differenceValue(preview(['a', 'b']), entry)).toBe('2 items');
  expect(differenceValue({ json: 'broken', truncated: false }, entry)).toBe('Cannot display value');
  expect(differenceValue({ json: 'partial', truncated: true }, entry)).toBe('Value too long to display; inspect it in the project');
  locale.current = 'ja';
  expect(differenceValue(preview(1234567), { ...entry, field: '/duration_us' })).toBe('1.234567 秒');
});

test('whole fade values expose both durations before setting or removing an effect', () => {
  const fade = { ...entry, collection: 'scenes', field: '/fade' };
  const value = preview({ in_us: 800000, out_us: 1234567 });
  expect(differenceValue(value, fade)).toBe('フェードイン 0.8 秒、フェードアウト 1.234567 秒');
  locale.current = 'en';
  expect(differenceValue(value, fade)).toBe('Fade in 0.8 s, fade out 1.234567 s');
  expect(differenceValue(preview({ in_us: 0, out_us: 0 }), fade)).toBe('Fade in 0 s, fade out 0 s');
  expect(differenceValue(null, fade)).toBe('None');
  expect(differenceValue(preview(null), fade)).toBe('Not set');
  expect(differenceValue({ ...value, truncated: true }, fade)).toContain('Value too long');
  for (const invalid of [{ in_us: -1, out_us: 0 }, { in_us: 1 }, { in_us: '1', out_us: 0 },
    { in_us: 1.5, out_us: 0 }, { in_us: 0, out_us: 0, unknown: true }]) {
    expect(differenceValue(preview(invalid), fade)).toBe('Settings');
  }
  expect(differenceValue(value, entry)).toBe('Settings');
});
