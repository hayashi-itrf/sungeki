import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { ReviewJob } from '@sungeki/api-client';
import ReviewResult from './ReviewResult.svelte';
import { locale } from '../../shared/i18n/locale.svelte';
afterEach(() => { locale.current = 'ja'; });
test('successful execution retains not-run labels and marks newer document revisions stale', () => {
  const job: ReviewJob = { job_id: 'job', session_id: 'session', source_revision: 2, content_hash: 'hash', phase: 'succeeded', progress: null, error: null, stale: false, asset_freshness: { status: 'unchecked', last_known_changed: false, checking: false, checked_at_ms: null, message: null },
    report: { report_id: 'job', source_revision: 2, content_hash: 'hash', asset_fingerprint: null, diagnostics: {
      checks: [{ kind: 'structure', status: 'passed', description: '構造を確認' }, { kind: 'text_layout', status: 'not_run', description: '未検査です' }], issues: [],
      summary: { passed: 1, failed: 0, not_run: 1, issue_count: 0, complete: false } } } };
  const { body } = render(ReviewResult, { props: { job, revision: 3 } });
  expect(body).toContain('古い結果です');
  expect(body).toContain('字幕の配置');
  expect(body).toContain('未検査');
  expect(body).toContain('品質は確認していません');
  expect(render(ReviewResult, { props: { job, revision: 2 } }).body).not.toContain('古い結果です');
  // Host can mark a report stale without a document revision change (renderer upgrade).
  const outdated = render(ReviewResult, { props: { job: { ...job, stale: true }, revision: 2 } }).body;
  expect(outdated).toContain('古い結果です');
  expect(outdated).toContain('描画処理');
});


test('English results distinguish image generation from visual inspection and preserve stale warnings', () => {
  locale.current = 'en';
  const job: ReviewJob = {
    job_id: 'job', session_id: 'session', source_revision: 2, content_hash: 'hash',
    phase: 'succeeded', progress: null, error: null, stale: true,
    asset_freshness: { status: 'unavailable', last_known_changed: false, checking: false, checked_at_ms: null, message: null },
    report: { report_id: 'job', source_revision: 2, content_hash: 'hash', asset_fingerprint: null,
      diagnostics: { checks: [
        { kind: 'representative_frames', status: 'passed', description: 'Generated' },
        { kind: 'visual_review', status: 'not_run', description: '人間または画像を読めるAIによる映像内容の確認は未実施です。' },
      ], issues: [], summary: { passed: 1, failed: 0, not_run: 1, issue_count: 0, complete: false } },
    },
  };
  const html = render(ReviewResult, { props: { job, revision: 2 } }).body;
  expect(html).toContain('This result is out of date.');
  expect(html).toContain('Images generated');
  expect(html).toContain('Visual inspection');
  expect(html).toContain('The video content has not been reviewed by a person');
  expect(html).not.toContain('人間または画像');
  expect(html).toContain('Not inspected');
  expect(html).not.toContain('Checked; no issues');
  expect(html).toContain('Overall video and audio quality has not been verified.');
  expect(html).toContain('Source asset recheck: Could not check');
  expect(html).toContain('Source comparison data is unavailable');
  expect(html).not.toContain('Matched at last check');
  expect(job.report?.diagnostics.checks[1].status).toBe('not_run');
});


test('warnings remain visible in the summary even when no check has failed', () => {
  const job: ReviewJob = {
    job_id: 'overlap', session_id: 'session', source_revision: 1, content_hash: 'hash',
    phase: 'succeeded', progress: null, error: null, stale: false,
    asset_freshness: { status: 'unchecked', last_known_changed: false, checking: false, checked_at_ms: null, message: null },
    report: { report_id: 'overlap', source_revision: 1, content_hash: 'hash', asset_fingerprint: null,
      diagnostics: { checks: [{ kind: 'audio', status: 'not_run', description: 'Listening not performed' }],
        issues: ['SPEECH_OVERLAP', 'MIX_PEAK'].map(code => ({
          severity: 'warning', code, field: '/clips', clip_id: null, scene_id: null,
          asset_id: null, start_us: null, end_us: null, message: code,
        })), summary: { passed: 2, failed: 0, not_run: 4, issue_count: 2, complete: false } },
    },
  };
  const original = structuredClone(job);
  const japanese = render(ReviewResult, { props: { job, revision: 1 } }).body;
  expect(japanese).toContain('不合格 0 ／ 未検査 4');
  expect(japanese).toContain('エラー 0 件 ／ 注意 2 件 ／ 情報 0 件');
  expect(japanese).not.toContain('問題あり 0');
  locale.current = 'en';
  const english = render(ReviewResult, { props: { job, revision: 1 } }).body;
  expect(english).toContain('Errors: 0 / Warnings: 2 / Information: 0');
  expect(english).toContain('Not inspected: 4');
  expect(job).toEqual(original);
});
