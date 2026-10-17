import type { Clip, Command, SessionSnapshot } from '@sungeki/api-client';
import type { EditorController } from './controller.svelte';
import type { Timing } from './timeline-gesture';

export async function commitTiming(editor: EditorController, captured: SessionSnapshot, clip: Clip, next: Timing | null, ids: string[] = [clip.id], releasePreview: () => void = () => {}) {
  try {
    if (next && (next.start_us !== clip.start_us || next.duration_us !== clip.duration_us)) {
      const commands: Command[] = ids.length > 1
        ? [{ type: 'move_selection', scene_ids: [], group_ids: [], clip_ids: [...ids], delta_us: next.start_us - clip.start_us }]
        : [{ type: 'move_clip', clip_id: clip.id, start_us: next.start_us },
           { type: 'resize_clip', clip_id: clip.id, duration_us: next.duration_us }];
      await editor.request({ type: 'edit', session_id: captured.session_id, edit: {
        operation_id: crypto.randomUUID(), expected_revision: captured.revision, action: { type: 'apply', commands } } });
    }
  } finally {
    releasePreview();
  }
  // Pointer drafts are gone even if the outcome is uncertain. The pending request
  // still blocks edits; retrying it must not leave a phantom form draft behind.
  editor.drafting = false;
  if (!editor.pending) {
    const error = editor.error;
    editor.acceptIncoming();
    editor.error = error;
  }
}
