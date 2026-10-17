import type { EditorController } from './controller.svelte';

type Key = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey' | 'repeat' | 'isComposing' | 'defaultPrevented' | 'preventDefault'>;
export type Shortcut = 'save' | 'saveAs' | 'undo' | 'redo' | 'delete';

export function shortcut(event: Key): Shortcut | null {
  if (event.defaultPrevented || event.isComposing || event.altKey || (event.ctrlKey && event.metaKey)) return null;
  const key = event.key.toLowerCase();
  if (event.ctrlKey || event.metaKey) {
    if (key === 's') return event.shiftKey ? 'saveAs' : 'save';
    if (key === 'z') return event.shiftKey ? 'redo' : 'undo';
    if (key === 'y' && event.ctrlKey && !event.shiftKey) return 'redo';
  } else if (!event.shiftKey && (key === 'delete' || key === 'backspace')) return 'delete';
  return null;
}

// Text editing and modal dialogs own their keyboard events. Other recognized keys
// are consumed even while locked, so a browser/native fallback cannot edit instead.
export function handleShortcut(event: Key, editor: EditorController, save: (asNew: boolean) => void,
  context: { editingText: boolean; modalOpen: boolean; choosing: boolean }) {
  if (context.editingText || context.modalOpen) return;
  const action = shortcut(event);
  if (!action) return;
  event.preventDefault();
  if (event.repeat || editor.busy || editor.pending || editor.drafting || context.choosing || !editor.snapshot) return;
  if (action === 'save' || action === 'saveAs') save(action === 'saveAs');
  else if (action === 'undo' || action === 'redo') void editor.history(action);
  else {
    const ids = new Set(editor.selection);
    const commands = editor.snapshot.project.clips.filter(clip => ids.has(clip.id))
      .map(clip => ({ type: 'delete_clip' as const, clip_id: clip.id }));
    if (commands.length) void editor.apply(commands);
  }
}
