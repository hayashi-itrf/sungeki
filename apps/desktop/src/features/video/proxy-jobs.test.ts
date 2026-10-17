import { expect, test, vi } from 'vitest';
import type { ProxyJob, SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { ProxyJobsController } from './proxy-jobs.svelte';
const snapshot = { session_id: 's', revision: 2, project: { assets: [{ id: 'a', probe: { type: 'video' }, locator: { type: 'absolute' } }] } } as SessionSnapshot;
function result(id: string, changes: Partial<ProxyJob> = {}): SessionResponse {
  return { type: 'proxy_job', job: { job_id: id, session_id: 's', source_revision: 2, content_hash: 'hash',
    asset_ids: ['a'], max_edge: 640, phase: 'queued', progress: null, completed: [], error: null, stale: false, ...changes } };
}
test('uncertain start retains exact identity, target copy and preset for retry', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'unknown' });
  const jobs = new ProxyJobsController({ session }, 's');
  const targets = ['a']; await jobs.start(snapshot, targets, 'compact');
  targets.push('other');
  const pending = jobs.pending!;
  expect(pending.asset_ids).toEqual(['a']);
  await jobs.start({ ...snapshot, revision: 3 }, ['a'], 'standard');
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request).toEqual({ type: 'start', args: pending });
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.job_id).toBe(pending.job_id);
  expect(snapshot.revision).toBe(2);
});
test('mismatched target, preset, revision and session stay pending; explicit rejection clears it', async () => {
  for (const changes of [{ asset_ids: ['other'] }, { max_edge: 1280 }, { source_revision: 3 }, { session_id: 'other' }]) {
    const session = vi.fn(async r => result(r.request.args.job_id, changes));
    const jobs = new ProxyJobsController({ session }, 's');
    await jobs.start(snapshot, ['a'], 'compact');
    expect(jobs.pending).not.toBeNull(); expect(jobs.selected).toBeNull();
    session.mockRejectedValueOnce({ code: 'CONFLICT', message: 'changed' });
    await jobs.retry(); expect(jobs.pending).toBeNull();
  }
});
test('invalid material requests never reach the host', async () => {
  const session = vi.fn(); const jobs = new ProxyJobsController({ session }, 's');
  for (const ids of [[], ['a', 'a'], ['missing'], Array(65).fill('a')]) await jobs.start(snapshot, ids, 'compact');
  await jobs.start({ ...snapshot, session_id: 'other' }, ['a'], 'compact');
  expect(session).not.toHaveBeenCalled();
});
test('history pagination, selection and cancellation preserve stale and error results', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `j${i}`);
  const session = vi.fn(async r => r.request.type === 'list'
    ? { type: 'proxy_jobs', job_ids: r.request.after ? ['next'] : ids } as SessionResponse
    : result(r.request.job_id, { phase: r.request.type === 'cancel' ? 'cancelling' : 'running', stale: true }));
  const jobs = new ProxyJobsController({ session }, 's');
  await jobs.list(); expect(jobs.more).toBe(true);
  await jobs.list(true); expect(jobs.jobs).toHaveLength(13); expect(jobs.more).toBe(false);
  await jobs.refresh('next'); await jobs.cancel();
  expect(jobs.selected).toMatchObject({ job_id: 'next', phase: 'cancelling', stale: true });
});
test('disposing a panel ignores late replies and stops new work', async () => {
  let resolve!: (value: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const jobs = new ProxyJobsController({ session }, 's');
  const start = jobs.start(snapshot, ['a'], 'compact');
  const id = jobs.pending!.job_id;
  jobs.dispose(); resolve(result(id)); await start;
  await jobs.list(); await jobs.refresh(id); await jobs.retry();
  expect(session).toHaveBeenCalledTimes(1); expect(jobs.selected).toBeNull();
});
test('cache removal requires terminal target and validates the response before reporting success', async () => {
  const session = vi.fn(async () => ({ type: 'proxy_removed', job_id: 'j', asset_id: 'a', removed: true } as SessionResponse));
  const jobs = new ProxyJobsController({ session }, 's');
  jobs.selected = (result('j', { phase: 'running' }) as { type: 'proxy_job'; job: ProxyJob }).job;
  expect(await jobs.remove('a')).toBe(false); expect(session).not.toHaveBeenCalled();
  jobs.selected.phase = 'succeeded';
  expect(await jobs.remove('other')).toBe(false);
  expect(await jobs.remove('a')).toBe(true);
  expect(jobs.notice).toContain('生成履歴は残ります');
  expect(jobs.selected.phase).toBe('succeeded');
  session.mockResolvedValueOnce({ type: 'proxy_removed', job_id: 'wrong', asset_id: 'a', removed: true });
  expect(await jobs.remove('a')).toBe(false); expect(jobs.error).toContain('一致しません');
  session.mockRejectedValueOnce(new Error('unknown outcome'));
  expect(await jobs.remove('a')).toBe(false);
  session.mockResolvedValueOnce({ type: 'proxy_removed', job_id: 'j', asset_id: 'a', removed: false });
  expect(await jobs.remove('a')).toBe(true);
});
