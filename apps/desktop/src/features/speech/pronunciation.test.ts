import { expect, test, vi } from 'vitest';
import type { SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { EditorController } from '../editor/controller.svelte';
import { PronunciationController } from './pronunciation.svelte';

function editor() {
  const value = new EditorController({ session: vi.fn() });
  value.snapshot = { session_id: 's', revision: 2 } as SessionSnapshot;
  return value;
}
function result(): SessionResponse {
  return { type: 'pronunciation_preview', session_id: 's', revision: 2, final_text: 'よみ', explicit_reading: false, applied: [] };
}
test('preview uses confirmed host revision, preserves text, and never changes the editor snapshot', async () => {
  const session = vi.fn().mockResolvedValue(result());
  const preview = new PronunciationController({ session }); const e = editor();
  const before = e.snapshot;
  preview.subtitle = ' 字幕\n'; await preview.preview(e);
  expect(session).toHaveBeenCalledWith({ type: 'preview_pronunciation', session_id: 's', expected_revision: 2, subtitle_text: ' 字幕\n', spoken_text: null });
  expect(preview.result?.final_text).toBe('よみ'); expect(preview.stale(e)).toBe(false);
  expect(e.snapshot).toBe(before);
  preview.explicit = true; preview.spoken = '';
  expect(preview.stale(e)).toBe(true);
  await preview.preview(e);
  expect(session.mock.lastCall?.[0].spoken_text).toBe('');
});
test('edits, pending incoming changes and changed inputs mark a delayed result stale', async () => {
  let resolve!: (result: SessionResponse) => void;
  const preview = new PronunciationController({ session: () => new Promise(r => { resolve = r; }) });
  const e = editor(); preview.subtitle = '前'; const pending = preview.preview(e);
  preview.subtitle = '後'; resolve(result()); await pending;
  expect(preview.stale(e)).toBe(true);
  preview.subtitle = '前'; expect(preview.stale(e)).toBe(false);
  e.drafting = true; expect(preview.stale(e)).toBe(true);
  e.drafting = false; e.incoming = { session_id: 's', revision: 3 } as SessionSnapshot;
  expect(preview.stale(e)).toBe(true);
  e.incoming = null; e.snapshot = { session_id: 's', revision: 3 } as SessionSnapshot;
  expect(preview.stale(e)).toBe(true);
});
test('closing invalidates in-flight reads; mismatched responses are rejected', async () => {
  let resolve!: (result: SessionResponse) => void;
  const preview = new PronunciationController({ session: () => new Promise(r => { resolve = r; }) });
  const pending = preview.preview(editor()); preview.reset(); resolve(result()); await pending;
  expect(preview.result).toBeNull(); expect(preview.busy).toBe(false);
  const bad = new PronunciationController({ session: vi.fn().mockResolvedValue({ ...result(), session_id: 'other' }) });
  await bad.preview(editor()); expect(bad.result).toBeNull(); expect(bad.error).toContain('応答が一致しません');
});
test('drafts and unresolved edits cannot be previewed as the current dictionary; read errors remain retryable', async () => {
  const session = vi.fn().mockRejectedValue({ message: '作品が更新されています' });
  const preview = new PronunciationController({ session }); const e = editor();
  e.drafting = true; await preview.preview(e);
  e.drafting = false; e.busy = true; await preview.preview(e);
  e.busy = false; e.pending = { type: 'get', session_id: 's' }; await preview.preview(e);
  expect(session).not.toHaveBeenCalled();
  e.pending = null; await preview.preview(e);
  expect(preview.error).toContain('更新'); expect(preview.busy).toBe(false); expect(e.pending).toBeNull();
  session.mockResolvedValue(result()); await preview.preview(e); expect(preview.result).not.toBeNull();
});
