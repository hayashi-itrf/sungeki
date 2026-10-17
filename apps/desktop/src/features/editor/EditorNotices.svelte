<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from './controller.svelte';
  let { editor }: { editor: EditorController } = $props();
</script>
  {#if editor.error}<p role="alert">{editor.error}</p>{/if}
  {#if editor.pending}<button disabled={editor.busy} onclick={() => editor.request(editor.pending!)}>{locale.text('recheckOperation')}</button>{/if}
  {#if editor.incoming && editor.drafting}
    <p role="status">{locale.text('incomingEdits')}</p>
    <button disabled={editor.busy} onclick={() => editor.acceptIncoming()}>{locale.text('discardIncoming')}</button>
  {/if}
<style>
  p { margin:0; padding:8px 14px; }
  [role=alert] { border-bottom:1px solid var(--border); background:var(--surface); }
</style>
