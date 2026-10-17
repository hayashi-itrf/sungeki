import { expect, test, vi } from 'vitest';
import type { SessionSnapshot } from '@sungeki/api-client';
import { EditorController } from './controller.svelte';

function snapshot(revision = 0): SessionSnapshot {
  return { session_id: 'session', revision, dirty: false, document_path: '/tmp/project.vvm.json',
    undo_operation_id: null, redo_operation_id: null,
    event_cursor: { host_instance_id: 'host', event_seq: revision },
    project: { format: 'vvm-project', schema_version: 1, id: 'project', title: `title ${revision}`,
      canvas: { width: 1920, height: 1080, fps: { numerator: 30, denominator: 1 }, background: '#ffffff' },
      tracks: [], clips: [], scenes: [], groups: [], assets: [], characters: [],
      pronunciation_dictionary: [], audio_mix: { normalization: null } } };
}

test.each([0, 1])('refresh keeps local input mounted when the returned revision is %s', async revision => {
  const result = snapshot(revision);
  const session = vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: result });
  const editor = new EditorController({ session });
  editor.snapshot = snapshot(); editor.drafting = true;
  await editor.request({ type: 'get', session_id: 'session' });
  expect(editor.drafting).toBe(true);
  expect(editor.draftGeneration).toBe(0);
  expect(editor.snapshot?.revision).toBe(0);
  expect(editor.incoming?.revision ?? null).toBe(revision ? 1 : null);
  if (revision) {
    editor.acceptIncoming();
    expect(editor.snapshot?.project.title).toBe('title 1');
    expect(editor.drafting).toBe(false);
    expect(editor.draftGeneration).toBe(1);
  }
});

test('refresh failure keeps input and does not create a pending mutation', async () => {
  const editor = new EditorController({ session: vi.fn().mockRejectedValue({ code: 'HOST_UNAVAILABLE', message: 'offline' }) });
  editor.snapshot = snapshot(); editor.drafting = true;
  await editor.request({ type: 'get', session_id: 'session' });
  expect(editor.drafting).toBe(true); expect(editor.draftGeneration).toBe(0);
  expect(editor.error).toBe('offline'); expect(editor.pending).toBeNull();
});

test('refresh without local input still adopts the latest confirmed document', async () => {
  const editor = new EditorController({ session: vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: snapshot(1) }) });
  editor.snapshot = snapshot();
  await editor.request({ type: 'get', session_id: 'session' });
  expect(editor.snapshot?.revision).toBe(1); expect(editor.incoming).toBeNull();
});


test('opening another document resets the shared position; refreshing it preserves the position', async () => {
  const next = snapshot(); next.session_id = 'other'; next.project.id = 'other-project';
  const editor = new EditorController({ session: vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: next }) });
  editor.snapshot = snapshot(); editor.time = 42; editor.previewVolume = 0.2;
  await editor.request({ type: 'get', session_id: 'other' });
  expect(editor.time).toBe(0); expect(editor.previewVolume).toBe(0.2);
  editor.time = 3;
  await editor.request({ type: 'get', session_id: 'other' });
  expect(editor.time).toBe(3); expect(editor.previewVolume).toBe(0.2);
});
