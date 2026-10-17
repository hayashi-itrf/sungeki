import { expect, test, vi } from 'vitest';
import type { ReviewJob, SessionSnapshot } from '@sungeki/api-client';
import { ReviewController } from './controller.svelte';
export function job(id = 'job'): ReviewJob {
  return { job_id: id, session_id: 'session', source_revision: 2, content_hash: 'hash', phase: 'queued', progress: null, report: null, error: null, stale: false, asset_freshness: { status: 'unchecked', last_known_changed: false, checking: false, checked_at_ms: null, message: null } };
}
const snapshot = { session_id: 'session', revision: 2 } as SessionSnapshot;
test('uncertain start retries the identical frozen request and blocks a second start', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '通信切断' }).mockImplementation(async request => ({ type: 'review_job', job: job(request.job_id) }));
  const reviews = new ReviewController({ session }, 'session');
  await reviews.start(snapshot);
  const captured = reviews.pending;
  expect(captured?.expected_revision).toBe(2);
  await reviews.start({ ...snapshot, revision: 3 });
  expect(session).toHaveBeenCalledTimes(1);
  await reviews.retry();
  expect(session.mock.calls[1][0]).toBe(captured);
  expect(reviews.pending).toBeNull();
  expect(reviews.selected?.job_id).toBe(captured?.job_id);
  expect(reviews.jobs).toHaveLength(1);
});
test('known conflict clears uncertainty while preserving the error', async () => {
  const reviews = new ReviewController({ session: vi.fn().mockRejectedValue({ code: 'CONFLICT', message: '作品が変更されました' }) }, 'session');
  await reviews.start(snapshot);
  expect(reviews.pending).toBeNull();
  expect(reviews.error).toBe('作品が変更されました');
  expect(reviews.selected).toBeNull();
});
test('late responses from a disposed panel are not shown', async () => {
  let resolve!: (value: unknown) => void;
  const session = vi.fn().mockImplementation(() => new Promise(r => { resolve = r; }));
  const reviews = new ReviewController({ session }, 'session');
  const request = reviews.refresh('job');
  reviews.dispose();
  resolve({ type: 'review_job', job: job() });
  await request;
  expect(reviews.selected).toBeNull();
});
test('list failure keeps the prior results and reports the failure', async () => {
  const session = vi.fn().mockRejectedValueOnce({ message: '結果を取得できません' });
  const reviews = new ReviewController({ session }, 'session');
  reviews.jobs = [job()];
  await reviews.list();
  expect(reviews.jobs.map(j => j.job_id)).toEqual(['job']);
  expect(reviews.error).toBe('結果を取得できません');
});
test('cancel preserves cancelling until host confirms terminal cleanup', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'review_job', job: { ...job(), phase: 'cancelling' } }).mockResolvedValueOnce({ type: 'review_job', job: { ...job(), phase: 'cancelled' } });
  const reviews = new ReviewController({ session }, 'session');
  reviews.selected = job();
  await reviews.cancel();
  expect(reviews.selected?.phase).toBe('cancelling');
  await reviews.refresh();
  expect(reviews.selected?.phase).toBe('cancelled');
});

test('unavailable work storage releases pending start so the dialog can close and a later start can proceed', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'CAPABILITY_UNAVAILABLE', message: 'SSDを確認して再起動してください', details: { job_created: 'false' } })
    .mockImplementation(async request => ({ type: 'review_job', job: job(request.job_id) }));
  const reviews = new ReviewController({ session }, 'session');
  await reviews.start(snapshot);
  const rejectedId = session.mock.calls[0][0].job_id;
  expect(reviews.pending).toBeNull();
  expect(reviews.busy).toBe(false);
  expect(reviews.error).toContain('SSD');
  await reviews.start(snapshot);
  expect(session.mock.calls[1][0].job_id).not.toBe(rejectedId);
  expect(reviews.selected?.job_id).toBe(session.mock.calls[1][0].job_id);
  expect(reviews.error).toBe('');
});

test('an uncertain I/O failure still retains the same start request', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'IO_ERROR', message: '結果の取得に失敗' })
    .mockImplementation(async request => ({ type: 'review_job', job: job(request.job_id) }));
  const reviews = new ReviewController({ session }, 'session');
  await reviews.start(snapshot);
  const request = reviews.pending;
  expect(request).not.toBeNull();
  await reviews.retry();
  expect(session.mock.calls[1][0]).toBe(request);
  expect(reviews.pending).toBeNull();
});
test.each([0, 5, 25])('uncertain review keeps all original sampling conditions including margin %i', async margin => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE' }).mockImplementation(async request => ({ type: 'review_job', job: job(request.job_id) }));
  const reviews = new ReviewController({ session }, 'session');
  const sampling = { requested_times_us: [3500000], max_frames: 8, text_safe_margin_percent: margin };
  await reviews.start(snapshot, sampling);
  sampling.requested_times_us[0] = 9000000; sampling.max_frames = 1; sampling.text_safe_margin_percent = 12;
  await reviews.retry();
  expect(session.mock.calls[1][0].sampling).toEqual({ requested_times_us: [3500000], max_frames: 8, text_safe_margin_percent: margin });
});

