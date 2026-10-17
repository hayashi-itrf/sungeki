<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import CanvasForm from './CanvasForm.svelte';
  import EditorNotices from './EditorNotices.svelte';
  import type { EditorController } from './controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  let open = $state(false);
  let dialog = $state<HTMLDialogElement>();
  $effect(() => { if (open && dialog) dialog.showModal(); });
  function close() {
    if (editor.busy || editor.pending) return;
    editor.acceptIncoming(); open = false;
  }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={() => { open = true; }}>{locale.text('canvasSettings')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('canvasSettings')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('canvasSettings')}</h2><button disabled={editor.busy || !!editor.pending} onclick={close}>{locale.text(editor.drafting ? 'trackDiscardClose' : 'close')}</button></header>
    <EditorNotices {editor} />
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.draftGeneration}`}<CanvasForm {editor} />{/key}
  </dialog>
{/if}
<style>header { display:flex; align-items:center; justify-content:space-between; gap:12px; }</style>
