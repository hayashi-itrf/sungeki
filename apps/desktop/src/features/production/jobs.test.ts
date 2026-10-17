import { expect, test, vi } from 'vitest';
import type { BuildVideoJob, SessionResponse } from '@sungeki/api-client';
import { BuildJobsController } from './jobs.svelte';
function result(id: string, changes: Partial<BuildVideoJob> = {}): SessionResponse {
  return { type: 'build_video_job', job: { job_id: id, session_id: `session-${id}`, title: id,
    phase: 'running', progress: { stage: '映像・音声を出力して検証中', fraction: null },
    completed_steps: 10, total_steps: 12, child_job_id: 'child', source_revision: 6,
    output: null, error: null, stale: false, review_asset_freshness: null, ...changes } };
}
test('global history pages preserve selection and cancel only the selected production', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `id-${i}`);
  const session = vi.fn(async r => r.request.type === 'list'
    ? { type: 'build_video_jobs', job_ids: r.request.after ? ['next'] : ids } as SessionResponse
    : result(r.request.job_id, { phase: r.request.type === 'cancel' ? 'cancelling' : 'running' }));
  const jobs = new BuildJobsController({ session });
  await jobs.list(); expect(jobs.more).toBe(true); expect(jobs.selected).toBeNull();
  await jobs.list(true); expect(jobs.jobs).toHaveLength(13); expect(jobs.more).toBe(false);
  expect(session.mock.calls[13]![0].request.after).toBe('id-11');
  await jobs.refresh('next'); await jobs.cancel();
  expect(jobs.selected).toMatchObject({ job_id: 'next', phase: 'cancelling' });
  expect(session.mock.lastCall![0]).toEqual({ type: 'build_video', request: { type: 'cancel', job_id: 'next' } });
});
test('unconfirmed cancellation retains the observed job and permits a read-only refresh', async () => {
  const session = vi.fn().mockResolvedValueOnce(result('one')).mockRejectedValueOnce(new Error('接続を確認してください'));
  const jobs = new BuildJobsController({ session });
  await jobs.refresh('one'); await jobs.cancel();
  expect(jobs.error).toContain('接続'); expect(jobs.selected?.phase).toBe('running');
  session.mockResolvedValueOnce(result('one', { phase: 'cancelled' }));
  await jobs.refresh(); expect(jobs.selected?.phase).toBe('cancelled');
  await jobs.cancel(); expect(session).toHaveBeenCalledTimes(3);
});
test('mismatched and duplicate replies do not replace known history', async () => {
  const session = vi.fn().mockResolvedValueOnce(result('one')).mockResolvedValueOnce(result('wrong'));
  const jobs = new BuildJobsController({ session });
  await jobs.refresh('one'); await jobs.refresh('two');
  expect(jobs.error).toContain('一致'); expect(jobs.selected?.job_id).toBe('one');
  session.mockResolvedValueOnce({ type: 'build_video_jobs', job_ids: ['one', 'one'] });
  await jobs.list(); expect(jobs.error).toContain('不正'); expect(jobs.jobs).toHaveLength(1);
});
test('disposal suppresses late responses and new requests', async () => {
  let resolve!: (r: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const jobs = new BuildJobsController({ session });
  const request = jobs.refresh('one'); jobs.dispose(); resolve(result('one')); await request;
  expect(jobs.selected).toBeNull(); await jobs.list(); expect(session).toHaveBeenCalledTimes(1);
});

test('uncertain start freezes recipe and ID; saved-but-queued stays retryable', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'CONFLICT', details: { job_created: 'true' }, message: '別制作中' });
  const jobs = new BuildJobsController({ session });
  const input = { recipe: { title: '最初の台本', scenes: [{ name: '元の場面' }] }, project_path: '/work/new.json', base_url: 'http://localhost:50021', engine_version: '1' } as unknown as import('@sungeki/api-client').StartBuildVideoArgs;
  await jobs.start(input); const original = JSON.stringify(jobs.pending); const id = jobs.pending!.job_id;
  input.recipe.title = '変更した台本'; input.recipe.scenes[0]!.name = '変更した場面';
  await jobs.start(input); await jobs.list(); expect(session).toHaveBeenCalledTimes(1);
  session.mockResolvedValueOnce(result(id, { phase: 'queued' }));
  await jobs.checkPending(); expect(JSON.stringify(jobs.pending)).toBe(original);
  session.mockResolvedValueOnce(result(id));
  await jobs.retry(); expect(jobs.pending).toBeNull();
  expect(JSON.stringify(session.mock.lastCall![0].request.args)).toBe(original);
});
test('unknown start stays unresolved after NotFound; observed running clears uncertainty', async () => {
  const session = vi.fn().mockRejectedValueOnce(new Error('応答不明'));
  const jobs = new BuildJobsController({ session });
  await jobs.start({ recipe: {} } as import('@sungeki/api-client').StartBuildVideoArgs);
  const id = jobs.pending!.job_id;
  session.mockRejectedValueOnce({ code: 'NOT_FOUND', message: 'まだ見つかりません' });
  await jobs.checkPending(); expect(jobs.pending?.job_id).toBe(id);
  session.mockResolvedValueOnce(result(id)); await jobs.checkPending();
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.job_id).toBe(id);
});
test('confirmed initial rejection unlocks input; rejection after a lost reply does not discard the ID', async () => {
  const session = vi.fn(async r => { throw { message: '未作成', details: { job_created: 'false', job_id: r.request.args.job_id } }; });
  const jobs = new BuildJobsController({ session });
  const input = { recipe: {} } as import('@sungeki/api-client').StartBuildVideoArgs;
  await jobs.start(input); expect(jobs.pending).toBeNull();
  session.mockRejectedValueOnce(new Error('応答喪失')); await jobs.start(input);
  const id = jobs.pending!.job_id; await jobs.retry(); expect(jobs.pending?.job_id).toBe(id);
});

