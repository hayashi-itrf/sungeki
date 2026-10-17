import { expect, test, vi } from 'vitest';
import type { FramePreviewJob, SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { FramePreviewJobsController } from './jobs.svelte';
const snapshot = { session_id: 's', revision: 7 } as SessionSnapshot;
function result(id: string, overrides: Partial<FramePreviewJob> = {}): SessionResponse {
  return { type: 'frame_preview_job', job: { job_id: id, session_id: 's', source_revision: 7,
    content_hash: 'hash', phase: 'queued', progress: null, output: null, error: null, stale: false, ...overrides } };
}

test('uncertain start blocks new requests and retries the identical revision and job ID', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'unknown' });
  const jobs = new FramePreviewJobsController({ session }, 's');
  await jobs.start(snapshot, 159);
  const original = jobs.pending;
  expect(original).toMatchObject({ session_id: 's', expected_revision: 7, frame_index: 159 });
  await jobs.start({ ...snapshot, revision: 8 }, 276);
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(original);
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.job_id).toBe(original!.job_id);
});

test('mismatched response remains uncertain while explicit rejection allows corrected submission', async () => {
  const session = vi.fn().mockResolvedValue(result('wrong'));
  const jobs = new FramePreviewJobsController({ session }, 's');
  await jobs.start(snapshot, 159);
  expect(jobs.pending).not.toBeNull(); expect(jobs.selected).toBeNull();
  session.mockRejectedValueOnce({ code: 'CAPABILITY_UNAVAILABLE', message: 'not configured' });
  await jobs.retry(); expect(jobs.pending).toBeNull();
  session.mockImplementation(async r => result(r.request.args.job_id, { session_id: 'other' }));
  await jobs.start(snapshot, 159); expect(jobs.error).toContain('一致しません');
});

test('pagination and cancellation operate on the selected job, preserving failure and stale state', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `job-${i}`);
  const session = vi.fn(async r => r.request.type === 'list'
    ? { type: 'frame_preview_jobs', job_ids: r.request.after ? ['next'] : ids } as SessionResponse
    : result(r.request.job_id, { phase: r.request.type === 'cancel' ? 'cancelled' : 'running', stale: true }));
  const jobs = new FramePreviewJobsController({ session }, 's');
  await jobs.list(); expect(jobs.more).toBe(true);
  await jobs.list(true); expect(jobs.jobs).toHaveLength(13); expect(jobs.more).toBe(false);
  expect(session.mock.calls[13]![0].request.after).toBe('job-11');
  await jobs.refresh('next'); await jobs.cancel();
  expect(jobs.selected).toMatchObject({ job_id: 'next', phase: 'cancelled', stale: true });
  expect(jobs.jobs.filter(j => j.job_id === 'next')).toHaveLength(1);
});

test('disposed controllers ignore late replies and never dispatch new work', async () => {
  let resolve!: (r: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const jobs = new FramePreviewJobsController({ session }, 's');
  const started = jobs.start(snapshot, 159); const id = jobs.pending!.job_id;
  jobs.dispose(); resolve(result(id)); await started;
  expect(jobs.selected).toBeNull();
  await jobs.refresh(id); await jobs.list(); await jobs.start(snapshot, 159);
  expect(session).toHaveBeenCalledTimes(1);
});

test('invalid frame rejection releases retry state so the user can correct its position', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'VALIDATION_FAILED', message: '範囲外' });
  const jobs = new FramePreviewJobsController({ session }, 's');
  await jobs.start(snapshot, 999999);
  expect(jobs.pending).toBeNull(); expect(jobs.error).toBe('範囲外');
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.start(snapshot, 0);
  expect(session.mock.lastCall![0].request.args.frame_index).toBe(0);
  expect(jobs.selected?.phase).toBe('queued');
});

test('start revision mismatch preserves the exact frame request for retry', async () => {
  const session = vi.fn(async r => result(r.request.args.job_id, { source_revision: 8 }));
  const jobs = new FramePreviewJobsController({ session }, 's');
  await jobs.start(snapshot, 159);
  const args = jobs.pending;
  expect(args).toMatchObject({ expected_revision: 7, frame_index: 159 });
  expect(jobs.selected).toBeNull(); expect(jobs.jobs).toEqual([]);
  expect(jobs.error).toContain('一致しません');
  await jobs.start({ ...snapshot, revision: 8 }, 276);
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(args);
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.source_revision).toBe(7);
});
