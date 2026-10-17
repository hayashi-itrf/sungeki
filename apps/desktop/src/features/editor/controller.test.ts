import { afterEach, expect, test, vi } from 'vitest';
import { locale } from '../../shared/i18n/locale.svelte';
afterEach(() => { locale.current = 'ja'; });
import type { SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { EditorController } from './controller.svelte';

function snapshot(): SessionSnapshot {
  return { event_cursor: { host_instance_id: 'host', event_seq: 0 }, session_id: 'session', revision: 0, dirty: true, document_path: null,
    undo_operation_id: null, redo_operation_id: null,
    project: { format: 'vvm-project', schema_version: 1, id: 'project', title: 'test',
      canvas: { width: 1920, height: 1080, fps: { numerator: 30, denominator: 1 }, background: '#ffffff' },
      tracks: [], clips: [], scenes: [], groups: [], assets: [], characters: [], pronunciation_dictionary: [], audio_mix: { normalization: null } },
  };
}
test('failed edits keep the confirmed snapshot and use observed revision', async () => {
  const session = vi.fn().mockRejectedValue({ code: 'CONFLICT', message: 'conflict' });
  const editor = new EditorController({ session });
  editor.snapshot = snapshot();
  await editor.apply([{ type: 'rename_project', title: 'changed' }]);
  expect(editor.snapshot.project.title).toBe('test');
  expect(session.mock.calls[0]?.[0].edit.expected_revision).toBe(0);
  expect(editor.error).toContain('最新');
});
test('uncertain edit blocks new commands and retries the identical operation', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'timeout' });
  const editor = new EditorController({ session });
  editor.snapshot = snapshot();
  await editor.apply([{ type: 'rename_project', title: 'changed' }]);
  const request = session.mock.calls[0]?.[0];
  await editor.add('shape');
  expect(session).toHaveBeenCalledTimes(1);
  const confirmed = snapshot(); confirmed.revision = 1; confirmed.project.title = 'changed';
  session.mockResolvedValue({ type: 'snapshot', snapshot: confirmed } satisfies SessionResponse);
  await editor.request(editor.pending!);
  expect(session.mock.calls[1]?.[0]).toEqual(request);
  expect(editor.pending).toBeNull();
  expect(editor.snapshot.revision).toBe(1);
});

test('uncertain preset application retains the exact IDs and operation for retry', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'timeout' });
  const editor = new EditorController({ session }); editor.snapshot = snapshot();
  const request = { type: 'instantiate_preset' as const, args: {
    session_id: 'session', expected_revision: 0, operation_id: 'operation', preset_id: 'preset',
    preset_revision: 1, ids: { character_id: 'character', asset_ids: {} }, placement: null,
  } };
  await editor.request(request);
  expect(editor.pending).toBe(request);
  await editor.add('shape');
  expect(session).toHaveBeenCalledTimes(1);
  session.mockResolvedValue({ type: 'snapshot', snapshot: snapshot() });
  await editor.request(editor.pending!);
  expect(session.mock.calls[1]?.[0]).toBe(request);
  expect(editor.pending).toBeNull();
});
test('first clip and track use one atomic batch', async () => {
  const session = vi.fn().mockRejectedValue({ code: 'VALIDATION_FAILED', message: 'rejected' });
  const editor = new EditorController({ session }); editor.snapshot = snapshot();
  await editor.add('text');
  const commands = session.mock.calls[0]?.[0].edit.action.commands;
  expect(commands.map((c: { type: string }) => c.type)).toEqual(['add_track', 'add_clip']);
  expect(commands[1].clip.track_id).toBe(commands[0].track.id);
  expect(editor.snapshot.project.clips).toEqual([]);
});

test('new text and shapes create separate front layers without reusing occupied tracks', async () => {
  for (const kind of ['text', 'shape'] as const) {
    for (const visible of [true, false]) {
      const session = vi.fn().mockRejectedValue({ code: 'VALIDATION_FAILED', message: 'test response' });
      const editor = new EditorController({ session }); const initial = snapshot();
      initial.project.tracks = [
        { id: 'background', name: 'background', kind: 'visual', visible, muted: false },
        { id: 'foreground', name: 'foreground', kind: 'visual', visible, muted: false },
        { id: 'hidden', name: 'hidden', kind: 'visual', visible: false, muted: false },
        { id: 'sound', name: 'sound', kind: 'audio', visible: true, muted: false },
      ];
      const before = structuredClone(initial); editor.snapshot = initial;
      await editor.add(kind);
      const commands = session.mock.calls[0][0].edit.action.commands;
      expect(commands.map((c: { type: string }) => c.type)).toEqual(['add_track', 'add_clip']);
      expect(commands[0].track.visible).toBe(true);
      expect(initial.project.tracks.some(t => t.id === commands[0].track.id)).toBe(false);
      expect(commands[1].clip.track_id).toBe(commands[0].track.id);
      expect(initial).toEqual(before);
    }
  }
});