test('unreadable history does not hide healthy jobs or consume the next-page cursor', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `id-${i}`);
  let broken = true;
  const session = vi.fn(async r => {
    if (r.request.type === 'list') return { type: 'build_video_jobs', job_ids: r.request.after ? ['last'] : ids } as SessionResponse;
    if (r.request.job_id === 'id-4' && broken) throw { message: '制作記録を読み込めません' };
    return result(r.request.job_id);
  });
  const jobs = new BuildJobsController({ session });
  await jobs.refresh('id-4');
  broken = false; await jobs.refresh('id-4'); broken = true;
  await jobs.list();
  expect(jobs.jobs).toHaveLength(11);
  expect(jobs.unavailable).toEqual([{ id: 'id-4', message: '制作記録を読み込めません' }]);
  expect(jobs.selected?.job_id).toBe('id-4'); // Keep the last observation, not a fabricated new state.
  expect(jobs.error).toBe(''); expect(jobs.loaded).toBe(true); expect(jobs.more).toBe(true);
  await jobs.list(true);
  expect(session.mock.calls.filter(([r]) => r.request.type === 'list').at(-1)![0].request.after).toBe('id-11');
  expect(jobs.jobs).toHaveLength(12); expect(jobs.unavailable).toHaveLength(1);
  broken = false; await jobs.list();
  expect(jobs.unavailable).toEqual([]); expect(jobs.jobs).toHaveLength(12);
});

test('a wholly unreadable page stays distinguishable from an empty history', async () => {
  const session = vi.fn(async r => {
    if (r.request.type === 'list') return { type: 'build_video_jobs', job_ids: ['bad'] } as SessionResponse;
    throw new Error('接続が切れました');
  });
  const jobs = new BuildJobsController({ session }); await jobs.list();
  expect(jobs.loaded).toBe(true); expect(jobs.jobs).toEqual([]);
  expect(jobs.unavailable).toEqual([{ id: 'bad', message: '接続が切れました' }]);
  session.mockResolvedValueOnce({ type: 'build_video_jobs', job_ids: [] });
  await jobs.list(); expect(jobs.unavailable).toEqual([]);
});

test('retrying one unavailable record recovers it without resetting paged history', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'build_video_jobs', job_ids: ['one', 'two'] })
    .mockResolvedValueOnce(result('one')).mockRejectedValueOnce(new Error('一時的な失敗'));
  const jobs = new BuildJobsController({ session }); await jobs.list();
  session.mockResolvedValueOnce(result('two', { phase: 'succeeded' }));
  await jobs.refresh('two');
  expect(jobs.unavailable).toEqual([]);
  expect(jobs.jobs.map(j => j.job_id).sort()).toEqual(['one', 'two']);
  expect(jobs.selected).toMatchObject({ job_id: 'two', phase: 'succeeded' });
  expect(session.mock.lastCall![0].request).toEqual({ type: 'get', job_id: 'two', wait_ms: 0 });
});
