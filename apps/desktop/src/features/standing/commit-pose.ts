import type { Clip, VrmStanding } from '@sungeki/api-client';
import type { EditorController } from '../editor/controller.svelte';
// Only the confirmed standing and the pose are submitted; unrelated form drafts never enter this command.
export async function commitPose(editor: EditorController, clip: Clip, pose: NonNullable<VrmStanding['pose']>) {
  if (clip.content.type !== 'vrm' || editor.busy || editor.pending || editor.incoming) return false;
  await editor.apply([{ type: 'set_vrm', clip_id: clip.id, standing: { ...clip.content.standing, pose } }]);
  return !editor.error && !editor.pending;
}
