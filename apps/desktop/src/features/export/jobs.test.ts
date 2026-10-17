import { expect, test, vi } from 'vitest';
import type { RenderJob, SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { RenderJobsController } from './jobs.svelte';
const snapshot = { session_id: 's', revision: 7 } as SessionSnapshot;
function result(id: string, overrides: Partial<RenderJob> = {}): SessionResponse {
  return { type: 'render_job', job: { purpose: 'final', job_id: id, session_id: 's', source_revision: 7,
    content_hash: 'hash', phase: 'queued', progress: null, output: null, error: null, stale: false, ...overrides } };
}

test('uncertain start blocks new requests and retries the identical revision and job ID', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'unknown' });
  const jobs = new RenderJobsController({ session }, 's');
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
  const jobs = new RenderJobsController({ session }, 's');
  await jobs.start(snapshot);
  expect(jobs.pending).not.toBeNull(); expect(jobs.selected).toBeNull();
  session.mockRejectedValueOnce({ code: 'CAPABILITY_UNAVAILABLE', message: 'not configured' });
  await jobs.retry(); expect(jobs.pending).toBeNull();
  session.mockImplementation(async r => result(r.request.args.job_id, { session_id: 'other' }));
  await jobs.start(snapshot); expect(jobs.error).toContain('一致しません');
});

test('pagination and cancellation operate on the selected job, preserving failure and stale state', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `job-${i}`);
  const session = vi.fn(async r => r.request.type === 'list'
    ? { type: 'render_jobs', job_ids: r.request.after ? ['next'] : ids } as SessionResponse
    : result(r.request.job_id, { phase: r.request.type === 'cancel' ? 'cancelled' : 'running', stale: true }));
  const jobs = new RenderJobsController({ session }, 's');
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
  const jobs = new RenderJobsController({ session }, 's');
  const started = jobs.start(snapshot); const id = jobs.pending!.job_id;
  jobs.dispose(); resolve(result(id)); await started;
  expect(jobs.selected).toBeNull();
  await jobs.refresh(id); await jobs.list(); await jobs.start(snapshot);
  expect(session).toHaveBeenCalledTimes(1);
});


test('preview retry preserves its purpose and rejects a final-output response', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE' });
  const jobs = new RenderJobsController({ session }, 's', 'editing_preview');
  await jobs.start(snapshot);
  const args = jobs.pending;
  expect(session.mock.lastCall![0].request.type).toBe('preview');
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(jobs.pending).toBe(args); expect(jobs.error).toContain('用途');
  session.mockImplementation(async r => result(r.request.args.job_id, { purpose: 'editing_preview' }));
  await jobs.retry();
  expect(session.mock.lastCall![0].request).toEqual({ type: 'preview', args });
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.purpose).toBe('editing_preview');
});

test('unreadable history entries do not hide good outputs or lose pagination, and can recover individually', async () => {
  let failed = true;
  const ids = Array.from({ length: 12 }, (_, i) => `job-${i}`);
  const session = vi.fn(async r => {
    if (r.request.type === 'list') return { type: 'render_jobs', job_ids: r.request.after ? ['last'] : ids } as SessionResponse;
    if (failed && r.request.job_id === 'job-1') throw { message: '出力記録を読めません' };
    return result(r.request.job_id);
  });
  const jobs = new RenderJobsController({ session }, 's');
  await jobs.list();
  expect(jobs.jobs).toHaveLength(11); expect(jobs.unavailable).toEqual([{ id: 'job-1', message: '出力記録を読めません' }]);
  expect(jobs.error).toBe(''); expect(jobs.more).toBe(true);
  await jobs.list(true); expect(jobs.jobs).toHaveLength(12); expect(jobs.more).toBe(false);
  expect(session).toHaveBeenCalledWith({ type: 'render_job', request: { type: 'list', session_id: 's', after: 'job-11', limit: 12 } });
  failed = false; await jobs.refresh('job-1');
  expect(jobs.unavailable).toEqual([]); expect(jobs.jobs).toHaveLength(13);
  expect(jobs.selected?.job_id).toBe('job-1');
});
test('failed observation keeps selected output and its warning until a successful read of that output', async () => {
  const session = vi.fn().mockResolvedValueOnce(result('older', { phase: 'running' }))
    .mockRejectedValueOnce({ message: '通信失敗' })
    .mockResolvedValueOnce({ type: 'render_jobs', job_ids: ['newer'] }).mockResolvedValueOnce(result('newer'))
    .mockResolvedValueOnce(result('older', { phase: 'succeeded' }));
  const jobs = new RenderJobsController({ session }, 's');
  await jobs.refresh('older'); await jobs.refresh();
  expect(jobs.selected?.phase).toBe('running'); expect(jobs.unavailable[0].id).toBe('older');
  await jobs.list(); expect(jobs.unavailable[0].id).toBe('older');
  await jobs.refresh(); expect(jobs.selected?.phase).toBe('succeeded'); expect(jobs.unavailable).toEqual([]);
});
test('all failed entries are not an empty history; failed listing preserves prior observations', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'render_jobs', job_ids: ['bad'] })
    .mockRejectedValueOnce({ message: 'broken' }).mockRejectedValueOnce({ message: 'offline' });
  const jobs = new RenderJobsController({ session }, 's');
  await jobs.list(); expect(jobs.jobs).toEqual([]); expect(jobs.unavailable).toHaveLength(1);
  await jobs.list(); expect(jobs.unavailable).toEqual([{ id: 'bad', message: 'broken' }]);
  expect(jobs.error).toBe('offline');
});

