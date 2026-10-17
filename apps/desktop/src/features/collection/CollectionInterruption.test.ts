import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { CollectionJob } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import CollectionInterruption from './CollectionInterruption.svelte';

const job: CollectionJob = {
  job_id: 'job', session_id: 'session', source_revision: 1, content_hash: 'hash',
  include_checkpoints: false, checkpoint_count: 0, phase: 'interrupted',
  destination: '/output', dry_run: false, plan: null, output: null, stale: false,
  error: { code: 'IO_ERROR', message: 'interrupted', retryable: false, suggested_actions: [],
    details: { staging_path: '/disk/<unfinished>&copy', staging_status: 'owned' } },
};
const view = (value = job) => render(CollectionInterruption, { props: { job: value } }).body;
afterEach(() => { locale.current = 'ja'; });

test('interrupted work shows an escaped historical path and retry guidance without mutating the job', () => {
  const before = structuredClone(job);
  const html = view();
  expect(html).toContain('/disk/&lt;unfinished>&amp;copy');
  expect(html).not.toContain('<unfinished>');
  expect(html).toContain('現在の存在や内容は再確認していません');
  expect(html).toContain('事前確認');
  locale.current = 'en';
  expect(view()).toContain('Its current existence and contents have not been checked again');
  expect(view()).toContain('Check without copying');
  expect(job).toEqual(before);
});

test('missing stage metadata still gives retry guidance; other phases do not claim interruption', () => {
  const html = view({ ...job, error: null });
  expect(html).toContain('この収集は完了していません');
  expect(html).not.toContain('作業フォルダ');
  for (const phase of ['queued', 'running', 'cancelling', 'cancelled', 'failed', 'succeeded'] as const) {
    expect(view({ ...job, phase })).not.toContain('<aside');
  }
});
