<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import AddSpeechForm from './AddSpeechForm.svelte';
  let { editor }: { editor: EditorController } = $props();
  let open = $state(false);
  let submittedId = $state('');
  let dialog = $state<HTMLDialogElement>();
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    // Includes successful replay through EditorNotices after an uncertain result.
    if (open && submittedId && !editor.busy && !editor.pending && !editor.drafting
      && editor.snapshot?.project.clips.some(c => c.id === submittedId)) {
      editor.revealClip(submittedId); submittedId = ''; open = false;
    }
  });
  function close() {
    if (editor.busy || editor.pending) return;
    editor.acceptIncoming(); submittedId = ''; open = false;
  }
</script>
<button disabled={!editor.snapshot || editor.busy || editor.drafting || !!editor.pending} onclick={() => { submittedId = ''; open = true; }}>{locale.text('speechAddButton')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('speechAddTitle')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('speechAddTitle')}</h2><button disabled={editor.busy || !!editor.pending} onclick={close}>{editor.drafting ? locale.text('speechDiscardClose') : locale.text('close')}</button></header>
    <EditorNotices {editor} />
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.draftGeneration}`}
      <AddSpeechForm {editor} submitting={id => { submittedId = id; }} />
    {/key}
  </dialog>
{/if}
<style>header { display:flex; justify-content:space-between; align-items:center; gap:12px; }</style>
