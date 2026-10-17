import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { AssetFreshness } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import AssetFreshnessView from './AssetFreshnessView.svelte';

test('never describes unchecked or unavailable observations as matching and shows scan progress', () => {
  const base: AssetFreshness = { status: 'unchecked', checking: true, last_known_changed: false, checked_at_ms: null, message: null };
  const unchecked = render(AssetFreshnessView, { props: { freshness: base } }).body;
  expect(unchecked).toContain('素材の再確認：未確認');
  expect(unchecked).toContain('再確認中');
  expect(unchecked).not.toContain('照合時点で一致');
  const failed = render(AssetFreshnessView, { props: { freshness: { ...base, status: 'unavailable', checking: false, message: '読み取り失敗' } } }).body;
  expect(failed).toContain('確認できません');
  expect(failed).toContain('読み取り失敗');
  expect(failed).not.toContain('照合時点で一致');
});

test('matching is explicitly time limited and includes the observation time', () => {
  const freshness: AssetFreshness = { status: 'matching', checking: false, last_known_changed: false, checked_at_ms: 1700000000000, message: null };
  const body = render(AssetFreshnessView, { props: { freshness } }).body;
  expect(body).toContain('照合時点で一致');
  expect(body).toContain('最終照合');
});

test('reports without a complete baseline do not promise automatic rescanning', () => {
  const freshness: AssetFreshness = { status: 'unchecked', checking: false, last_known_changed: false, checked_at_ms: null, message: null };
  const body = render(AssetFreshnessView, { props: { freshness, watchable: false } }).body;
  expect(body).toContain('原本の比較情報を取得できていません');
  expect(body).not.toContain('定期的に再確認');
});


afterEach(() => { locale.current = 'ja'; });
test('English freshness messages preserve changed/missing status and unknown diagnostic details', () => {
  const cases = [
    ['matching', '登録された画像・音声・PSD・動画はありません', 'No images, audio, PSD or video assets are registered.'],
    ['changed', '検査後に画像・音声・PSD・動画の内容が変わっています', 'contents have changed since inspection.'],
    ['changed', '検査時にあった画像・音声・PSD・動画が見つかりません', 'present during inspection is now missing.'],
  ] as const;
  for (const [status, message, english] of cases) {
    const freshness: AssetFreshness = { status, checking: false, last_known_changed: status === 'changed', checked_at_ms: null, message };
    locale.current = 'ja';
    expect(render(AssetFreshnessView, { props: { freshness } }).body).toContain(message);
    locale.current = 'en';
    const body = render(AssetFreshnessView, { props: { freshness } }).body;
    expect(body).toContain(english);
    expect(body).not.toContain(message);
    if (status === 'changed') expect(body).not.toContain('Matched at last check');
    expect(freshness.message).toBe(message);
  }
  const message = '読み取り失敗: /日本語/$file.wav';
  const freshness: AssetFreshness = { status: 'unavailable', checking: false, last_known_changed: false, checked_at_ms: null, message };
  expect(render(AssetFreshnessView, { props: { freshness } }).body).toContain(message);
});
