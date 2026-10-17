import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import { locale } from '../../shared/i18n/locale.svelte';
import { RenderJobsController, renderPhase } from './jobs.svelte';
import RenderHistory from './RenderHistory.svelte';
afterEach(() => { locale.current = 'ja'; });
test('unreadable entries render their retry controls rather than empty history in both languages', () => {
  const jobs = new RenderJobsController({ session: vi.fn() }, 'session');
  jobs.unavailable = [{ id: 'broken-id', message: 'fixture failure' }];
  for (const language of ['ja', 'en'] as const) {
    locale.current = language;
    const { body } = render(RenderHistory, { props: { jobs } });
    expect(body).toContain('fixture failure');
    expect(body).toContain(locale.text('renderReadRetry'));
    expect(body).not.toContain(locale.text('renderEmpty'));
  }
});
test('an empty idle history is explicit and render phase labels follow language changes', () => {
  const jobs = new RenderJobsController({ session: vi.fn() }, 'session');
  locale.current = 'en';
  expect(render(RenderHistory, { props: { jobs } }).body).toContain('No export history.');
  expect(renderPhase.running).toBe('Exporting video');
  locale.current = 'ja'; expect(renderPhase.running).toBe('動画を出力中');
  jobs.busy = true;
  expect(render(RenderHistory, { props: { jobs } }).body).not.toContain(locale.text('renderEmpty'));
});
test('an in-flight automatic refresh leaves history markup and enabled controls unchanged', async () => {
  let finish!: (value: import('@sungeki/api-client').SessionResponse) => void;
  const jobs = new RenderJobsController({ session: vi.fn(() => new Promise<import('@sungeki/api-client').SessionResponse>(resolve => { finish = resolve; })) }, 'session');
  const job: import('@sungeki/api-client').RenderJob = {
    job_id: 'current', session_id: 'session', purpose: 'final', source_revision: 1, content_hash: 'hash',
    phase: 'running', progress: null, output: null, error: null, stale: false,
  };
  jobs.jobs = [job]; jobs.selected = job;
  const before = render(RenderHistory, { props: { jobs } }).body;
  const pending = jobs.poll();
  expect(render(RenderHistory, { props: { jobs } }).body).toBe(before);
  finish({ type: 'render_job', job }); await pending;
  expect(render(RenderHistory, { props: { jobs } }).body).toBe(before);
});
