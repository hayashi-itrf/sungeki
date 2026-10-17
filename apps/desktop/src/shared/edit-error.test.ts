import { afterEach, expect, test, vi } from 'vitest';
import { locale } from './i18n/locale.svelte';
import { editErrorMessage } from './edit-error';
import { EditorController } from '../features/editor/controller.svelte';
import { EditPlan } from '../features/scenes/edit-plan.svelte';

afterEach(() => { locale.current = 'ja'; });
const overlap = { code: 'VALIDATION_FAILED', message: '元の説明', details: {
  reason: 'track_overlap', track_name: '図形 $& {track}',
  track_id: 't', first_clip_id: 'a', second_clip_id: 'b',
} };
test('overlap is localized from structured data and keeps literal track names', () => {
  expect(editErrorMessage(overlap)).toContain('トラック「図形 $& {track}」');
  locale.current = 'en';
  expect(editErrorMessage(overlap)).toBe('Clips overlap on track “図形 $& {track}”. Adjust the start time or duration, or choose another track.');
});
test('other or incomplete errors retain the original explanation', () => {
  for (const error of [new Error('detail'), { message: 'detail' },
    { ...overlap, message: 'detail', details: null },
    { ...overlap, message: 'detail', details: { reason: 'track_overlap' } },
    { ...overlap, message: 'detail', code: 'IO_ERROR' }]) {
    expect(editErrorMessage(error)).toBe('detail');
  }
  expect(editErrorMessage('failure')).toBe('failure');
});
test('committed edits and previews both display the localized rejection', async () => {
  locale.current = 'en';
  const api = { session: vi.fn().mockRejectedValue(overlap) };
  const editor = new EditorController(api);
  await editor.request({ type: 'edit', session_id: 's', edit: {
    operation_id: 'o', expected_revision: 0, action: { type: 'apply', commands: [] },
  } });
  expect(editor.error).toBe(editErrorMessage(overlap));
  expect(editor.pending).toBeNull();
  const plan = new EditPlan(api);
  plan.request = { type: 'edit', session_id: 's', edit: {
    operation_id: 'o', expected_revision: 0, action: { type: 'apply', commands: [] },
  } };
  await plan.page(false);
  expect(plan.error).toBe(editor.error);
  expect(plan.preview).toBeNull();
});
