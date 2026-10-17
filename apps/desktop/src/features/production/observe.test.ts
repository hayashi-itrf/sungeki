import { afterEach, expect, test, vi } from 'vitest';
import type { BuildVideoJob, SessionResponse } from '@sungeki/api-client';
import { BuildJobsController } from './jobs.svelte';
import { observeBuildJob } from './observe';

const running: BuildVideoJob = {
  job_id: 'existing-production', session_id: 'project', title: 'Long dialogue',
  phase: 'running', progress: { stage: '映像・音声を出力して検証中', fraction: null },
  completed_steps: 10, total_steps: 12, child_job_id: 'render-child', source_revision: 6,
  output: null, error: null, stale: false, review_asset_freshness: null,
};
const reply = (phase: BuildVideoJob['phase']): SessionResponse => ({
  type: 'build_video_job', job: { ...running, phase },
});
afterEach(() => { vi.clearAllTimers(); vi.useRealTimers(); });

test('a failed observation retries the same ID, reaches terminal, and never restarts production', async () => {
  vi.useFakeTimers();
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'timeout' })
    .mockResolvedValueOnce(reply('running')).mockResolvedValueOnce(reply('succeeded'));
  const jobs = new BuildJobsController({ session }); jobs.selected = running;
  const stop = observeBuildJob(jobs);
  await vi.advanceTimersByTimeAsync(1500);
  expect(jobs.error).toBe('timeout'); expect(jobs.selected?.phase).toBe('running');
  await vi.advanceTimersByTimeAsync(1500);
  expect(jobs.error).toBe(''); expect(jobs.selected?.phase).toBe('running');
  await vi.advanceTimersByTimeAsync(1500);
  expect(jobs.selected?.phase).toBe('succeeded');
  await vi.advanceTimersByTimeAsync(4500); stop();
  expect(session.mock.calls).toEqual(Array.from({ length: 3 }, () => [{
    type: 'build_video', request: { type: 'get', job_id: 'existing-production', wait_ms: 0 },
  }]));
});

test('in-flight reads do not overlap and closing observation prevents further requests', async () => {
  vi.useFakeTimers();
  let resolve!: (reply: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const jobs = new BuildJobsController({ session }); jobs.selected = running;
  const stop = observeBuildJob(jobs);
  await vi.advanceTimersByTimeAsync(6000);
  expect(session).toHaveBeenCalledTimes(1);
  stop(); resolve(reply('running')); await vi.advanceTimersByTimeAsync(6000);
  expect(jobs.busy).toBe(false); expect(session).toHaveBeenCalledTimes(1);
});

test('completed output keeps observing asset freshness and staleness until the view closes', async () => {
  vi.useFakeTimers();
  const completed: BuildVideoJob = { ...running, phase: 'succeeded', output: {
    project_path: '/project.json', project_sha256: 'project', render_job_id: 'render', review_job_id: 'review',
    render: { path: '/output.mp4', sha256: 'video', byte_length: 1, frame_count: 1, font_sha256: 'font', layout_version: 9 },
    review: { report_id: 'review', source_revision: 6, content_hash: 'project', asset_fingerprint: 'assets',
      diagnostics: { checks: [], issues: [], summary: { passed: 0, failed: 0, not_run: 0, issue_count: 0, complete: false } } },
  }, review_asset_freshness: { status: 'unchecked', checking: true, checked_at_ms: null, last_known_changed: false, message: null } };
  const changed: BuildVideoJob = { ...completed, stale: true, review_asset_freshness: {
    status: 'changed', checking: false, checked_at_ms: 123, last_known_changed: true, message: null,
  } };
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'timeout' })
    .mockResolvedValue({ type: 'build_video_job', job: changed });
  const jobs = new BuildJobsController({ session }); jobs.selected = completed;
  const stop = observeBuildJob(jobs);
  await vi.advanceTimersByTimeAsync(1500);
  expect(jobs.selected!.review_asset_freshness!.status).toBe('unchecked');
  await vi.advanceTimersByTimeAsync(1500);
  expect(jobs.selected!.review_asset_freshness!.status).toBe('changed');
  expect(jobs.selected!.stale).toBe(true);
  stop(); await vi.advanceTimersByTimeAsync(6000);
  expect(session.mock.calls).toEqual(Array.from({ length: 2 }, () => [{
    type: 'build_video', request: { type: 'get', job_id: running.job_id, wait_ms: 0 },
  }]));
});
