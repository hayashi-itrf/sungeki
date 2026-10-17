import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { EditPreview, Project, SessionSnapshot } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import { locale } from '../../shared/i18n/locale.svelte';
import { describeCommand } from './command-description';
import { EditPlan } from './edit-plan.svelte';
import EditReview from './EditReview.svelte';

afterEach(() => { locale.current = 'ja'; });
const project = structuredClone(fixture.project) as Project;
project.scenes = [{ id: 'scene', name: '残す作品名', start_us: 2000000, duration_us: 5000000 }];

test('English structure descriptions distinguish deleting content from removing membership', () => {
  locale.current = 'en';
  expect(describeCommand({ type: 'delete_scene', scene_id: 'scene' }, project)).toBe('Delete scene “残す作品名” (2 s), its clips and its groups.');
  expect(describeCommand({ type: 'ungroup_scene', scene_id: 'scene' }, project)).toContain('keep its clips at their current times');
  expect(describeCommand({ type: 'move_selection', scene_ids: ['scene'], group_ids: [], clip_ids: [], delta_us: -1500000 }, project)).toContain('1.5 s earlier. Each clip moves once.');
  locale.current = 'ja';
  expect(describeCommand({ type: 'delete_scene', scene_id: 'scene' }, project)).toContain('所属クリップ・グループを削除');
});

test('stale review explains why it cannot apply, without editing the project', async () => {
  locale.current = 'en';
  const preview: EditPreview = { source_revision: 4, before_hash: 'before', after_hash: 'after', before_duration_us: 7000000,
    after_duration_us: 0, no_op: false, total_changes: 3, entries: [], next_offset: null };
  const session = vi.fn().mockResolvedValue({ type: 'edit_preview', preview });
  const plan = new EditPlan({ session });
  await plan.prepare({ session_id: 'session', revision: 4 } as SessionSnapshot, [{ type: 'delete_scene', scene_id: 'scene' }]);
  const confirm = vi.fn(), before = JSON.stringify(project);
  const body = render(EditReview, { props: { plan, project, stale: true, disabled: false, confirm } }).body;
  expect(body).toContain('out of date and cannot be applied');
  expect(body).toMatch(/<button[^>]*disabled[^>]*>Apply reviewed changes<\/button>/);
  expect(body).toContain('3 changes / Duration 7 s → 0 s');
  expect(confirm).not.toHaveBeenCalled();
  expect(session).toHaveBeenCalledTimes(1);
  expect(session.mock.calls[0]?.[0].type).toBe('preview_edit');
  expect(JSON.stringify(project)).toBe(before);
});

test.each(['ja', 'en'] as const)('mismatched first review offers retry without an apply control (%s)', async language => {
  locale.current = language;
  const preview: EditPreview = { source_revision: 3, before_hash: 'before', after_hash: 'after',
    before_duration_us: 0, after_duration_us: 0, no_op: false, total_changes: 1,
    entries: [], next_offset: null };
  const session = vi.fn().mockResolvedValue({ type: 'edit_preview', preview });
  const plan = new EditPlan({ session });
  await plan.prepare({ session_id: 'session', revision: 4 } as SessionSnapshot,
    [{ type: 'rename_project', title: '新しい作品名' }]);
  const confirm = vi.fn();
  const body = render(EditReview, { props: { plan, project, stale: false, disabled: false, confirm } }).body;
  expect(body).toContain(locale.text('reviewChangedError'));
  expect(body).toContain(locale.text('reviewRetry'));
  expect(body).not.toContain(locale.text('reviewApply'));
  expect(confirm).not.toHaveBeenCalled();
});