test('incoming changes preserve a draft and reject stale responses', async () => {
  const editor = new EditorController({ session: vi.fn() });
  editor.snapshot = snapshot(); editor.drafting = true;
  const remote = snapshot(); remote.revision = 1; remote.event_cursor.event_seq = 1;
  remote.project.title = 'remote';
  editor.receive(remote);
  expect(editor.snapshot.project.title).toBe('test');
  expect(editor.incoming?.project.title).toBe('remote');
  editor.acceptIncoming();
  expect(editor.snapshot.project.title).toBe('remote');
  editor.receive(snapshot());
  expect(editor.snapshot.revision).toBe(1);
});

test('save notification preserves unapplied input even without a revision change', async () => {
  const saved = snapshot(); saved.dirty = false; saved.document_path = '/tmp/saved.vvm.json';
  saved.event_cursor.event_seq = 1;
  const session = vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: saved });
  const editor = new EditorController({ session }); editor.snapshot = snapshot(); editor.drafting = true;
  await editor.request({ type: 'save', session_id: 'session', path: saved.document_path, expected_revision: 0 });
  expect(editor.drafting).toBe(true);
  expect(editor.draftGeneration).toBe(0);
  expect(editor.incoming).toBeNull();
  expect(editor.snapshot.dirty).toBe(false);
  expect(editor.drafting).toBe(true);
});

test('uncertain checkpoint restore retries the exact operation and blocks edits', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'timeout' });
  const editor = new EditorController({ session }); editor.snapshot = snapshot();
  const request = { type: 'checkpoint', session_id: 'session', request: { type: 'restore',
    checkpoint_id: 'checkpoint', operation_id: 'restore-operation', expected_revision: 0 } } as const;
  await editor.request(request);
  expect(editor.pending).toEqual(request);
  await editor.add('text');
  expect(session).toHaveBeenCalledTimes(1);
  const confirmed = snapshot(); confirmed.revision = 1;
  session.mockResolvedValue({ type: 'snapshot', snapshot: confirmed } satisfies SessionResponse);
  await editor.request(editor.pending!);
  expect(session.mock.calls[1]?.[0]).toEqual(request);
  expect(editor.pending).toBeNull();
  expect(editor.snapshot.revision).toBe(1);
});

test('multiple selection toggles, preserves order and prunes remotely removed clips', () => {
  const editor = new EditorController({ session: vi.fn() }); const state = snapshot();
  state.project.clips = ['a', 'b', 'c'].map(id => ({ id, track_id: 'track', scene_id: null, start_us: 0, duration_us: 100,
    transform: { x: 0, y: 0, width: 1, height: 1, opacity: 1 }, content: { type: 'shape', shape: 'rectangle', fill: '#000000' } }));
  editor.snapshot = state;
  editor.selectClip('a'); editor.selectClip('b', true); editor.selectClip('c', true);
  expect(editor.selection).toEqual(['a', 'b', 'c']);
  editor.selectClip('b', true); expect(editor.selection).toEqual(['a', 'c']);
  editor.selectClip('missing', true); expect(editor.selection).toEqual(['a', 'c']);
  editor.drafting = true; editor.selectClip('b'); expect(editor.selection).toEqual(['a', 'c']);
  const remote = structuredClone(state); remote.revision = 1; remote.event_cursor.event_seq = 1;
  remote.project.clips = remote.project.clips.filter(c => c.id !== 'c');
  editor.receive(remote); expect(editor.selection).toEqual(['a', 'c']);
  editor.acceptIncoming(); expect(editor.selection).toEqual(['a']); expect(editor.selected).toBe('a');
  editor.selected = 'b'; expect(editor.selection).toEqual(['b']);
  editor.selected = null; expect(editor.selection).toEqual([]);
});

