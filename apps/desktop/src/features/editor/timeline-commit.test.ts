import { expect, test, vi } from 'vitest';
import type { Clip, SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { EditorController } from './controller.svelte';
import { commitTiming } from './timeline-commit';
const clip: Clip = { id: 'clip', track_id: 'track', scene_id: null, start_us: 1e6, duration_us: 2e6,
  transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content: { type: 'shape', shape: 'rectangle', fill: '#000000' } };
function snapshot(revision = 0): SessionSnapshot {
  return { event_cursor: { host_instance_id: 'host', event_seq: revision }, session_id: 'session', revision,
    dirty: true, document_path: null, undo_operation_id: null, redo_operation_id: null,
    project: { format: 'vvm-project', schema_version: 1, id: 'project', title: `revision ${revision}`,
      canvas: { width: 1920, height: 1080, fps: { numerator: 30, denominator: 1 }, background: '#ffffff' },
      tracks: [], clips: [structuredClone(clip)], scenes: [], groups: [], assets: [], characters: [], pronunciation_dictionary: [], audio_mix: { normalization: null } } };
}
const next = { start_us: 2e6, duration_us: 1e6 };
test('captured revision is sent once; conflict adopts incoming state and preserves the error', async () => {
  const session = vi.fn().mockRejectedValue({ code: 'CONFLICT', message: '別の編集があります' });
  const editor = new EditorController({ session }); const original = snapshot();
  editor.snapshot = original; editor.drafting = true; editor.receive(snapshot(1));
  await commitTiming(editor, original, clip, next);
  expect(session).toHaveBeenCalledTimes(1);
  const request = session.mock.calls[0][0];
  expect(request.edit.expected_revision).toBe(0);
  expect(request.edit.action.commands).toEqual([
    { type: 'move_clip', clip_id: 'clip', start_us: 2e6 }, { type: 'resize_clip', clip_id: 'clip', duration_us: 1e6 }]);
  expect(editor.snapshot?.revision).toBe(1); expect(editor.drafting).toBe(false);
  expect(editor.error).toContain('別の編集'); expect(original.project.clips[0]).toEqual(clip);
});
test('cancel or unchanged timing never sends an edit and releases incoming changes', async () => {
  for (const timing of [null, { start_us: clip.start_us, duration_us: clip.duration_us }]) {
    const session = vi.fn(); const editor = new EditorController({ session }); const original = snapshot();
    editor.snapshot = original; editor.drafting = true; editor.receive(snapshot(1));
    await commitTiming(editor, original, clip, timing);
    expect(session).not.toHaveBeenCalled(); expect(editor.snapshot?.revision).toBe(1); expect(editor.drafting).toBe(false);
  }
});
test('uncertain result keeps exact retry, then rejection releases input without hiding its reason', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '切断' })
    .mockRejectedValueOnce({ code: 'CONFLICT', message: '競合のため適用されませんでした' });
  const editor = new EditorController({ session }); const original = snapshot();
  editor.snapshot = original; editor.drafting = true;
  await commitTiming(editor, original, clip, next);
  const pending = editor.pending;
  expect(pending).not.toBeNull(); expect(editor.drafting).toBe(false);
  editor.receive(snapshot(1));
  expect(editor.snapshot?.revision).toBe(0);
  await editor.request(pending!);
  expect(session.mock.calls[1][0]).toBe(session.mock.calls[0][0]);
  expect(editor.pending).toBeNull(); expect(editor.drafting).toBe(false);
  expect(editor.snapshot?.revision).toBe(1); expect(editor.error).toContain('適用されませんでした');
});

test('multiple drag commits one frozen selection with a shared delta', async () => {
  const session = vi.fn().mockRejectedValue({ code: 'CONFLICT', message: '競合' });
  const editor = new EditorController({ session }); const original = snapshot();
  editor.snapshot = original; editor.drafting = true;
  const ids = ['clip', 'second'];
  await commitTiming(editor, original, clip, { start_us: 2500000, duration_us: clip.duration_us }, ids);
  ids.push('later-selection');
  expect(session).toHaveBeenCalledTimes(1);
  expect(session.mock.calls[0][0].edit.action.commands).toEqual([
    { type: 'move_selection', scene_ids: [], group_ids: [], clip_ids: ['clip', 'second'], delta_us: 1500000 }]);
  expect(original.project.clips[0]).toEqual(clip);
});

