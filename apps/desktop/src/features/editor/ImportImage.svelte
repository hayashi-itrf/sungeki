<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { chooseImage, choosePsd, imageError } from './image-file';
  import { importVisual } from './import-visual';
  import type { Command } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  let { editor, kind = 'image' }: { editor: EditorController; kind?: 'image' | 'psd' } = $props();
  async function add() {
    const current = editor.snapshot;
    if (!current || editor.busy || editor.pending || editor.drafting) return;
    const start = Math.round(editor.time * 1_000_000);
    editor.busy = true; editor.error = '';
    let commands: Command[] | null = null;
    let clipId = '';
    try {
      const chosen = kind === 'psd' ? await choosePsd() : await chooseImage(locale.text('imageAdd'));
      if (!chosen) return;
      clipId = crypto.randomUUID();
      const source = 'kind' in chosen ? chosen : { ...chosen, kind: 'image' as const };
      commands = importVisual(current.project, source, start,
        { asset: crypto.randomUUID(), clip: clipId, track: crypto.randomUUID() });
    } catch (error) { editor.error = imageError(error); }
    finally {
      editor.busy = false;
      if (!commands && editor.incoming && !editor.drafting && !editor.pending) {
        const error = editor.error; editor.acceptIncoming(); editor.error = error;
      }
    }
    if (commands) {
      await editor.request({ type: 'edit', session_id: current.session_id, edit: {
        operation_id: crypto.randomUUID(), expected_revision: current.revision, action: { type: 'apply', commands } } });
      if (!editor.error) editor.selected = clipId;
    }
  }
</script>
<button disabled={!editor.snapshot || editor.busy || !!editor.pending || editor.drafting} onclick={add}>{kind === 'psd' ? locale.text('psdAddButton') : locale.text('imageAddButton')}</button>