test('review navigation selects the current clip time without changing the document or sending edits', () => {
  const session = vi.fn();
  const editor = new EditorController({ session });
  const initial = snapshot();
  initial.project.clips.push({ id: 'target', track_id: 'track', scene_id: null, start_us: 42_000_000, duration_us: 1_000_000,
    transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content: { type: 'shape', shape: 'rectangle', fill: '#ffffff' } });
  editor.snapshot = initial;
  const before = JSON.stringify(editor.snapshot);
  expect(editor.revealClip('target')).toBe(true);
  expect(editor.selection).toEqual(['target']);
  expect(editor.time).toBe(42);
  expect(editor.navigationGeneration).toBe(1);
  expect(JSON.stringify(editor.snapshot)).toBe(before);
  expect(session).not.toHaveBeenCalled();
  expect(editor.revealClip('deleted')).toBe(false);
  expect(editor.selection).toEqual(['target']);
  expect(editor.navigationGeneration).toBe(1);
  for (const blocked of ['busy', 'drafting'] as const) {
    editor[blocked] = true;
    expect(editor.revealClip('target')).toBe(false);
    editor[blocked] = false;
  }
  editor.pending = { type: 'get', session_id: 'session' };
  expect(editor.revealClip('target')).toBe(false);
  expect(editor.navigationGeneration).toBe(1);
});

test('blur starts on a new visible front track and preserves the confirmed project', async () => {
  const session = vi.fn().mockRejectedValue({ code: 'CONFLICT', message: 'conflict' });
  const editor = new EditorController({ session });
  const initial = snapshot();
  initial.project.tracks.push({ id: 'hidden', name: 'hidden', kind: 'visual', visible: false, muted: false });
  editor.snapshot = initial;
  editor.time = 2.5;
  const before = JSON.stringify(initial);
  await editor.add('blur');
  const request = session.mock.calls[0]?.[0];
  const [track, clip] = request.edit.action.commands;
  expect(track.type).toBe('add_track');
  expect(track.track.visible).toBe(true);
  expect(clip.clip.track_id).toBe(track.track.id);
  expect(clip.clip.track_id).not.toBe('hidden');
  expect(clip.clip.start_us).toBe(2_500_000);
  expect(clip.clip.transform).toEqual({ x: 0, y: 0, width: 1920, height: 1080, opacity: 1 });
  expect(clip.clip.content).toEqual({ type: 'blur', sigma: 18 });
  expect(JSON.stringify(editor.snapshot)).toBe(before);
});

test('add does not change selection while an edit is blocked', async () => {
  const session = vi.fn();
  const editor = new EditorController({ session });
  editor.snapshot = snapshot(); editor.selected = 'existing';
  for (const flag of ['busy', 'drafting'] as const) {
    editor[flag] = true; await editor.add('blur'); editor[flag] = false;
  }
  editor.pending = { type: 'get', session_id: 'session' };
  await editor.add('blur');
  expect(session).not.toHaveBeenCalled();
  expect(editor.selected).toBe('existing');
});
test('diagnostic navigation seeks the reported frame and rejects invalid times without edits', () => {
  const session = vi.fn();
  const editor = new EditorController({ session });
  const initial = snapshot();
  initial.project.canvas.fps = { numerator: 30, denominator: 1 };
  initial.project.clips.push({ id: 'diagnostic', track_id: 'track', scene_id: null, start_us: 0, duration_us: 1000000,
    transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content: { type: 'text', text: 'test', font_size: 20, color: '#000000' } });
  editor.snapshot = initial;
  expect(editor.revealClip('diagnostic', 33333)).toBe(true);
  expect(editor.time).toBe(1 / 30);
  expect(editor.selected).toBe('diagnostic');
  for (const time of [-1, NaN, Infinity, 1000000, 1.5]) expect(editor.revealClip('diagnostic', time)).toBe(false);
  expect(editor.time).toBe(1 / 30);
  expect(editor.navigationGeneration).toBe(1);
  expect(editor.snapshot).toBe(initial);
  expect(session).not.toHaveBeenCalled();
});

test('duplicate notification after an edit keeps the confirmed document reference', async () => {
  const initial = snapshot();
  const edited = snapshot(); edited.revision = 1; edited.event_cursor.event_seq = 1;
  edited.project.title = 'edited';
  const session = vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: edited });
  const editor = new EditorController({ session }); editor.snapshot = initial;
  editor.busy = true; editor.receive(structuredClone(edited)); editor.busy = false;
  await editor.apply([{ type: 'rename_project', title: 'edited' }]);
  expect(editor.snapshot).toBe(edited);
  expect(editor.incoming).toBeNull();
  editor.receive(structuredClone(edited));
  expect(editor.snapshot).toBe(edited);
});

