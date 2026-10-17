import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { BuildVideoJob } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import BuildFailure from './BuildFailure.svelte';

afterEach(() => { locale.current = 'ja'; });
const job: BuildVideoJob = {
  job_id: 'parent', session_id: 'session', title: '途中の作品', phase: 'interrupted',
  completed_steps: 3, total_steps: 10, progress: null, source_revision: 2,
  child_job_id: 'child', stale: false, output: null, review_asset_freshness: null,
  error: {
    code: 'VALIDATION_FAILED', message: '音声生成で制作が停止しました', retryable: false,
    suggested_actions: ['話者設定を確認してください', '<script>再実行</script>'],
    details: { build_child_job_id: 'child', clip_id: 'speech-2' },
  },
};
test('production failure preserves actionable diagnostics and escapes host text', () => {
  const before = structuredClone(job);
  const html = render(BuildFailure, { props: { job } }).body;
  for (const value of ['音声生成で制作が停止しました', '話者設定を確認してください', 'VALIDATION_FAILED', 'speech-2', 'child']) expect(html).toContain(value);
  expect(html).toContain('&lt;script>');
  expect(html).not.toContain('<script>');
  expect(html).toContain('この制作をそのまま再開する操作は未対応');
  expect(job).toEqual(before);
});
test.each(['failed', 'interrupted', 'cancelled'] as const)('explains incomplete production for %s even without a recorded error', phase => {
  locale.current = 'en';
  const html = render(BuildFailure, { props: { job: { ...job, phase, error: null } } }).body;
  expect(html).toContain('It may not yet include all speech or visual elements.');
  expect(html).toContain('you can select saved speech to carry over');
  expect(html).not.toContain('role="alert"');
});
test.each(['queued', 'running', 'cancelling', 'succeeded'] as const)('does not suggest editing an incomplete project during %s', phase => {
  const html = render(BuildFailure, { props: { job: { ...job, phase, error: null } } }).body;
  expect(html).not.toContain('この制作をそのまま再開する操作は未対応');
});
