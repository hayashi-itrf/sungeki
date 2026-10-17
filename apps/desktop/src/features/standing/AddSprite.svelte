<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import AddSpriteForm from './AddSpriteForm.svelte';
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
<button disabled={!editor.snapshot || editor.busy || editor.drafting || !!editor.pending} onclick={() => { submittedId = ''; open = true; }}>{locale.text('standingSpriteButton')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('standingSpriteAdd')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('standingSpriteAdd')}</h2><button disabled={editor.busy || !!editor.pending} onclick={close}>{editor.drafting ? locale.text('standingDiscardClose') : locale.text('standingClose')}</button></header>
    <EditorNotices {editor} />
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.draftGeneration}`}
      <AddSpriteForm {editor} submitting={id => { submittedId = id; }} />
    {/key}
  </dialog>
{/if}
<style>header { display:flex; justify-content:space-between; align-items:center; gap:12px; }</style>
