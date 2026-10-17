import { expect, test, vi } from 'vitest';
import type { Command, EditPreview, SessionSnapshot } from '@sungeki/api-client';
import { EditPlan } from './edit-plan.svelte';
const snapshot = { session_id: 'session', revision: 4 } as SessionSnapshot;
const preview: EditPreview = { source_revision: 4, before_hash: 'before', after_hash: 'after',
  before_duration_us: 0, after_duration_us: 0, no_op: false, total_changes: 1,
  entries: [], next_offset: null };

test('review freezes command IDs and revision despite subsequent form edits', async () => {
  const session = vi.fn().mockResolvedValue({ type: 'edit_preview', preview });
  const plan = new EditPlan({ session });
  const command: Command = { type: 'duplicate_group', group_id: 'old', new_group_id: 'new', start_us: 10, clip_ids: { old_clip: 'new_clip' } };
  await plan.prepare(snapshot, [command]);
  command.clip_ids.old_clip = 'different';
  expect(session.mock.calls[0]?.[0].commands[0].clip_ids.old_clip).toBe('new_clip');
  expect(plan.request?.edit.expected_revision).toBe(4);
  expect(plan.request?.edit.action.type).toBe('apply');
  if (plan.request?.edit.action.type === 'apply') expect(plan.request.edit.action.commands[0]).toEqual({ ...command, clip_ids: { old_clip: 'new_clip' } });
  expect(plan.stale(snapshot)).toBe(false);
  expect(plan.stale({ ...snapshot, revision: 5 })).toBe(true);
  expect(plan.stale({ ...snapshot, session_id: 'other' })).toBe(true);
});

test('cancelled asynchronous preview cannot reappear over a newer plan', async () => {
  let resolve!: (value: unknown) => void;
  const session = vi.fn().mockReturnValueOnce(new Promise(r => { resolve = r; }))
    .mockResolvedValue({ type: 'edit_preview', preview: { ...preview, after_hash: 'new' } });
  const plan = new EditPlan({ session });
  const old = plan.prepare(snapshot, [{ type: 'rename_project', title: 'old' }]);
  plan.cancel();
  await plan.prepare(snapshot, [{ type: 'rename_project', title: 'new' }]);
  resolve({ type: 'edit_preview', preview }); await old;
  expect(plan.preview?.after_hash).toBe('new');
  expect(plan.busy).toBe(false);
});

test('pagination resends the reviewed commands and retains conflict errors', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'edit_preview', preview: { ...preview, next_offset: 20 } })
    .mockRejectedValueOnce({ code: 'CONFLICT', message: '文書が更新されています' });
  const plan = new EditPlan({ session });
  await plan.prepare(snapshot, [{ type: 'rename_project', title: 'preview' }]);
  const request = plan.request;
  await plan.page(true);
  expect(session.mock.calls[1]?.[0]).toEqual({ ...session.mock.calls[0]?.[0], offset: 20 });
  expect(plan.request).toBe(request);
  expect(plan.error).toContain('更新');
});

test('a preview for a different revision cannot authorize the prepared edit', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'edit_preview', preview: { ...preview, source_revision: 3 } })
    .mockResolvedValueOnce({ type: 'edit_preview', preview });
  const plan = new EditPlan({ session });
  await plan.prepare(snapshot, [{ type: 'rename_project', title: 'review me' }]);
  const request = plan.request;
  expect(plan.preview).toBeNull();
  expect(plan.error).not.toBe('');
  expect(plan.busy).toBe(false);
  // Retry the same frozen operation, without creating a new edit or operation ID.
  await plan.page(false);
  expect(plan.request).toBe(request);
  expect(session.mock.calls[1]?.[0]).toEqual(session.mock.calls[0]?.[0]);
  expect(plan.preview).toEqual(preview);
  expect(plan.error).toBe('');
  expect(session.mock.calls.every(([request]) => request.type === 'preview_edit')).toBe(true);
});

test.each([
  { source_revision: 5 }, { before_hash: 'other-before' }, { after_hash: 'other-after' },
])('pagination rejects a different comparison identity: %j', async mismatch => {
  const first = { ...preview, next_offset: 20 };
  const session = vi.fn().mockResolvedValueOnce({ type: 'edit_preview', preview: first })
    .mockResolvedValueOnce({ type: 'edit_preview', preview: { ...preview, ...mismatch } });
  const plan = new EditPlan({ session });
  await plan.prepare(snapshot, [{ type: 'rename_project', title: 'review me' }]);
  const request = plan.request;
  await plan.page(true);
  expect(plan.preview).toEqual(first);
  expect(plan.request).toBe(request);
  expect(plan.error).not.toBe('');
  expect(plan.busy).toBe(false);
});
