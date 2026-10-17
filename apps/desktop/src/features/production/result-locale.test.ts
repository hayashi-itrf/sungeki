import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { BuildVideoJob } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { buildPhase } from './jobs.svelte';
import BuildResult from './BuildResult.svelte';
afterEach(() => { locale.current = 'ja'; });
const job: BuildVideoJob = {
  job_id: 'build', session_id: 'session', title: '日本語の作品', phase: 'succeeded', progress: null,
  completed_steps: 10, total_steps: 10, source_revision: 4, child_job_id: null, error: null, stale: true,
  review_asset_freshness: null, output: {
    project_path: '/作品/保存.vvm.json', project_sha256: 'project', render_job_id: 'render', review_job_id: 'review',
    render: { path: '/作品/動画.mp4', sha256: 'video', byte_length: 200, frame_count: 60, font_sha256: 'font', layout_version: 9 },
    review: { report_id: 'review', source_revision: 4, content_hash: 'project', asset_fingerprint: null, diagnostics: {
      checks: [{ kind: 'structure', status: 'passed', description: '元の検査説明' },
        { kind: 'audio', status: 'not_run', description: '音声は未検査' }], issues: [],
      summary: { passed: 1, failed: 0, not_run: 1, issue_count: 0, complete: false } } },
  },
};
test('production result translates labels while retaining paths, diagnostics and incomplete review', () => {
  const original = structuredClone(job);
  locale.current = 'en';
  const english = render(BuildResult, { props: { job } }).body;
  for (const label of ['Saved project', 'Completed video', 'Open completed video', 'Review results', 'Audio', 'Not checked']) expect(english).toContain(label);
  expect(english).toContain('Completion does not mean all quality checks have passed.');
  expect(english).toContain('/作品/動画.mp4'); expect(english).toContain('音声は未検査');
  expect(english).not.toContain('完成動画を開く');
  locale.current = 'ja';
  const japanese = render(BuildResult, { props: { job } }).body;
  expect(japanese).toContain('完成動画を開く'); expect(japanese).toContain('未検査');
  expect(job).toEqual(original);
});
test('every production phase follows changes of display language', () => {
  locale.current = 'en';
  expect(Object.values(buildPhase)).toEqual(['Queued', 'Producing video', 'Confirming stop', 'Cancelled', 'Production complete', 'Failed', 'Interrupted']);
  locale.current = 'ja';
  expect(buildPhase.cancelling).toBe('停止を確認中');
  expect(buildPhase.succeeded).toBe('制作完了');
});

test('build results expose review images, generation errors and omitted coverage without claiming inspection', () => {
  const withFrames = structuredClone(job);
  const report = withFrames.output!.review;
  report.representative_frames = {
    sampling: { frame_count: 100, selected: [], scenes_without_visible_frame: [],
      omitted: [{ frame_index: 99, time_us: 3_300_000, reasons: [] }], unsampled: [{ first: 1, end: 30 }] },
    frames: [{ sample: { frame_index: 60, time_us: 2_000_000, reasons: [] }, output: null, error: 'Frame generation failed' }],
    contact_sheets: [],
    check: { kind: 'representative_frames', status: 'failed', description: 'Frame generation failed' },
    visual_review: { kind: 'visual_review', status: 'not_run', description: 'Not inspected' },
  };
  const html = render(BuildResult, { props: { job: withFrames } }).body;
  expect(html).toContain('Frame generation failed');
  expect(html).toContain('2.000 秒の画像');
  expect(html).toContain('作品・素材・描画処理の変更前の画像');
  expect(html).toContain('目視確認済みにはなりません');
  expect(html).toContain('3.300000');
  expect(html).toContain('1 – 29');
  expect(html).not.toContain('<img');
});


test('production review summary distinguishes findings from failed checks', () => {
  const reviewed = structuredClone(job);
  reviewed.output!.review.diagnostics.issues = ['error', 'warning', 'info'].map(severity => ({
    severity: severity as 'error' | 'warning' | 'info', code: 'REVIEW_FINDING', field: '/',
    clip_id: null, scene_id: null, asset_id: null, start_us: null, end_us: null, message: severity,
  }));
  reviewed.output!.review.diagnostics.summary.issue_count = 3;
  const original = structuredClone(reviewed);
  expect(render(BuildResult, { props: { job: reviewed } }).body)
    .toContain('エラー 1 件 ／ 注意 1 件 ／ 情報 1 件');
  locale.current = 'en';
  expect(render(BuildResult, { props: { job: reviewed } }).body)
    .toContain('Errors: 1 / Warnings: 1 / Information: 1');
  expect(reviewed).toEqual(original);
});
