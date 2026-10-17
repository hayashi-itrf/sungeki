import { expect, test, vi } from 'vitest';
import type { ProjectReadResult, SessionRequest } from '@sungeki/api-client';
import { SessionBrowserModel } from './session-browser.svelte';
const summary = (id: string): ProjectReadResult => ({ session_id: id, project_id: `p-${id}`, revision: 3,
  event_cursor: { host_instance_id: 'host', event_seq: 5 }, data: { view: 'summary', title: '同じ作品名',
    canvas: { width: 1920, height: 1080, fps: { numerator: 30, denominator: 1 }, background: '#ffffff' },
    duration_us: 5000000, counts: { clips: 4, tracks: 1, scenes: 2, groups: 2, assets: 0, characters: 0 },
    dirty: true, document_path: `/projects/${id}.vvm.json` } });
const deferred = <T>() => { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r; }); return { promise, resolve }; };

test('loads bounded pages with summary metadata, keeping duplicate titles distinct', async () => {
  let active = 0; let maximum = 0;
  const ids = Array.from({ length: 25 }, (_, i) => String(i));
  const session = vi.fn(async (request: SessionRequest) => {
    if (request.type === 'list') return { type: 'sessions' as const, session_ids: ids };
    if (request.type !== 'read_project') throw new Error('unexpected full-document request');
    expect(request.query.view).toBe('summary');
    active++; maximum = Math.max(maximum, active); await Promise.resolve(); active--;
    return { type: 'project_read' as const, result: summary(request.session_id) };
  });
  const model = new SessionBrowserModel({ session });
  await model.refresh(); expect(model.entries).toHaveLength(12); expect(model.loaded).toBe(true);
  await Promise.all([model.more(), model.more()]); expect(model.entries).toHaveLength(24);
  await model.more(); expect(model.entries.map(e => e.id)).toEqual(ids);
  expect(maximum).toBeLessThanOrEqual(4);
  expect(model.entries[0].summary?.document_path).not.toBe(model.entries[1].summary?.document_path);
  expect(model.entries[0].summary?.dirty).toBe(true); expect(model.entries[0].revision).toBe(3);
});

test('one failed summary remains explicit without hiding other recoverable projects', async () => {
  const session = vi.fn(async (request: SessionRequest) => {
    if (request.type === 'list') return { type: 'sessions' as const, session_ids: ['bad', 'good'] };
    if (request.type !== 'read_project') throw new Error('unexpected request');
    if (request.session_id === 'bad') throw { message: '復旧情報を読めません' };
    return { type: 'project_read' as const, result: summary('good') };
  });
  const model = new SessionBrowserModel({ session }); await model.refresh();
  expect(model.entries[0].summary).toBeNull(); expect(model.entries[0].error).toContain('読めません');
  expect(model.entries[1].summary?.title).toBe('同じ作品名');
});

test('refresh ignores slow results from an older list', async () => {
  const old = deferred<{ type: 'sessions'; session_ids: string[] }>();
  const session = vi.fn().mockReturnValueOnce(old.promise).mockResolvedValueOnce({ type: 'sessions', session_ids: [] });
  const model = new SessionBrowserModel({ session }); const first = model.refresh(); await model.refresh();
  old.resolve({ type: 'sessions', session_ids: ['old'] }); await first;
  expect(model.ids).toEqual([]); expect(model.entries).toEqual([]); expect(session).toHaveBeenCalledTimes(2);
});

test('closing the browser discards in-flight summary responses', async () => {
  const pending = deferred<{ type: 'project_read'; result: ProjectReadResult }>();
  const session = vi.fn().mockResolvedValueOnce({ type: 'sessions', session_ids: ['a'] }).mockReturnValueOnce(pending.promise);
  const model = new SessionBrowserModel({ session }); const loading = model.refresh(); await Promise.resolve();
  model.dispose(); pending.resolve({ type: 'project_read', result: summary('a') }); await loading;
  expect(model.entries).toEqual([]);
});
