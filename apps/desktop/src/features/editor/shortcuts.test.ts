import { expect, test, vi } from 'vitest';
import type { SessionSnapshot } from '@sungeki/api-client';
import { EditorController } from './controller.svelte';
import { handleShortcut, shortcut } from './shortcuts';

function key(overrides = {}) {
  return { key: 'z', ctrlKey: false, metaKey: true, altKey: false, shiftKey: false,
    repeat: false, isComposing: false, defaultPrevented: false, preventDefault: vi.fn(), ...overrides };
}
const context = { editingText: false, modalOpen: false, choosing: false };
function setup() {
  const session = vi.fn().mockRejectedValue({ code: 'CONFLICT', message: 'test' });
  const editor = new EditorController({ session });
  // Only the properties consumed by the shortcut and controller are needed here.
  editor.snapshot = { session_id: 'session', revision: 9, undo_operation_id: 'undo', redo_operation_id: 'redo',
    project: { clips: [{ id: 'a' }, { id: 'b' }, { id: 'c' }] } } as SessionSnapshot;
  return { editor, session, save: vi.fn() };
}
test('standard save/history keys support both platforms without stealing unrelated combinations', () => {
  expect(shortcut(key())).toBe('undo');
  expect(shortcut(key({ shiftKey: true }))).toBe('redo');
  expect(shortcut(key({ key: 'Y', ctrlKey: true, metaKey: false }))).toBe('redo');
  expect(shortcut(key({ key: 's', shiftKey: true }))).toBe('saveAs');
  for (const change of [{ altKey: true }, { isComposing: true }, { ctrlKey: true }, { key: 'a' }, { defaultPrevented: true }]) {
    expect(shortcut(key(change))).toBeNull();
  }
});
test('text inputs and dialogs retain native undo and delete, while locked/repeated edits are consumed', () => {
  for (const area of [{ editingText: true }, { modalOpen: true }]) {
    const { editor, session, save } = setup(); const event = key();
    handleShortcut(event, editor, save, { ...context, ...area });
    expect(event.preventDefault).not.toHaveBeenCalled(); expect(session).not.toHaveBeenCalled();
  }
  for (const lock of ['busy', 'drafting', 'pending', 'choosing', 'repeat'] as const) {
    const { editor, session, save } = setup(); const event = key({ repeat: lock === 'repeat' });
    if (lock === 'busy' || lock === 'drafting') editor[lock] = true;
    if (lock === 'pending') editor.pending = { type: 'get', session_id: 'session' };
    handleShortcut(event, editor, save, { ...context, choosing: lock === 'choosing' });
    expect(event.preventDefault).toHaveBeenCalledOnce(); expect(session).not.toHaveBeenCalled();
  }
});
test('history keeps current revision and target; multi-delete sends one batch without altering Project', () => {
  const { editor, session, save } = setup();
  handleShortcut(key(), editor, save, context);
  expect(session.mock.calls[0][0].edit).toMatchObject({ expected_revision: 9, action: { type: 'undo', expected_operation_id: 'undo' } });
  const next = setup(); next.editor.selection = ['b', 'a', 'a', 'deleted'];
  handleShortcut(key({ key: 'Backspace', metaKey: false }), next.editor, next.save, context);
  expect(next.session).toHaveBeenCalledOnce();
  expect(next.session.mock.calls[0][0].edit.action.commands).toEqual([
    { type: 'delete_clip', clip_id: 'a' }, { type: 'delete_clip', clip_id: 'b' },
  ]);
  expect(next.editor.snapshot!.project.clips).toHaveLength(3);
});
test('save uses the existing file dialog path and empty selection does not create an edit', () => {
  const { editor, session, save } = setup();
  handleShortcut(key({ key: 's' }), editor, save, context);
  handleShortcut(key({ key: 's', shiftKey: true }), editor, save, context);
  expect(save.mock.calls).toEqual([[false], [true]]);
  handleShortcut(key({ key: 'Delete', metaKey: false }), editor, save, context);
  expect(session).not.toHaveBeenCalled();
});
