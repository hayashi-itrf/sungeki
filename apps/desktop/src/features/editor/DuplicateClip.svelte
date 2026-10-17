<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Clip } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  let { editor, clip }: { editor: EditorController; clip: Clip } = $props();
  async function duplicate() {
    if (editor.busy || editor.drafting || editor.pending) return;
    const id = crypto.randomUUID();
    await editor.apply([{ type: 'duplicate_clip', clip_id: clip.id, new_clip_id: id, start_us: clip.start_us }]);
    if (!editor.error && editor.snapshot?.project.clips.some(c => c.id === id)) editor.selected = id;
  }
</script>
<button type="button" disabled={editor.busy || editor.drafting || !!editor.pending} onclick={duplicate}
  title={locale.text('inspectorDuplicateHint')}>{locale.text('inspectorDuplicate')}</button>
<style>button { margin:4px 0; }</style>
