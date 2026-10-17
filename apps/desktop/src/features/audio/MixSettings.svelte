<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import MixForm from './MixForm.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  let open = $state(false), dialog = $state<HTMLDialogElement>();
  $effect(() => { if (open && dialog) dialog.showModal(); });
  function close() {
    if (editor.busy || editor.pending) return;
    editor.acceptIncoming(); open = false;
  }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={() => { open = true; }}>{locale.text('mixTitle')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('mixTitle')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('mixTitle')}</h2><button disabled={editor.busy || !!editor.pending} onclick={close}>{editor.drafting ? locale.text('mixDiscardClose') : locale.text('close')}</button></header>
    <EditorNotices {editor} />
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.draftGeneration}`}<MixForm {editor} />{/key}
  </dialog>
{/if}
<style>header { display:flex; justify-content:space-between; align-items:center; gap:12px; }</style>
