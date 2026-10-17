import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { Command, Project, SessionSnapshot } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { describeCommand } from './command-description';
import { EditPlan } from './edit-plan.svelte';
import RippleForm from './RippleForm.svelte';

afterEach(() => { locale.current = 'ja'; });
const project = { clips: [] } as unknown as Project;
const command: Command = { type: 'ripple', clip_ids: ['a', 'b'], boundary_us: 2500000, delta_us: -125000 };

test('review states the boundary, selected count and signed direction in both languages', () => {
  expect(describeCommand(command, project)).toBe('選択した2クリップのうち、2.5秒以降に始まるものを0.125秒前へ移動');
  locale.current = 'en';
  expect(describeCommand(command, project)).toBe('Of 2 selected clips, move those starting at or after 2.5 s by 0.125 s Earlier.');
  expect(describeCommand({ ...command, delta_us: 125000 }, project)).toContain('Later');
});

test('form explains boundary rejection and starts without implicit all-project selection', () => {
  const html = render(RippleForm, { props: { project, selected: [], submit: vi.fn() } }).body;
  expect(html).toContain('選択中：0クリップ');
  expect(html).toContain('境界をまたぐ場合は適用できません');
  expect(html).toContain('タイムラインの選択を使う');
  locale.current = 'en';
  const english = render(RippleForm, { props: { project, selected: [], submit: vi.fn() } }).body;
  expect(english).toContain('Boundary (seconds)');
  expect(english).toContain('A selected clip crossing the boundary prevents the edit.');
});

test('rejected boundary preview never changes the project or authorizes apply', async () => {
  const session = vi.fn().mockRejectedValue({ code: 'VALIDATION', message: '境界をまたぐクリップがあります' });
  const plan = new EditPlan({ session });
  const snapshot = { session_id: 's', revision: 2, project } as SessionSnapshot;
  await plan.prepare(snapshot, [command]);
  expect(session).toHaveBeenCalledTimes(1);
  expect(session.mock.calls[0][0].type).toBe('preview_edit');
  expect(plan.preview).toBeNull();
  expect(plan.error).toContain('境界');
  expect(plan.stale({ ...snapshot, session_id: 'another' })).toBe(true);
  plan.cancel();
  expect(plan.request).toBeNull();
});
