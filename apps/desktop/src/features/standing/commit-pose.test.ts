import { expect, test, vi } from 'vitest';
import type { Project, SessionSnapshot } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/vrm-clip-project.json';
import { EditorController } from '../editor/controller.svelte';
import { commitPose } from './commit-pose';
function setup() {
  const project = structuredClone(fixture) as Project;
  const snapshot: SessionSnapshot = { project, session_id: 'test', revision: 5, dirty: false,
    document_path: null, undo_operation_id: null, redo_operation_id: null,
    event_cursor: { host_instance_id: 'host', event_seq: 0 } };
  const session = vi.fn().mockRejectedValue({ code: 'CONFLICT', message: 'conflict' });
  const editor = new EditorController({ session }); editor.snapshot = snapshot; editor.drafting = true;
  return { editor, session, clip: project.clips[0] };
}
test('one pose confirmation sends one common command, preserves other settings, and keeps failed drafts', async () => {
  const { editor, session, clip } = setup();
  const before = structuredClone(clip);
  expect(await commitPose(editor, clip, { head: [0, 20, 0] })).toBe(false);
  expect(session).toHaveBeenCalledTimes(1);
  if (clip.content.type !== 'vrm') throw Error('fixture');
  expect(session.mock.calls[0][0].edit).toMatchObject({ expected_revision: 5, action: { type: 'apply', commands: [
    { type: 'set_vrm', clip_id: clip.id, standing: { ...clip.content.standing, pose: { head: [0, 20, 0] } } },
  ] } });
  expect(clip).toEqual(before); expect(editor.drafting).toBe(true);
});
test('buffered remote edits and uncertain requests are never overwritten by pose confirmation', async () => {
  const { editor, session, clip } = setup();
  editor.incoming = { ...editor.snapshot!, revision: 6 };
  expect(await commitPose(editor, clip, {})).toBe(false);
  editor.incoming = null; editor.pending = { type: 'get', session_id: 'test' };
  expect(await commitPose(editor, clip, {})).toBe(false);
  expect(session).not.toHaveBeenCalled();
});
