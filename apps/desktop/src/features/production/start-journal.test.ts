import { expect, test, vi } from 'vitest';
import type { StartBuildVideoArgs, SessionResponse } from '@sungeki/api-client';
import { BrowserStartJournal } from './start-journal';
import { BuildJobsController } from './jobs.svelte';
import { buildConnection } from './voice-settings';
function setup() {
  const data = new Map<string, string>();
  const storage = { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => { data.set(k, v); }, removeItem: (k: string) => { data.delete(k); } };
  const journal = new BrowserStartJournal(() => storage);
  const input = { recipe: { title: '保存する台本', scenes: [] }, project_path: '/qa/new.json', base_url: 'http://127.0.0.1:50021', engine_version: '1' } as unknown as Omit<StartBuildVideoArgs, 'job_id'>;
  return { data, storage, journal, input };
}
test('lost reply survives controller recreation without automatic dispatch and retries identical args', async () => {
  const { journal, input } = setup();
  const session = vi.fn().mockRejectedValueOnce(new Error('response lost'));
  const first = new BuildJobsController({ session }, journal);
  await first.start(input); const original = JSON.stringify(first.pending);
  first.dispose(); input.recipe.title = '編集後';
  const restored = new BuildJobsController({ session }, journal);
  expect(JSON.stringify(restored.pending)).toBe(original); expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementationOnce(async r => { throw { message: 'not created', details: { job_created: 'false', job_id: r.request.args.job_id } }; });
  await restored.retry(); expect(JSON.stringify(restored.pending)).toBe(original);
  session.mockImplementationOnce(async r => ({ type: 'build_video_job', job: { job_id: r.request.args.job_id, phase: 'running' } }) as SessionResponse);
  await restored.retry(); expect(JSON.stringify(session.mock.lastCall![0].request.args)).toBe(original);
  expect(restored.pending).toBeNull(); expect(journal.read()).toBeNull();
});
test('write failure or corrupt saved data prevents a new network start without removing evidence', async () => {
  const { journal, storage, data, input } = setup(); const session = vi.fn();
  storage.setItem = () => { throw new Error('quota'); };
  const jobs = new BuildJobsController({ session }, journal);
  await jobs.start(input); expect(session).not.toHaveBeenCalled(); expect(jobs.recoveryError).toContain('quota');
  data.set('sungeki.pending-build-start.v1', '{broken');
  const restored = new BuildJobsController({ session }, journal);
  await restored.start(input); expect(session).not.toHaveBeenCalled(); expect(restored.recoveryError).toContain('読み取れません');
  expect([...data.values()]).toEqual(['{broken']);
});
test('cleanup failure retains pending intent and a later read-only check can resolve it', async () => {
  const { journal, storage, input } = setup();
  const session = vi.fn(async r => ({ type: 'build_video_job', job: { job_id: r.request.args?.job_id ?? r.request.job_id, phase: 'running' } }) as SessionResponse);
  const remove = storage.removeItem; storage.removeItem = () => { throw new Error('storage unavailable'); };
  const jobs = new BuildJobsController({ session }, journal); await jobs.start(input);
  expect(jobs.pending).not.toBeNull(); expect(journal.read()).not.toBeNull();
  storage.removeItem = remove; await jobs.checkPending();
  expect(session.mock.lastCall![0].request.type).toBe('get'); expect(jobs.pending).toBeNull(); expect(journal.read()).toBeNull();
});
test('a different saved intent is never overwritten or cleared', () => {
  const { journal, input } = setup(); const first = { ...input, job_id: crypto.randomUUID() };
  journal.write(first);
  expect(() => journal.write({ ...first, job_id: crypto.randomUUID() })).toThrow('未確認');
  expect(() => journal.clear({ ...first, job_id: crypto.randomUUID() })).toThrow('別の制作');
  expect(journal.read()).toEqual(first);
});

test('changed payload under the same ID cannot be replayed or erased by a stale controller', async () => {
  const { journal, data, input } = setup();
  const session = vi.fn().mockRejectedValueOnce(new Error('lost response'));
  const jobs = new BuildJobsController({ session }, journal); await jobs.start(input);
  const original = structuredClone(jobs.pending!);
  const changed = { ...original, recipe: { ...original.recipe, title: '別の入力' } };
  data.set('sungeki.pending-build-start.v1', JSON.stringify({ version: 1, args: changed }));
  await jobs.retry(); expect(session).toHaveBeenCalledTimes(1);
  expect(jobs.error).toContain('一致しません'); expect(jobs.pending).toEqual(original);
  session.mockResolvedValueOnce({ type: 'build_video_job', job: { job_id: original.job_id, phase: 'running' } });
  await jobs.checkPending(); expect(jobs.error).toContain('保持しました');
  expect(journal.read()).toEqual(changed); expect(jobs.pending).toEqual(original);
});
test('missing journal prevents blind retry but an observed job can resolve the old intent', async () => {
  const { journal, data, input } = setup();
  const session = vi.fn().mockRejectedValueOnce(new Error('lost response'));
  const jobs = new BuildJobsController({ session }, journal); await jobs.start(input);
  const id = jobs.pending!.job_id; data.clear();
  await jobs.retry(); expect(session).toHaveBeenCalledTimes(1);
  session.mockResolvedValueOnce({ type: 'build_video_job', job: { job_id: id, phase: 'running' } });
  await jobs.checkPending(); expect(jobs.pending).toBeNull(); expect(journal.read()).toBeNull();
});

test.each(['sbv2', 'qwen', 'irodori'])('managed-only %s start survives a lost reply and restores its unchanged request', async engine => {
  const { journal, input } = setup();
  input.recipe.roles = [{ character: { id: 'speaker', voice: { type: engine } } }] as unknown as typeof input.recipe.roles;
  const discovery = vi.fn();
  Object.assign(input, await buildConnection({ session: discovery }, input.recipe, ''), { managed_profiles: { speaker: 'local-profile' } });
  expect(discovery).not.toHaveBeenCalled();
  const session = vi.fn().mockRejectedValueOnce(new Error('reply lost'));
  const first = new BuildJobsController({ session }, journal);
  await first.start(input);
  expect(session).toHaveBeenCalledTimes(1);
  const sent = structuredClone(session.mock.calls[0][0].request.args);
  first.dispose();
  const restored = new BuildJobsController({ session }, journal);
  expect(restored.recoveryError).toBe('');
  expect(restored.pending).toEqual(sent);
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementationOnce(async r => ({ type: 'build_video_job', job: { job_id: r.request.args.job_id, phase: 'running' } }));
  await restored.retry();
  expect(session.mock.lastCall![0].request.args).toEqual(sent);
  expect(restored.pending).toBeNull();
  expect(journal.read()).toBeNull();
});

test.each([
  { project_path: '' }, { base_url: null }, { engine_version: 42 },
])('invalid envelope stays preserved: %j', invalid => {
  const { journal, data, input } = setup();
  const record = JSON.stringify({ version: 1, args: { ...input, job_id: crypto.randomUUID(), ...invalid } });
  data.set('sungeki.pending-build-start.v1', record);
  expect(() => journal.read()).toThrow('不正');
  expect([...data.values()]).toEqual([record]);
});