test('one unreadable report does not hide healthy reports or prevent the next page; single retry recovers it', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `job-${i}`);
  let failed = true;
  const session = vi.fn().mockImplementation(async request => {
    if (request.type === 'list_review_jobs') return { type: 'review_jobs', job_ids: request.after ? ['last'] : ids };
    if (request.job_id === 'job-1' && failed) throw { message: '破損した結果' };
    return { type: 'review_job', job: job(request.job_id) };
  });
  const reviews = new ReviewController({ session }, 'session');
  reviews.selected = job('job-1');
  await reviews.list();
  expect(reviews.jobs).toHaveLength(11);
  expect(reviews.unavailable).toEqual([{ id: 'job-1', message: '破損した結果' }]);
  expect(reviews.selected?.job_id).toBe('job-1');
  expect(reviews.after).toBe('job-11'); expect(reviews.more).toBe(true);
  await reviews.list(true);
  expect(session).toHaveBeenCalledWith({ type: 'list_review_jobs', session_id: 'session', after: 'job-11', limit: 12 });
  expect(reviews.jobs).toHaveLength(12); expect(reviews.more).toBe(false);
  expect(reviews.unavailable).toHaveLength(1);
  failed = false; await reviews.refresh('job-1');
  expect(reviews.jobs).toHaveLength(13); expect(reviews.unavailable).toEqual([]);
  expect(reviews.selected?.job_id).toBe('job-1');
});
test('all unreadable reports remain distinguishable from an empty history and mismatched IDs are rejected', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'review_jobs', job_ids: ['requested'] })
    .mockResolvedValue({ type: 'review_job', job: job('different') });
  const reviews = new ReviewController({ session }, 'session');
  await reviews.list();
  expect(reviews.jobs).toEqual([]); expect(reviews.unavailable[0].id).toBe('requested');
  expect(reviews.unavailable[0].message).toContain('一致しません');
  await reviews.refresh('requested'); expect(reviews.selected).toBeNull();
});
test('disposing a list in flight prevents remaining batches and state updates', async () => {
  let release!: (value: unknown) => void;
  const session = vi.fn().mockImplementation(async request => {
    if (request.type === 'list_review_jobs') return { type: 'review_jobs', job_ids: ['a', 'b', 'c', 'd'] };
    if (request.job_id === 'a') return new Promise(resolve => { release = resolve; });
    return { type: 'review_job', job: job(request.job_id) };
  });
  const reviews = new ReviewController({ session }, 'session');
  const pending = reviews.list();
  await vi.waitFor(() => expect(release).toBeDefined());
  reviews.dispose(); release({ type: 'review_job', job: job('a') }); await pending;
  expect(session).toHaveBeenCalledTimes(4); expect(reviews.jobs).toEqual([]); expect(reviews.unavailable).toEqual([]);
});

test('selected report read failure stays explicit until that report is successfully refreshed', async () => {
  const session = vi.fn().mockRejectedValueOnce({ message: '接続が切れました' })
    .mockResolvedValueOnce({ type: 'review_jobs', job_ids: ['newer'] })
    .mockResolvedValueOnce({ type: 'review_job', job: job('newer') })
    .mockResolvedValueOnce({ type: 'review_job', job: { ...job('older'), phase: 'succeeded' } });
  const reviews = new ReviewController({ session }, 'session');
  reviews.selected = job('older'); reviews.jobs = [job('older')];
  await reviews.refresh();
  expect(reviews.selected?.phase).toBe('queued');
  expect(reviews.unavailable).toEqual([{ id: 'older', message: '接続が切れました' }]);
  await reviews.list();
  expect(reviews.unavailable[0].id).toBe('older');
  expect(reviews.selected?.job_id).toBe('older');
  await reviews.refresh();
  expect(reviews.unavailable).toEqual([]);
  expect(reviews.selected?.phase).toBe('succeeded');
});
test('late read failure after disposal cannot add an error or trigger later work', async () => {
  let reject!: (error: unknown) => void;
  const session = vi.fn().mockImplementation(() => new Promise((_, fail) => { reject = fail; }));
  const reviews = new ReviewController({ session }, 'session');
  const pending = reviews.refresh('job'); reviews.dispose(); reject({ message: 'late failure' }); await pending;
  expect(reviews.error).toBe(''); expect(reviews.unavailable).toEqual([]);
  await reviews.list(); await reviews.start(snapshot);
  expect(session).toHaveBeenCalledTimes(1); expect(reviews.pending).toBeNull();
});

test('a review of another revision remains uncertain and retains the original sampling for retry', async () => {
  const session = vi.fn().mockImplementation(async r => ({ type: 'review_job', job: { ...job(r.job_id), source_revision: 3 } }));
  const reviews = new ReviewController({ session }, 'session');
  await reviews.start(snapshot, { requested_times_us: [1000000], max_frames: 2 });
  const request = reviews.pending;
  expect(request?.expected_revision).toBe(2);
  expect(reviews.selected).toBeNull(); expect(reviews.jobs).toEqual([]);
  expect(reviews.error).toContain('一致しません');
  await reviews.start({ ...snapshot, revision: 3 });
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => ({ type: 'review_job', job: job(r.job_id) }));
  await reviews.retry();
  expect(session.mock.lastCall![0]).toBe(request);
  expect(reviews.pending).toBeNull(); expect(reviews.selected?.source_revision).toBe(2);
});