test.each(['final', 'editing_preview'] as const)('%s start rejects another source revision without losing the pending identity or previous output', async purpose => {
  const session = vi.fn(async r => result(r.request.job_id, { purpose }));
  const jobs = new RenderJobsController({ session }, 's', purpose);
  await jobs.refresh('previous');
  const previous = jobs.selected;
  session.mockImplementation(async r => result(r.request.args.job_id, { purpose, source_revision: 8 }));
  await jobs.start(snapshot);
  const args = jobs.pending;
  expect(args).toMatchObject({ expected_revision: 7 });
  expect(jobs.error).toContain('一致しません');
  expect(jobs.selected).toEqual(previous);
  expect(jobs.jobs).toEqual([previous]);
  await jobs.start({ ...snapshot, revision: 8 });
  expect(session).toHaveBeenCalledTimes(2);
  session.mockImplementation(async r => result(r.request.args.job_id, { purpose, stale: true }));
  await jobs.retry();
  expect(session.mock.lastCall![0].request).toEqual({ type: purpose === 'final' ? 'start' : 'preview', args });
  expect(session.mock.lastCall![0].request.args).toBe(args);
  expect(jobs.pending).toBeNull();
  expect(jobs.error).toBe('');
  expect(jobs.selected).toMatchObject({ job_id: args!.job_id, source_revision: 7, purpose, stale: true });
});

test('background progress preserves visible history and does not toggle foreground busy state', async () => {
  let finish!: (value: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(resolve => { finish = resolve; }));
  const jobs = new RenderJobsController({ session }, 's');
  const first = result('first'), second = result('second');
  if (first.type !== 'render_job' || second.type !== 'render_job') throw new Error('fixture');
  jobs.jobs = [first.job, second.job]; jobs.selected = second.job;
  const pending = jobs.poll(); await jobs.poll();
  expect(session).toHaveBeenCalledTimes(1);
  expect(jobs.busy).toBe(false); expect(jobs.jobs.map(j => j.job_id)).toEqual(['first', 'second']);
  finish(result('second', { phase: 'running' })); await pending;
  expect(jobs.jobs.map(j => j.job_id)).toEqual(['first', 'second']);
  expect(jobs.selected?.phase).toBe('running'); expect(jobs.busy).toBe(false);
});

test('late poll cannot overwrite cancellation and polling failure retains last known rows', async () => {
  let finish!: (value: SessionResponse) => void;
  const session = vi.fn().mockImplementationOnce(() => new Promise<SessionResponse>(resolve => { finish = resolve; }))
    .mockResolvedValueOnce(result('one', { phase: 'cancelled' })).mockRejectedValueOnce(new Error('offline'));
  const jobs = new RenderJobsController({ session }, 's');
  const initial = result('one', { phase: 'running' }); if (initial.type !== 'render_job') throw new Error('fixture');
  jobs.jobs = [initial.job]; jobs.selected = initial.job;
  const pending = jobs.poll(); await jobs.cancel(); finish(initial); await pending;
  expect(jobs.selected?.phase).toBe('cancelled');
  await jobs.poll(); expect(jobs.jobs[0].phase).toBe('cancelled');
  expect(jobs.unavailable).toEqual([{ id: 'one', message: 'offline' }]); expect(jobs.busy).toBe(false);
});
