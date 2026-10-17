import { expect, test, vi } from 'vitest';
import type { AudioPreviewJob, SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { AudioPreviewJobsController } from './preview-jobs.svelte';
const snapshot = { session_id: 's', revision: 7 } as SessionSnapshot;
function result(id: string, overrides: Partial<AudioPreviewJob> = {}): SessionResponse {
  return { type: 'audio_preview_job', job: { job_id: id, session_id: 's', source_revision: 7,
    content_hash: 'hash', phase: 'queued', progress: null, output: null, error: null, stale: false, ...overrides } };
}

test('uncertain start blocks new requests and retries the identical revision and job ID', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'unknown' });
  const jobs = new AudioPreviewJobsController({ session }, 's');
  await jobs.start(snapshot);
  const original = jobs.pending;
  expect(original).toMatchObject({ session_id: 's', expected_revision: 7 });
  await jobs.start({ ...snapshot, revision: 8 });
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(original);
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.job_id).toBe(original!.job_id);
});

test('mismatched response remains uncertain while explicit rejection allows corrected submission', async () => {
  const session = vi.fn().mockResolvedValue(result('wrong'));
  const jobs = new AudioPreviewJobsController({ session }, 's');
  await jobs.start(snapshot);
  expect(jobs.pending).not.toBeNull(); expect(jobs.selected).toBeNull();
  session.mockRejectedValueOnce({ code: 'CAPABILITY_UNAVAILABLE', message: 'not configured' });
  await jobs.retry(); expect(jobs.pending).toBeNull();
  session.mockImplementation(async r => result(r.request.args.job_id, { session_id: 'other' }));
  await jobs.start(snapshot); expect(jobs.error).toContain('一致しません');
});

test('wrong source revision preserves the pending request and previous result until an identical retry succeeds', async () => {
  const session = vi.fn(async r => result(r.request.job_id));
  const jobs = new AudioPreviewJobsController({ session }, 's');
  await jobs.refresh('previous');
  const previous = jobs.selected;
  session.mockImplementation(async r => result(r.request.args.job_id, { source_revision: 8 }));
  await jobs.start(snapshot);
  const original = jobs.pending;
  expect(original).toMatchObject({ expected_revision: 7 });
  expect(jobs.error).toContain('一致しません');
  expect(jobs.selected).toEqual(previous);
  expect(jobs.jobs).toEqual([previous]);
  await jobs.start({ ...snapshot, revision: 8 });
  expect(session).toHaveBeenCalledTimes(2);
  session.mockImplementation(async r => result(r.request.args.job_id, { stale: true }));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(original);
  expect(jobs.pending).toBeNull();
  expect(jobs.error).toBe('');
  expect(jobs.selected).toMatchObject({ job_id: original!.job_id, source_revision: 7, stale: true });
  expect(jobs.jobs).toHaveLength(2);
});

test('pagination and cancellation operate on the selected job, preserving failure and stale state', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `job-${i}`);
  const session = vi.fn(async r => r.request.type === 'list'
    ? { type: 'audio_preview_jobs', job_ids: r.request.after ? ['next'] : ids } as SessionResponse
    : result(r.request.job_id, { phase: r.request.type === 'cancel' ? 'cancelled' : 'running', stale: true }));
  const jobs = new AudioPreviewJobsController({ session }, 's');
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
  const jobs = new AudioPreviewJobsController({ session }, 's');
  const started = jobs.start(snapshot); const id = jobs.pending!.job_id;
  jobs.dispose(); resolve(result(id)); await started;
  expect(jobs.selected).toBeNull();
  await jobs.refresh(id); await jobs.list(); await jobs.start(snapshot);
  expect(session).toHaveBeenCalledTimes(1);
});