test('same-revision save metadata refreshes without rebuilding Project or losing drafts', () => {
  const editor = new EditorController({ session: vi.fn() });
  const initial = snapshot(); editor.snapshot = initial; editor.drafting = true;
  const saved = structuredClone(initial);
  saved.dirty = false; saved.document_path = '/tmp/renamed.vvm.json'; saved.event_cursor.event_seq++;
  editor.receive(saved);
  expect(editor.snapshot?.project).toBe(initial.project);
  expect(editor.snapshot?.document_path).toBe(saved.document_path);
  expect(editor.snapshot?.dirty).toBe(false);
  expect(editor.snapshot?.event_cursor).toEqual(saved.event_cursor);
  expect(editor.drafting).toBe(true);
});

test('revision changes and host restarts never reuse cached Project data', () => {
  const editor = new EditorController({ session: vi.fn() }); editor.snapshot = snapshot();
  const remote = snapshot(); remote.revision = 1; remote.event_cursor.event_seq = 1;
  remote.project.title = 'remote';
  editor.receive(remote); expect(editor.snapshot?.project).toBe(remote.project);
  const restarted = structuredClone(remote); restarted.event_cursor = { host_instance_id: 'new', event_seq: 0 };
  editor.receive(restarted); expect(editor.snapshot).toBe(restarted);
  const stale = snapshot(); editor.receive(stale); expect(editor.snapshot).toBe(restarted);
});


test('connection labels follow language changes without requests or Project changes', () => {
  const session = vi.fn(); const editor = new EditorController({ session });
  editor.snapshot = snapshot(); const before = JSON.stringify(editor.snapshot);
  for (const [state, english, japanese] of [
    ['connecting', 'Connecting', '接続中'], ['live', 'Receiving', '自動受信'],
    ['resynced', 'Reconnected', '再接続しました'], ['retrying', 'Reconnecting', '再接続中'],
  ] as const) {
    editor.syncState = state;
    locale.current = 'en'; expect(editor.syncStatus).toContain(english);
    locale.current = 'ja'; expect(editor.syncStatus).toContain(japanese);
  }
  expect(session).not.toHaveBeenCalled();
  expect(JSON.stringify(editor.snapshot)).toBe(before);
});

test('English uncertain and conflict guidance retains the exact pending operation and original contents', async () => {
  locale.current = 'en';
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '切断' });
  const editor = new EditorController({ session }); editor.snapshot = snapshot();
  const before = JSON.stringify(editor.snapshot);
  await editor.add('text');
  const original = editor.pending;
  expect(original?.type).toBe('edit');
  expect(session.mock.calls[0]?.[0].edit.action.commands[0].track.name).toBe('Visual');
  expect(session.mock.calls[0]?.[0].edit.action.commands[1].clip.content.text).toBe('New text');
  await editor.apply([{ type: 'rename_project', title: 'must not send' }]);
  expect(editor.error).toContain('Check that same operation');
  expect(editor.pending).toBe(original); expect(session).toHaveBeenCalledTimes(1);
  session.mockRejectedValueOnce({ code: 'CONFLICT', message: '別の編集' });
  await editor.request(original!);
  expect(session.mock.calls[1]?.[0]).toBe(original);
  expect(editor.pending).toBeNull();
  expect(editor.error).toContain('別の編集');
  expect(editor.error).toContain('Get the latest state');
  expect(JSON.stringify(editor.snapshot)).toBe(before);
});


test('new text is readable on dark and light solid canvases without rewriting existing clips', async () => {
  for (const [background, color] of [['#000000', '#ffffff'], ['#18253B', '#ffffff'], ['#ffffff', '#000000'], ['#fff2cc', '#000000']]) {
    const session = vi.fn().mockRejectedValue({ code: 'VALIDATION_FAILED', message: 'test response' });
    const editor = new EditorController({ session }); const initial = snapshot();
    initial.project.canvas.background = background!;
    initial.project.clips = [{ id: 'existing', track_id: 'track', scene_id: null,
      start_us: 0, duration_us: 1_000_000,
      transform: { x: 0, y: 0, width: 200, height: 100, opacity: 1 },
      content: { type: 'text', text: '既存の色指定', font_size: 32, color: '#aa5500' } }];
    const before = structuredClone(initial); editor.snapshot = initial;
    await editor.add('text');
    const commands = session.mock.calls[0][0].edit.action.commands;
    expect(commands.find((c: { type: string }) => c.type === 'add_clip').clip.content.color).toBe(color);
    expect(initial).toEqual(before);
  }
});