test('newer external deletion received during commit wins over its older successful response', async () => {
  const original = snapshot();
  const applied = snapshot(1);
  applied.project.clips[0] = { ...applied.project.clips[0], ...next };
  const deleted = snapshot(2); deleted.project.clips = [];
  let respond!: (value: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(resolve => { respond = resolve; }));
  const editor = new EditorController({ session });
  editor.snapshot = original; editor.selected = clip.id; editor.drafting = true;
  const completion = commitTiming(editor, original, clip, next);
  editor.receive(deleted);
  expect(editor.snapshot?.revision).toBe(0);
  respond({ type: 'edited', snapshot: applied, receipt: { operation_id: 'drag', revision: 1, no_op: false } });
  await completion;
  expect(editor.snapshot?.revision).toBe(2);
  expect(editor.snapshot?.project.clips).toEqual([]);
  expect(editor.selection).toEqual([]); expect(editor.selected).toBeNull();
  expect(editor.incoming).toBeNull(); expect(editor.drafting).toBe(false);
  expect(editor.pending).toBeNull(); expect(session).toHaveBeenCalledTimes(1);
});

test('cancelling a timing draft adopts external deletion without sending a replacement edit', async () => {
  const session = vi.fn(); const editor = new EditorController({ session });
  const original = snapshot(); const deleted = snapshot(1); deleted.project.clips = [];
  editor.snapshot = original; editor.selected = clip.id; editor.drafting = true;
  editor.receive(deleted);
  await commitTiming(editor, original, clip, null);
  expect(session).not.toHaveBeenCalled();
  expect(editor.snapshot?.revision).toBe(1); expect(editor.selected).toBeNull();
  expect(editor.selection).toEqual([]); expect(editor.drafting).toBe(false);
  expect(original.project.clips).toEqual([clip]);
});


test('buffered external edits do not regress when older snapshots arrive while input is locked', () => {
  for (const lock of ['drafting', 'busy', 'pending'] as const) {
    const editor = new EditorController({ session: vi.fn() });
    editor.snapshot = snapshot();
    if (lock === 'pending') editor.pending = { type: 'get', session_id: 'session' };
    else editor[lock] = true;
    const newest = snapshot(2); newest.project.clips = [];
    editor.receive(newest);
    editor.receive(snapshot(1));
    expect(editor.incoming?.revision).toBe(2);
    editor.busy = false; editor.pending = null; editor.acceptIncoming();
    expect(editor.snapshot?.revision).toBe(2);
    expect(editor.snapshot?.project.clips).toEqual([]);
  }
});

test('buffered save metadata retains the latest event at the same revision', () => {
  const editor = new EditorController({ session: vi.fn() });
  editor.snapshot = snapshot(); editor.drafting = true;
  const saved = snapshot(1); saved.event_cursor.event_seq = 3;
  saved.dirty = false; saved.document_path = '/tmp/saved.vvm.json';
  editor.receive(saved);
  editor.receive(snapshot(1));
  editor.acceptIncoming();
  expect(editor.snapshot?.dirty).toBe(false);
  expect(editor.snapshot?.document_path).toBe(saved.document_path);
  expect(editor.snapshot?.event_cursor.event_seq).toBe(3);
});

test('drop retains its visual preview until a delayed host response settles', async () => {
  for (const outcome of ['success', 'conflict', 'disconnected'] as const) {
    let resolve!: (value: SessionResponse) => void;
    let reject!: (reason: unknown) => void;
    const session = vi.fn(() => new Promise<SessionResponse>((yes, no) => { resolve = yes; reject = no; }));
    const editor = new EditorController({ session });
    const original = snapshot(); editor.snapshot = original; editor.drafting = true;
    const release = vi.fn();
    const completion = commitTiming(editor, original, clip, next, [clip.id], release);
    await Promise.resolve();
    expect(release).not.toHaveBeenCalled();
    expect(editor.snapshot?.project.clips[0]).toEqual(clip);
    expect(editor.busy).toBe(true);
    if (outcome === 'success') {
      const applied = snapshot(1); applied.project.clips[0] = { ...clip, ...next };
      resolve({ type: 'edited', snapshot: applied, receipt: { operation_id: 'drag', revision: 1, no_op: false } });
    } else reject({ code: outcome === 'conflict' ? 'CONFLICT' : 'HOST_UNAVAILABLE', message: outcome });
    await completion;
    expect(release).toHaveBeenCalledTimes(1);
    expect(editor.busy).toBe(false); expect(editor.drafting).toBe(false);
    expect(editor.snapshot?.project.clips[0].start_us).toBe(outcome === 'success' ? next.start_us : clip.start_us);
    expect(!!editor.pending).toBe(outcome === 'disconnected');
  }
});
