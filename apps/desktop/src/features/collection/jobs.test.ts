import { expect, test, vi } from 'vitest';
import type { CollectionJob, SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { CollectionJobsController } from './jobs.svelte';
const snapshot = { session_id: 's', revision: 7 } as SessionSnapshot;
function result(id: string, overrides: Partial<CollectionJob> = {}): SessionResponse {
  return { type: 'collection_job', job: { job_id: id, session_id: 's', source_revision: 7,
    content_hash: 'hash', include_checkpoints: false, checkpoint_count: 0, phase: 'queued', destination: '/tmp/collection', dry_run: true, plan: null, output: null, error: null, stale: false, ...overrides } };
}

test('uncertain start blocks new requests and retries the identical revision and job ID', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'unknown' });
  const jobs = new CollectionJobsController({ session }, 's');
  await jobs.start(snapshot, '/tmp/collection', true);
  const original = jobs.pending;
  expect(original).toMatchObject({ session_id: 's', expected_revision: 7 });
  await jobs.start({ ...snapshot, revision: 8 }, '/tmp/changed', false);
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(original);
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.job_id).toBe(original!.job_id);
});

test('mismatched response remains uncertain while explicit rejection allows corrected submission', async () => {
  const session = vi.fn().mockResolvedValue(result('wrong'));
  const jobs = new CollectionJobsController({ session }, 's');
  await jobs.start(snapshot, '/tmp/collection', true);
  expect(jobs.pending).not.toBeNull(); expect(jobs.selected).toBeNull();
  session.mockRejectedValueOnce({ code: 'CAPABILITY_UNAVAILABLE', message: 'not configured' });
  await jobs.retry(); expect(jobs.pending).toBeNull();
  session.mockImplementation(async r => result(r.request.args.job_id, { session_id: 'other' }));
  await jobs.start(snapshot, '/tmp/collection', true); expect(jobs.error).toContain('一致しません');
});

test('pagination and cancellation operate on the selected job, preserving failure and stale state', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `job-${i}`);
  const session = vi.fn(async r => r.request.type === 'list'
    ? { type: 'collection_jobs', job_ids: r.request.after ? ['next'] : ids } as SessionResponse
    : result(r.request.job_id, { phase: r.request.type === 'cancel' ? 'cancelled' : 'running', stale: true }));
  const jobs = new CollectionJobsController({ session }, 's');
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
  const jobs = new CollectionJobsController({ session }, 's');
  const started = jobs.start(snapshot, '/tmp/collection', true); const id = jobs.pending!.job_id;
  jobs.dispose(); resolve(result(id)); await started;
  expect(jobs.selected).toBeNull();
  await jobs.refresh(id); await jobs.list(); await jobs.start(snapshot, '/tmp/collection', true);
  expect(session).toHaveBeenCalledTimes(1);
});

test('planning mode and revision are checked before resolving an uncertain start', async () => {
  const session = vi.fn(async r => result(r.request.args.job_id, { dry_run: false }));
  const jobs = new CollectionJobsController({ session }, 's');
  await jobs.start(snapshot, '/tmp/collection', true);
  expect(jobs.pending).toMatchObject({ destination: '/tmp/collection', dry_run: true });
  expect(jobs.error).toContain('条件が一致');
  expect(jobs.selected).toBeNull();
});

test('plan page is bounded, validated, and cleared when another job is selected', async () => {
  const plan = { checkpoint_count: 0, source_count: 50, unique_files: 1, estimated_asset_bytes: 42, sources_ready: true, destination_exists: false, excluded_dependencies: [] };
  const session = vi.fn(async r => r.request.type === 'read_plan'
    ? { type: 'collection_plan_page', page: { offset: r.request.args.offset, total: 50, items: Array.from({ length: 25 }, (_, i) => ({ checkpoint_id: null, asset_id: String(i), source_path: '/tmp/a', source_path_truncated: false, destination_path: 'assets/a.png', sha256: 'hash', byte_length: 42, status: 'verified', detail: null, conflicting_metadata: false })) } } as SessionResponse
    : result(r.request.job_id, { phase: 'succeeded', plan }));
  const jobs = new CollectionJobsController({ session }, 's');
  await jobs.refresh('a'); await jobs.loadPage(25);
  expect(session.mock.lastCall![0].request.args).toMatchObject({ job_id: 'a', offset: 25, limit: 25 });
  expect(jobs.page?.offset).toBe(25);
  await jobs.refresh('b'); expect(jobs.page).toBeNull();
  session.mockResolvedValueOnce({ type: 'collection_plan_page', page: { offset: 0, total: 99, items: [] } });
  await jobs.loadPage(); expect(jobs.page).toBeNull(); expect(jobs.error).toContain('一致しません');
});

test('history selection is frozen across uncertain requests and retries', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'unknown' });
  const jobs = new CollectionJobsController({ session }, 's');
  await jobs.start(snapshot, '/tmp/collection', true, true);
  const original = jobs.pending;
  expect(original?.include_checkpoints).toBe(true);
  await jobs.start(snapshot, '/tmp/collection', true, false);
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id, { include_checkpoints: true, checkpoint_count: 2 }));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(original);
  expect(jobs.selected?.checkpoint_count).toBe(2);
  expect(jobs.pending).toBeNull();
});

test('definitive history limit rejection allows a corrected new request', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'VALIDATION_FAILED', message: '保存時点は最大127件です' });
  const jobs = new CollectionJobsController({ session }, 's');
  await jobs.start(snapshot, '/tmp/collection', true, true);
  const rejected = session.mock.calls[0]![0].request.args;
  expect(jobs.pending).toBeNull();
  expect(jobs.error).toContain('127');
  session.mockImplementation(async r => result(r.request.args.job_id, { include_checkpoints: false }));
  await jobs.start(snapshot, '/tmp/collection', true, false);
  const corrected = session.mock.lastCall![0].request.args;
  expect(corrected.job_id).not.toBe(rejected.job_id);
  expect(corrected.include_checkpoints).toBe(false);
  expect(jobs.pending).toBeNull(); expect(jobs.error).toBe('');
});
